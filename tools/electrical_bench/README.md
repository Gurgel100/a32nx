# electrical_bench

Deterministic performance benchmark for the **A320** and **A380** electrical
systems. It exists so the legacy per-tick-orchestrated electrical engine can be
measured and compared, tick for tick, against the future compiled-graph engine
using the *same* harness.

The benchmark drives each aircraft's electrical system through the same
`SimulationTestBed` and stub-aircraft fixtures the electrical golden tests use
(exposed from `a320_systems` / `a380_systems` via their opt-in `bench` cargo
feature). Because the stub aircraft are electrical-centric, the measured cost
is dominated by the electrical engine itself.

Nothing in this crate is part of a simulator build; the `bench` features are
off by default, so wasm/CI builds of the aircraft crates are unaffected.

## Usage

```sh
# Build & run everything (prints JSON to stdout)
cargo build -p electrical_bench --release
cargo run -p electrical_bench --release

# Save a baseline report
cargo run -p electrical_bench --release -- --label legacy --out tools/electrical_bench/baselines/legacy-YYYY-MM-DD.json

# Compare two reports (prints per-scenario median/p95 ratios current / baseline;
# < 1.0 means the current build is faster)
cargo run -p electrical_bench --release -- --compare baselines/legacy-2026-11-27.json new.json

# Check that each scenario reaches its intended bus configuration
cargo run -p electrical_bench --release -- --sanity

# Useful knobs (see --help for all)
#   --only a320      run only matching scenarios
#   --ticks N        measured ticks per repetition (default 5000)
#   --reps N         measured repetitions (default 5)
#   --settle N       settle/warmup ticks (default 200)
#   --seed N         base RNG seed (default 0xfa320380beef1234)
```

## Scenarios (identical list for both aircraft)

| Scenario           | Configuration                                                                 |
| ------------------ | ----------------------------------------------------------------------------- |
| `engines_online`   | All engine generators powering (A320: 2, A380: 4), in flight at 250 kt.       |
| `ext_apu`          | External power connected & on + APU generator running.                        |
| `emergency`        | No AC source at all, IAS > 100 kt in flight; the RAT/emergency-generator configuration emerges during the settle phase. |
| `battery_only`     | On ground, 0 kt, no generators and no external power; batteries power the DC buses. |
| `switching_storm`  | Engines + external power + APU running; a seeded RNG toggles overhead pushbuttons (gens, APU gen, ext pwr, BATs, bus tie, AC ESS feed, etc.) during the measured phase. |

Scenario keys are prefixed `a320/…` and `a380/…`. Each scenario runs a fixed
settle (warmup) phase, then `ticks × reps` measured ticks; per-tick durations
are collected and reported as `min_ns`, `median_ns`, `p95_ns`, `max_ns`.

## Determinism

* Fixed 100 ms simulation delta; the whole electrical tick runs inside
  `SimulationTestBed::run_with_delta`.
* Seeded RNG (`StdRng::seed_from_u64`) with a stable per-scenario seed derived
  from the base seed via FNV-1a, so scenarios never share a stream.
* The report records `git_sha` (captured at *build* time via `build.rs`, or
  from the `ELECTRICAL_BENCH_GIT_SHA` env var), timestamp, host CPU count/OS
  and all bench settings.

Comparisons are only meaningful between runs of this same binary version on
the same machine; always compare like for like (same `--ticks/--reps/--seed`).

## Benching the new engine

When the electrical system is rewritten:

1. Keep this harness (and the fixture entry points in `bench_support`) intact.
2. Build & run `cargo run -p electrical_bench --release -- --label new --out new.json`.
3. `cargo run -p electrical_bench --release -- --compare baselines/legacy-2026-11-27.json new.json`

## Baselines

`baselines/legacy-2026-11-27.json` — legacy implementation, captured on
2026-11-27 on a Linux x86_64 host (see the `host` field inside the file).
