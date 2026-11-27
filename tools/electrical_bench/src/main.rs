//! Deterministic performance benchmark for the electrical systems of the
//! A320 and the A380.
//!
//! The harness drives each aircraft's electrical system through the same
//! `SimulationTestBed` used by the electrical golden tests (exposed via the
//! `bench` feature of `a320_systems`/`a380_systems`), so the measured cost is
//! dominated by the electrical engine itself. It is designed to produce
//! comparable numbers across implementations: run it once against the legacy
//! (`--label legacy`) and once against a rewritten engine (`--label new`),
//! then compare the two JSON reports with `--compare`.
//!
//! Determinism: fixed 100 ms simulation delta, fixed settle phase, fixed
//! number of measured ticks and repetitions, and seeded RNG
//! (`StdRng::seed_from_u64`) for the `switching_storm` scenario.
//!
//! Run with `--help` for usage.

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use systems::{
    electrical::Electricity,
    shared::{ElectricalBusType, ElectricalBuses},
    simulation::test::{TestBed, WriteByName},
};

const DEFAULT_LABEL: &str = "run";
const DEFAULT_DELTA_MS: u64 = 100;
/// Ticks run after building a scenario and before measuring. Doubles as the
/// warmup phase; long enough for slow behaviour such as RAT/emergency
/// generator availability (~8 s) to emerge at 100 ms per tick.
const DEFAULT_SETTLE_TICKS: usize = 200;
const DEFAULT_MEASURED_TICKS: usize = 5000;
const DEFAULT_REPETITIONS: usize = 5;
const DEFAULT_SEED: u64 = 0xFA32_0380_BEEF_1234;

const SCENARIOS: [&str; 10] = [
    "a320/engines_online",
    "a320/ext_apu",
    "a320/emergency",
    "a320/battery_only",
    "a320/switching_storm",
    "a380/engines_online",
    "a380/ext_apu",
    "a380/emergency",
    "a380/battery_only",
    "a380/switching_storm",
];

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
struct ScenarioStats {
    min_ns: f64,
    median_ns: f64,
    p95_ns: f64,
    max_ns: f64,
    ticks: usize,
}

impl ScenarioStats {
    fn from_samples(samples: &[u64]) -> Self {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let quantile = |q: f64| {
            let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
            sorted[idx] as f64
        };
        Self {
            min_ns: sorted[0] as f64,
            median_ns: quantile(0.5),
            p95_ns: quantile(0.95),
            max_ns: *sorted.last().unwrap_or(&sorted[0]) as f64,
            ticks: samples.len(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct HostInfo {
    cpu_count: usize,
    os: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
struct Settings {
    delta_ms: u64,
    settle_ticks: usize,
    measured_ticks: usize,
    repetitions: usize,
    seed: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct BenchReport {
    label: String,
    git_sha: String,
    timestamp: String,
    host: HostInfo,
    settings: Settings,
    results: BTreeMap<String, ScenarioStats>,
}

/// Toggles a set of overhead pushbuttons with a seeded RNG. State is tracked
/// locally so that every tick writes a full coherent picture of the panel.
struct PbStorm {
    pushbuttons: Vec<(&'static str, bool)>,
    toggle_probability: f64,
}

impl PbStorm {
    fn new(names: &[&'static str]) -> Self {
        Self {
            pushbuttons: names.iter().map(|name| (*name, true)).collect(),
            toggle_probability: 0.05,
        }
    }

    fn tick<B: TestBed>(&mut self, bed: &mut B, rng: &mut StdRng) {
        for (name, state) in &mut self.pushbuttons {
            if rng.random_bool(self.toggle_probability) {
                *state = !*state;
                bed.write_by_name(name, *state);
            }
        }
    }
}

const A320_SWITCHING_STORM_PBS: &[&str] = &[
    "OVHD_ELEC_ENG_GEN_1_PB_IS_ON",
    "OVHD_ELEC_ENG_GEN_2_PB_IS_ON",
    "OVHD_ELEC_APU_GEN_PB_IS_ON",
    "OVHD_ELEC_EXT_PWR_PB_IS_ON",
    "OVHD_ELEC_BAT_1_PB_IS_AUTO",
    "OVHD_ELEC_BAT_2_PB_IS_AUTO",
    "OVHD_ELEC_BUS_TIE_PB_IS_AUTO",
    "OVHD_ELEC_AC_ESS_FEED_PB_IS_NORMAL",
    "OVHD_ELEC_COMMERCIAL_PB_IS_ON",
    "OVHD_ELEC_GALY_AND_CAB_PB_IS_AUTO",
    "OVHD_EMER_ELEC_GEN_1_LINE_PB_IS_ON",
    "OVHD_EMER_ELEC_RAT_AND_EMER_GEN_IS_PRESSED",
];

const A380_SWITCHING_STORM_PBS: &[&str] = &[
    "OVHD_ELEC_ENG_GEN_1_PB_IS_ON",
    "OVHD_ELEC_ENG_GEN_2_PB_IS_ON",
    "OVHD_ELEC_ENG_GEN_3_PB_IS_ON",
    "OVHD_ELEC_ENG_GEN_4_PB_IS_ON",
    "OVHD_ELEC_APU_GEN_1_PB_IS_ON",
    "OVHD_ELEC_APU_GEN_2_PB_IS_ON",
    "OVHD_ELEC_EXT_PWR_1_PB_IS_ON",
    "OVHD_ELEC_EXT_PWR_2_PB_IS_ON",
    "OVHD_ELEC_EXT_PWR_3_PB_IS_ON",
    "OVHD_ELEC_EXT_PWR_4_PB_IS_ON",
    "OVHD_ELEC_BAT_1_PB_IS_AUTO",
    "OVHD_ELEC_BAT_2_PB_IS_AUTO",
    "OVHD_ELEC_BAT_ESS_PB_IS_AUTO",
    "OVHD_ELEC_BAT_APU_PB_IS_AUTO",
    "OVHD_ELEC_BUS_TIE_PB_IS_AUTO",
    "OVHD_ELEC_AC_ESS_FEED_PB_IS_NORMAL",
    "OVHD_EMER_ELEC_RAT_AND_EMER_GEN_IS_PRESSED",
];

/// Stable per-scenario seed: same base seed, but scenarios do not share an
/// RNG stream (so adding/removing scenarios cannot change other results).
fn seed_for(base: u64, scenario: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in scenario.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    base ^ hash
}

fn bench_scenario<B: TestBed>(
    mut fixture: impl FnMut() -> B,
    mut per_tick: impl FnMut(&mut B, &mut StdRng),
    settings: &Settings,
    seed: u64,
) -> ScenarioStats {
    let delta = Duration::from_millis(settings.delta_ms);

    let mut bed = fixture();
    bed.run_iterations_with_delta(settings.settle_ticks as u32, delta);

    let mut rng = StdRng::seed_from_u64(seed);
    let mut samples = vec![0_u64; settings.measured_ticks * settings.repetitions];
    for sample in samples.iter_mut() {
        per_tick(&mut bed, &mut rng);
        let start = Instant::now();
        bed.run_with_delta(delta);
        *sample = start.elapsed().as_nanos() as u64;
    }

    ScenarioStats::from_samples(&samples)
}

mod a320 {
    use a320_systems::bench_support::{self as fixtures, A320ElectricalTestBed};
    use uom::si::{f64::Velocity, velocity::knot};

    pub fn engines_online() -> A320ElectricalTestBed {
        fixtures::test_bed_with().running_engines().run()
    }

    pub fn ext_apu() -> A320ElectricalTestBed {
        fixtures::test_bed_with()
            .connected_external_power()
            .running_apu()
            .run()
    }

    /// In flight at 250 kt with no generator and no external power: loss of
    /// AC makes the emergency config (RAT + emergency generator) emerge
    /// during the settle phase.
    pub fn emergency() -> A320ElectricalTestBed {
        fixtures::test_bed_with().run()
    }

    pub fn battery_only() -> A320ElectricalTestBed {
        fixtures::test_bed_with()
            .airspeed(Velocity::new::<knot>(0.))
            .on_the_ground()
            .run()
    }

    /// Rich source configuration for the pushbutton storm.
    pub fn switching_storm() -> A320ElectricalTestBed {
        fixtures::test_bed_with()
            .running_engines()
            .connected_external_power()
            .running_apu()
            .run()
    }
}

mod a380 {
    use a380_systems::bench_support::{self as fixtures, A380ElectricalTestBed};
    use uom::si::{f64::Velocity, velocity::knot};

    pub fn engines_online() -> A380ElectricalTestBed {
        fixtures::test_bed_with().running_engines().run()
    }

    pub fn ext_apu() -> A380ElectricalTestBed {
        fixtures::test_bed_with()
            .connected_external_power()
            .ext_pwr_on(1)
            .ext_pwr_on(2)
            .running_apu()
            .run()
    }

    /// In flight at 250 kt with no generator and no external power: loss of
    /// AC makes the emergency config (RAT + emergency generator) emerge
    /// during the settle phase.
    pub fn emergency() -> A380ElectricalTestBed {
        fixtures::test_bed_with().run()
    }

    pub fn battery_only() -> A380ElectricalTestBed {
        fixtures::test_bed_with()
            .airspeed(Velocity::new::<knot>(0.))
            .on_the_ground()
            .run()
    }

    /// Rich source configuration for the pushbutton storm.
    pub fn switching_storm() -> A380ElectricalTestBed {
        fixtures::test_bed_with()
            .running_engines()
            .connected_external_power()
            .ext_pwr_on(1)
            .ext_pwr_on(2)
            .running_apu()
            .run()
    }
}

fn run_a320(settings: &Settings, only: Option<&str>) -> BTreeMap<String, ScenarioStats> {
    let mut results = BTreeMap::new();

    macro_rules! scenario {
        ($name:literal, $fixture:expr, $per_tick:expr) => {{
            if only.is_none_or(|filter| $name.contains(filter)) {
                let stats = bench_scenario(
                    $fixture,
                    $per_tick,
                    settings,
                    seed_for(settings.seed, $name),
                );
                results.insert($name.to_owned(), stats);
            }
        }};
    }

    scenario!(
        "a320/engines_online",
        a320::engines_online,
        |_: &mut _, _| {}
    );
    scenario!("a320/ext_apu", a320::ext_apu, |_: &mut _, _| {});
    scenario!("a320/emergency", a320::emergency, |_: &mut _, _| {});
    scenario!("a320/battery_only", a320::battery_only, |_: &mut _, _| {});
    {
        let mut storm = PbStorm::new(A320_SWITCHING_STORM_PBS);
        scenario!("a320/switching_storm", a320::switching_storm, |bed, rng| {
            storm.tick(bed, rng)
        });
    }

    results
}

fn run_a380(settings: &Settings, only: Option<&str>) -> BTreeMap<String, ScenarioStats> {
    let mut results = BTreeMap::new();

    macro_rules! scenario {
        ($name:literal, $fixture:expr, $per_tick:expr) => {{
            if only.is_none_or(|filter| $name.contains(filter)) {
                let stats = bench_scenario(
                    $fixture,
                    $per_tick,
                    settings,
                    seed_for(settings.seed, $name),
                );
                results.insert($name.to_owned(), stats);
            }
        }};
    }

    scenario!(
        "a380/engines_online",
        a380::engines_online,
        |_: &mut _, _| {}
    );
    scenario!("a380/ext_apu", a380::ext_apu, |_: &mut _, _| {});
    scenario!("a380/emergency", a380::emergency, |_: &mut _, _| {});
    scenario!("a380/battery_only", a380::battery_only, |_: &mut _, _| {});
    {
        let mut storm = PbStorm::new(A380_SWITCHING_STORM_PBS);
        scenario!("a380/switching_storm", a380::switching_storm, |bed, rng| {
            storm.tick(bed, rng)
        });
    }

    results
}

/// Prints bus power states after the settle phase so that a scenario can be
/// checked to actually reach its intended configuration (`--sanity`).
fn sanity_scenario<B: TestBed>(name: &str, mut fixture: impl FnMut() -> B, settings: &Settings) {
    let delta = Duration::from_millis(settings.delta_ms);
    let mut bed = fixture();
    bed.run_iterations_with_delta(settings.settle_ticks as u32, delta);

    let powered = |bus: ElectricalBusType| {
        bed.query_elec_ref(|_, electricity: &Electricity| electricity.potential_of(bus))
            .is_powered()
    };
    let yes = |on: bool| if on { "yes" } else { "no " };
    println!(
        "{name:<26} AC1={} AC2={} AC ESS={} DC ESS={}",
        yes(powered(ElectricalBusType::AlternatingCurrent(1))),
        yes(powered(ElectricalBusType::AlternatingCurrent(2))),
        yes(powered(ElectricalBusType::AlternatingCurrentEssential)),
        yes(powered(ElectricalBusType::DirectCurrentEssential))
    );
}

fn run_sanity(settings: &Settings, only: Option<&str>) {
    let selected = |name: &str| only.is_none_or(|filter| name.contains(filter));

    if selected("a320/engines_online") {
        sanity_scenario("a320/engines_online", a320::engines_online, settings);
    }
    if selected("a320/ext_apu") {
        sanity_scenario("a320/ext_apu", a320::ext_apu, settings);
    }
    if selected("a320/emergency") {
        sanity_scenario("a320/emergency", a320::emergency, settings);
    }
    if selected("a320/battery_only") {
        sanity_scenario("a320/battery_only", a320::battery_only, settings);
    }
    if selected("a320/switching_storm") {
        sanity_scenario("a320/switching_storm", a320::switching_storm, settings);
    }
    if selected("a380/engines_online") {
        sanity_scenario("a380/engines_online", a380::engines_online, settings);
    }
    if selected("a380/ext_apu") {
        sanity_scenario("a380/ext_apu", a380::ext_apu, settings);
    }
    if selected("a380/emergency") {
        sanity_scenario("a380/emergency", a380::emergency, settings);
    }
    if selected("a380/battery_only") {
        sanity_scenario("a380/battery_only", a380::battery_only, settings);
    }
    if selected("a380/switching_storm") {
        sanity_scenario("a380/switching_storm", a380::switching_storm, settings);
    }
}

/// Howard Hinnant's `civil_from_days`, used to render an RFC 3339 UTC
/// timestamp without pulling in a date-time dependency.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = (days - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn utc_timestamp(now: SystemTime) -> String {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    let time_of_day = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60
    )
}

fn host_info() -> HostInfo {
    HostInfo {
        cpu_count: std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1),
        os: format!("{} {}", env::consts::OS, env::consts::ARCH),
    }
}

fn run(settings: Settings, label: String, out: Option<PathBuf>, only: Option<String>) {
    let mut results = run_a320(&settings, only.as_deref());
    results.extend(run_a380(&settings, only.as_deref()));

    let report = BenchReport {
        label,
        git_sha: env!("ELECTRICAL_BENCH_GIT_SHA").to_owned(),
        timestamp: utc_timestamp(SystemTime::now()),
        host: host_info(),
        settings,
        results,
    };

    let json = serde_json::to_string_pretty(&report).expect("serialising the report cannot fail");
    match out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("creating the output directory failed");
            }
            let mut file = fs::File::create(&path).expect("creating the output file failed");
            writeln!(file, "{json}").expect("writing the output file failed");
            eprintln!("wrote report to {}", path.display());
        }
        None => println!("{json}"),
    }
}

fn ratio(current: f64, baseline: f64) -> f64 {
    if baseline == 0. {
        f64::NAN
    } else {
        current / baseline
    }
}

fn compare(baseline_path: &Path, current_path: &Path) -> Result<(), Box<dyn Error>> {
    let baseline: BenchReport = serde_json::from_str(&fs::read_to_string(baseline_path)?)?;
    let current: BenchReport = serde_json::from_str(&fs::read_to_string(current_path)?)?;

    println!(
        "baseline: {} ({} @ {}) current: {} ({} @ {})",
        baseline.label,
        baseline.git_sha,
        baseline.timestamp,
        current.label,
        current.git_sha,
        current.timestamp
    );
    println!();
    println!(
        "{:<26} {:>13} {:>13} {:>9} {:>9}",
        "scenario", "baseline_ns", "current_ns", "median", "p95"
    );

    let mut missing = Vec::new();
    for (name, base) in &baseline.results {
        match current.results.get(name) {
            Some(curr) => println!(
                "{name:<26} {:>13.0} {:>13.0} {:>9.3} {:>9.3}",
                base.median_ns,
                curr.median_ns,
                ratio(curr.median_ns, base.median_ns),
                ratio(curr.p95_ns, base.p95_ns)
            ),
            None => missing.push(name.clone()),
        }
    }
    for name in current.results.keys() {
        if !baseline.results.contains_key(name) {
            println!("{name:<26} {:>13} {:>13.0} {:>9} {:>9}", "-", 0., "-", "-");
        }
    }
    for name in missing {
        println!("{name:<26} missing in current run");
    }

    println!();
    println!(
        "median/p95 columns are ratios current / baseline; < 1.0 means the current build is faster"
    );
    Ok(())
}

fn parse_u64(value: &str, flag: &str) -> u64 {
    value
        .parse()
        .unwrap_or_else(|_| exit_error(&format!("{flag} expects a number, got '{value}'")))
}

fn exit_error(message: &str) -> ! {
    eprintln!("error: {message}");
    process::exit(2)
}

fn usage() {
    println!(
        "Electrical systems performance benchmark (A320 & A380)

Usage:
  electrical_bench [OPTIONS]
    --label <label>      Label stored in the report (e.g. legacy, new)
    --out <file.json>    Write the JSON report to a file (default: stdout)
    --reps <n>           Measured repetitions (default {DEFAULT_REPETITIONS})
    --ticks <n>          Measured ticks per repetition (default {DEFAULT_MEASURED_TICKS})
    --settle <n>         Settle (warmup) ticks per scenario (default {DEFAULT_SETTLE_TICKS})
    --delta-ms <ms>      Simulation delta per tick (default {DEFAULT_DELTA_MS})
    --seed <n>           Base RNG seed (default {DEFAULT_SEED:#x})
    --only <substring>   Run only scenarios whose name contains <substring>

  electrical_bench --compare <baseline.json> <current.json>
    Prints per-scenario median/p95 ratios current / baseline (< 1.0 = faster).

  electrical_bench --sanity [--only <substring>]
    Settles each scenario and prints bus power states, to check that a
    scenario reaches its intended configuration.

Scenarios:
  {}
",
        SCENARIOS.join("\n  ")
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut compare_mode = false;
    let mut sanity_mode = false;
    let mut label = DEFAULT_LABEL.to_owned();
    let mut out: Option<PathBuf> = None;
    let mut delta_ms = DEFAULT_DELTA_MS;
    let mut settle_ticks = DEFAULT_SETTLE_TICKS;
    let mut measured_ticks = DEFAULT_MEASURED_TICKS;
    let mut repetitions = DEFAULT_REPETITIONS;
    let mut seed = DEFAULT_SEED;
    let mut only: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();

    fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
        args.next()
            .unwrap_or_else(|| exit_error(&format!("{flag} needs a value")))
    }

    let mut args = args.into_iter().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                usage();
                return;
            }
            "--compare" => compare_mode = true,
            "--sanity" => sanity_mode = true,
            "--label" => label = next_value(&mut args, "--label"),
            "--out" => out = Some(PathBuf::from(next_value(&mut args, "--out"))),
            "--delta-ms" => {
                delta_ms = parse_u64(&next_value(&mut args, "--delta-ms"), "--delta-ms")
            }
            "--settle" => {
                settle_ticks = parse_u64(&next_value(&mut args, "--settle"), "--settle") as usize
            }
            "--ticks" => {
                measured_ticks = parse_u64(&next_value(&mut args, "--ticks"), "--ticks") as usize
            }
            "--reps" => {
                repetitions = parse_u64(&next_value(&mut args, "--reps"), "--reps") as usize
            }
            "--seed" => seed = parse_u64(&next_value(&mut args, "--seed"), "--seed"),
            "--only" => only = Some(next_value(&mut args, "--only")),
            other => {
                if other.starts_with('-') {
                    exit_error(&format!("unknown option '{other}'"));
                }
                positional.push(other.to_owned());
            }
        }
    }

    if compare_mode {
        if positional.len() != 2 {
            exit_error("--compare expects exactly two paths: <baseline.json> <current.json>");
        }
        if let Err(error) = compare(Path::new(&positional[0]), Path::new(&positional[1])) {
            exit_error(&format!("comparing failed: {error}"));
        }
        return;
    }

    let settings = Settings {
        delta_ms,
        settle_ticks,
        measured_ticks,
        repetitions,
        seed,
    };

    if sanity_mode {
        run_sanity(&settings, only.as_deref());
        return;
    }

    if delta_ms == 0 {
        exit_error("--delta-ms must be positive");
    }

    eprintln!(
        "electrical_bench: label={label} delta={delta_ms}ms settle={settle_ticks} ticks={measured_ticks} reps={repetitions} seed={seed:#x}"
    );
    let total_ticks: usize = SCENARIOS
        .iter()
        .filter(|name| only.as_deref().is_none_or(|f| name.contains(f)))
        .count()
        * (settle_ticks + measured_ticks * repetitions);
    eprintln!("running {total_ticks} simulation ticks in total...");

    run(settings, label, out, only);
}
