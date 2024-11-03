use a380_systems::A380;
use std::time::Duration;
use systems::{
    failures::FailureType,
    pneumatic::EngineState,
    shared::{arinc429::Arinc429Word, InternationalStandardAtmosphere},
    simulation::test::{ReadByName, SimulationTestBed, TestBed, WriteByName},
};
use uom::si::{
    f64::*, length::foot, ratio::percent, thermodynamic_temperature::degree_celsius, velocity::knot,
};

pub fn main() {
    let mut test_bed = SimulationTestBed::new(A380::new);
    test_bed.set_on_ground(true);
    for e in 1..=4 {
        set_engine_power(
            &mut test_bed,
            e,
            Ratio::new::<percent>(85.),
            Ratio::new::<percent>(92.),
            Ratio::new::<percent>(95.),
        );
    }
    test_bed.run_multiple_frames(Duration::from_secs(1));

    println!("----------------------  Setting to inflight  ----------------------");
    test_bed.set_ambient_temperature(ThermodynamicTemperature::new::<degree_celsius>(-55.));
    test_bed.set_ambient_pressure(InternationalStandardAtmosphere::pressure_at_altitude(
        Length::new::<foot>(40_000.),
    ));
    test_bed.set_pressure_altitude(Length::new::<foot>(40_000.));
    test_bed.set_true_airspeed(Velocity::new::<knot>(400.));
    test_bed.set_on_ground(true);
    test_bed.fail(FailureType::RapidDecompression);

    let frame_time = Duration::from_secs_f64(1. / 20.);
    let total_time = Duration::from_secs(3600 * 24);
    let mut executed_duration = Duration::from_secs(0);
    //println!("{:?}", test_bed.variable_registry);
    while executed_duration < total_time {
        test_bed.run_with_delta(frame_time);
        executed_duration += frame_time;
        print_cabin_pressure_status(&mut test_bed);
        print_temperature_stats(&mut test_bed);
    }
}

fn set_engine_power(
    test_bed: &mut SimulationTestBed<A380>,
    engine: usize,
    n1: Ratio,
    n2: Ratio,
    n3: Ratio,
) {
    test_bed.write_by_name(&format!("GENERAL ENG STARTER ACTIVE:{engine}"), true);
    test_bed.write_by_name(&format!("TURB ENG CORRECTED N2:{engine}"), n1);
    test_bed.write_by_name(&format!("TURB ENG CORRECTED N1:{engine}"), n2);
    test_bed.write_by_name(&format!("ENGINE_N3:{engine}"), n3);
    test_bed.write_by_name(&format!("ENGINE_STATE:{engine}"), EngineState::On);
}

fn print_cabin_pressure_status(test_bed: &mut SimulationTestBed<A380>) {
    let cpcs_b1_discrete_word: Arinc429Word<u32> =
        test_bed.read_arinc429_by_name("COND_CPIOM_B1_CPCS_DISCRETE_WORD");
    let cpcs_b2_discrete_word: Arinc429Word<u32> =
        test_bed.read_arinc429_by_name("COND_CPIOM_B2_CPCS_DISCRETE_WORD");
    let cpcs_b3_discrete_word: Arinc429Word<u32> =
        test_bed.read_arinc429_by_name("COND_CPIOM_B3_CPCS_DISCRETE_WORD");
    let cpcs_b4_discrete_word: Arinc429Word<u32> =
        test_bed.read_arinc429_by_name("COND_CPIOM_B4_CPCS_DISCRETE_WORD");

    let cpcs_to_use = if cpcs_b1_discrete_word.is_normal_operation() {
        1
    } else if cpcs_b2_discrete_word.is_normal_operation() {
        2
    } else if cpcs_b3_discrete_word.is_normal_operation() {
        3
    } else if cpcs_b4_discrete_word.is_normal_operation() {
        4
    } else {
        0
    };

    let man_delta_psi: f64 = test_bed.read_by_name("PRESS_MAN_CABIN_DELTA_PRESSURE");
    let cabin_alt_arinc: Arinc429Word<f64> =
        test_bed.read_arinc429_by_name(&format!("PRESS_CABIN_ALTITUDE_B{cpcs_to_use}"));
    let delta_psi_arinc: Arinc429Word<f64> =
        test_bed.read_arinc429_by_name(&format!("PRESS_CABIN_DELTA_PRESSURE_B{cpcs_to_use}"));
    let cabin_vs_arinc: Arinc429Word<f64> =
        test_bed.read_arinc429_by_name(&format!("PRESS_CABIN_VS_B{cpcs_to_use}"));

    let delta_psi = delta_psi_arinc.normal_value().unwrap_or(man_delta_psi);

    println!(
        "{:.2?}f {:.2}psi {:.2?}fpm",
        cabin_alt_arinc.normal_value(),
        delta_psi,
        cabin_vs_arinc.normal_value()
    );
    println!(
        "{:?} {:?} {:?}",
        cabin_alt_arinc.ssm(),
        delta_psi_arinc.ssm(),
        cabin_vs_arinc.ssm()
    );
}

fn print_temperature_stats(test_bed: &mut SimulationTestBed<A380>) {
    let cockpit_cabin_temp: ThermodynamicTemperature =
        test_bed.read_by_name("A32NX_COND_CKPT_TEMP");
    let fwd_cargo_temp: ThermodynamicTemperature =
        test_bed.read_by_name("A32NX_COND_CARGO_FWD_TEMP");
    let aft_cargo_temp: ThermodynamicTemperature =
        test_bed.read_by_name("A32NX_COND_CARGO_BULK_TEMP");

    const MAIN_CABIN_ZONES: [&str; 8] = [
        "MAIN_DECK_1",
        "MAIN_DECK_2",
        "MAIN_DECK_3",
        "MAIN_DECK_4",
        "MAIN_DECK_5",
        "MAIN_DECK_6",
        "MAIN_DECK_7",
        "MAIN_DECK_8",
    ];

    const UPPER_CABIN_ZONES: [&str; 7] = [
        "UPPER_DECK_1",
        "UPPER_DECK_2",
        "UPPER_DECK_3",
        "UPPER_DECK_4",
        "UPPER_DECK_5",
        "UPPER_DECK_6",
        "UPPER_DECK_7",
    ];

    let main_deck_temps: [ThermodynamicTemperature; 8] =
        MAIN_CABIN_ZONES.map(|zone| test_bed.read_by_name(&format!("COND_{zone}_TEMP")));
    let upper_deck_temps: [ThermodynamicTemperature; 7] =
        UPPER_CABIN_ZONES.map(|zone| test_bed.read_by_name(&format!("COND_{zone}_TEMP")));

    println!("Upper deck: {upper_deck_temps:.2?}");
    println!("Main deck : {main_deck_temps:.2?}");
    println!("Cockpit   : {cockpit_cabin_temp:.2?}");
    println!("Cargo     : {fwd_cargo_temp:.2?} {aft_cargo_temp:.2?}");
}
