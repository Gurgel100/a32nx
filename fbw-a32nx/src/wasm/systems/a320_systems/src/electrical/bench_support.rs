//! Stub aircraft and fluent fixture builders shared by the electrical golden
//! tests (`a320_electrical_circuit_tests` in `mod.rs`) and the standalone
//! `tools/electrical_bench` performance harness.
//!
//! The code in this module was extracted verbatim from the `#[cfg(test)]`
//! module so that the bench harness (an external crate) can drive the
//! electrical system through the same `SimulationTestBed` API the tests use.
//! It is compiled either during unit tests or when the `bench` cargo feature
//! is enabled; default (wasm/CI) builds never see it.

use super::alternating_current::A320AcEssFeedContactors;
use super::*;
use std::{cell::Ref, time::Duration};
use systems::{
    apu::ApuGenerator,
    electrical::{
        ElectricalElement, ElectricalElementIdentifier, ElectricalElementIdentifierProvider,
        ElectricitySource, Potential, ProvideFrequency, ProvidePotential,
        INTEGRATED_DRIVE_GENERATOR_STABILIZATION_TIME,
    },
    failures::FailureType,
    shared::{
        ApuAvailable, ContactorSignal, ControllerSignal, ElectricalBusType, ElectricalBuses,
        EngineCorrectedN1, EngineCorrectedN2, EngineUncorrectedN2, PotentialOrigin,
    },
    simulation::{
        test::{ReadByName, SimulationTestBed, TestBed, WriteByName},
        Aircraft,
    },
};
use uom::si::{
    electric_potential::volt,
    frequency::hertz,
    length::foot,
    power::watt,
    ratio::{percent, ratio},
    velocity::knot,
};
pub fn test_bed_with() -> A320ElectricalTestBed {
    test_bed()
}

pub fn test_bed() -> A320ElectricalTestBed {
    A320ElectricalTestBed::new()
}

struct TestApu {
    gen: TestApuGenerator,
    is_available: bool,
    start_motor_is_powered: bool,
    should_close_start_contactor: bool,
}
impl TestApu {
    fn new(context: &mut InitContext) -> Self {
        Self {
            gen: TestApuGenerator::new(context),
            is_available: false,
            start_motor_is_powered: false,
            should_close_start_contactor: false,
        }
    }

    fn set_available(&mut self, available: bool) {
        self.gen.set_available(available);
        self.is_available = available;
    }

    fn fail_generator(&mut self) {
        self.gen.set_fault(true);
    }

    fn command_closing_of_start_contactors(&mut self) {
        self.should_close_start_contactor = true;
    }

    fn start_motor_is_powered(&self) -> bool {
        self.start_motor_is_powered
    }
}
impl SimulationElement for TestApu {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.gen.accept(visitor);
        visitor.visit(self);
    }

    fn receive_power(&mut self, buses: &impl ElectricalBuses) {
        self.start_motor_is_powered = buses.is_powered(ElectricalBusType::Sub("49-42-00"));
    }
}
impl AuxiliaryPowerUnitElectrical for TestApu {
    type Generator = TestApuGenerator;

    fn generator(&self, number: usize) -> &Self::Generator {
        assert_eq!(number, 1);
        &self.gen
    }
}
impl ApuAvailable for TestApu {
    fn is_available(&self) -> bool {
        self.is_available
    }
}
impl ControllerSignal<ContactorSignal> for TestApu {
    fn signal(&self) -> Option<ContactorSignal> {
        if self.should_close_start_contactor {
            Some(ContactorSignal::Close)
        } else {
            Some(ContactorSignal::Open)
        }
    }
}

struct TestApuGenerator {
    identifier: ElectricalElementIdentifier,
    is_available: bool,
    has_fault: bool,
}
impl TestApuGenerator {
    fn new(context: &mut InitContext) -> Self {
        Self {
            identifier: context.next_electrical_identifier(),
            is_available: false,
            has_fault: false,
        }
    }

    fn set_available(&mut self, available: bool) {
        self.is_available = available;
    }

    fn set_fault(&mut self, has_fault: bool) {
        self.has_fault = has_fault;
    }
}
impl ApuGenerator for TestApuGenerator {
    fn update(&mut self, _n: Ratio, _is_emergency_shutdown: bool) {}

    fn output_within_normal_parameters(&self) -> bool {
        self.is_available && !self.has_fault
    }
}
impl ProvidePotential for TestApuGenerator {
    fn potential(&self) -> ElectricPotential {
        if self.output_within_normal_parameters() {
            ElectricPotential::new::<volt>(115.)
        } else {
            ElectricPotential::default()
        }
    }

    fn potential_normal(&self) -> bool {
        self.output_within_normal_parameters()
    }
}
impl ProvideFrequency for TestApuGenerator {
    fn frequency(&self) -> Frequency {
        if self.output_within_normal_parameters() {
            Frequency::new::<hertz>(400.)
        } else {
            Frequency::default()
        }
    }

    fn frequency_normal(&self) -> bool {
        self.output_within_normal_parameters()
    }
}
impl ElectricitySource for TestApuGenerator {
    fn output_potential(&self) -> Potential {
        if self.output_within_normal_parameters() {
            Potential::new(
                PotentialOrigin::ApuGenerator(1),
                ElectricPotential::new::<volt>(115.),
            )
        } else {
            Potential::none()
        }
    }
}
impl ElectricalElement for TestApuGenerator {
    fn input_identifier(&self) -> systems::electrical::ElectricalElementIdentifier {
        self.identifier
    }

    fn output_identifier(&self) -> systems::electrical::ElectricalElementIdentifier {
        self.identifier
    }

    fn is_conductive(&self) -> bool {
        true
    }
}
impl SimulationElement for TestApuGenerator {}

struct TestEngineFirePushButtons {
    is_released: [bool; 2],
}
impl TestEngineFirePushButtons {
    fn new() -> Self {
        Self {
            is_released: [false, false],
        }
    }

    fn release(&mut self, engine_number: usize) {
        self.is_released[engine_number - 1] = true;
    }
}
impl EngineFirePushButtons for TestEngineFirePushButtons {
    fn is_released(&self, engine_number: usize) -> bool {
        self.is_released[engine_number - 1]
    }
}

struct TestEngine {
    is_running: bool,
}
impl TestEngine {
    fn new() -> Self {
        Self { is_running: false }
    }

    fn run(&mut self) {
        self.is_running = true;
    }

    fn stop(&mut self) {
        self.is_running = false;
    }
}
impl EngineCorrectedN1 for TestEngine {
    fn corrected_n1(&self) -> Ratio {
        unimplemented!()
    }
}
impl EngineCorrectedN2 for TestEngine {
    fn corrected_n2(&self) -> Ratio {
        Ratio::new::<percent>(if self.is_running { 80. } else { 0. })
    }
}
impl EngineUncorrectedN2 for TestEngine {
    fn uncorrected_n2(&self) -> Ratio {
        unimplemented!()
    }
}
impl Engine for TestEngine {
    fn hydraulic_pump_output_speed(&self) -> AngularVelocity {
        unimplemented!()
    }

    fn oil_pressure_is_low(&self) -> bool {
        unimplemented!()
    }

    fn is_above_minimum_idle(&self) -> bool {
        unimplemented!()
    }

    fn net_thrust(&self) -> Mass {
        unimplemented!()
    }

    fn gearbox_speed(&self) -> AngularVelocity {
        AngularVelocity::new::<revolution_per_minute>(self.corrected_n2().get::<ratio>() * 16000.)
    }
}

struct TestApuOverhead {
    master_sw_pb_on: bool,
    start_pb_on: bool,
}
impl TestApuOverhead {
    fn new() -> Self {
        Self {
            master_sw_pb_on: false,
            start_pb_on: false,
        }
    }

    fn set_apu_master_sw_pb_on(&mut self) {
        self.master_sw_pb_on = true;
    }

    fn set_start_pb_on(&mut self) {
        self.start_pb_on = true;
    }
}
impl ApuMaster for TestApuOverhead {
    fn master_sw_is_on(&self) -> bool {
        self.master_sw_pb_on
    }
}
impl ApuStart for TestApuOverhead {
    fn start_is_on(&self) -> bool {
        self.start_pb_on
    }
}

struct TestHydraulicSystem {
    time_since_start: Duration,
    starting_or_started: bool,
    emergency_motor_speed: AngularVelocity,
}
impl TestHydraulicSystem {
    fn new() -> Self {
        Self {
            time_since_start: Duration::from_secs(0),
            starting_or_started: false,
            emergency_motor_speed: AngularVelocity::new::<revolution_per_minute>(0.),
        }
    }

    fn update(
        &mut self,
        is_rat_hydraulic_loop_pressurised: bool,
        is_emergency_elec: bool,
        rat_man_on_pressed: bool,
        context: &UpdateContext,
    ) {
        if (is_emergency_elec || rat_man_on_pressed) && is_rat_hydraulic_loop_pressurised {
            self.starting_or_started = true;
        }

        if self.starting_or_started {
            self.time_since_start += context.delta();
        }

        if self.time_since_start > Duration::from_secs(8) && is_rat_hydraulic_loop_pressurised {
            self.emergency_motor_speed = AngularVelocity::new::<revolution_per_minute>(12000.);
        } else {
            self.emergency_motor_speed = AngularVelocity::new::<revolution_per_minute>(0.);
        }
    }
}
impl EmergencyGeneratorControlUnit for TestHydraulicSystem {
    fn max_allowed_power(&self) -> Power {
        if self.emergency_motor_speed.get::<revolution_per_minute>() > 10000. {
            Power::new::<watt>(5000.)
        } else {
            Power::new::<watt>(0.)
        }
    }
    fn motor_speed(&self) -> AngularVelocity {
        self.emergency_motor_speed
    }
}

struct TestLandingGear {
    is_down: bool,
}
impl TestLandingGear {
    fn new(is_down: bool) -> Self {
        Self { is_down }
    }
}
impl LgciuWeightOnWheels for TestLandingGear {
    fn right_gear_compressed(&self, _: bool) -> bool {
        self.is_down
    }

    fn right_gear_extended(&self, _: bool) -> bool {
        !self.is_down
    }

    fn left_gear_compressed(&self, _: bool) -> bool {
        self.is_down
    }

    fn left_gear_extended(&self, _: bool) -> bool {
        !self.is_down
    }

    fn left_and_right_gear_compressed(&self, _: bool) -> bool {
        self.is_down
    }

    fn left_and_right_gear_extended(&self, _: bool) -> bool {
        !self.is_down
    }

    fn nose_gear_compressed(&self, _: bool) -> bool {
        self.is_down
    }

    fn nose_gear_extended(&self, _: bool) -> bool {
        !self.is_down
    }
}

struct TestAdirs {
    airspeed: Velocity,
}
impl TestAdirs {
    fn new(airspeed: Velocity) -> Self {
        Self { airspeed }
    }
}
impl AdirsDiscreteOutputs for TestAdirs {
    fn low_speed_warning_1(&self, adiru_number: usize) -> bool {
        assert_eq!(adiru_number, 1);
        self.airspeed.get::<knot>() > 100.
    }

    fn low_speed_warning_2(&self, adiru_number: usize) -> bool {
        assert_eq!(adiru_number, 1);
        self.airspeed.get::<knot>() > 50.
    }

    fn low_speed_warning_3(&self, _adiru_number: usize) -> bool {
        false
    }

    fn low_speed_warning_4(&self, _adiru_number: usize) -> bool {
        false
    }
}

pub struct A320ElectricalTestAircraft {
    engines: [TestEngine; 2],
    ext_pwr: ExternalPowerSource,
    elec: A320Electrical,
    overhead: A320ElectricalOverheadPanel,
    emergency_overhead: A320EmergencyElectricalOverheadPanel,
    apu: TestApu,
    apu_overhead: TestApuOverhead,
    engine_fire_push_buttons: TestEngineFirePushButtons,
    hydraulics: TestHydraulicSystem,
    force_run_emergency_gen: bool,
}
impl A320ElectricalTestAircraft {
    fn new(context: &mut InitContext) -> Self {
        Self {
            engines: [TestEngine::new(), TestEngine::new()],
            ext_pwr: ExternalPowerSource::new(context, 1),
            elec: A320Electrical::new(context),
            overhead: A320ElectricalOverheadPanel::new(context),
            emergency_overhead: A320EmergencyElectricalOverheadPanel::new(context),
            apu: TestApu::new(context),
            apu_overhead: TestApuOverhead::new(),
            engine_fire_push_buttons: TestEngineFirePushButtons::new(),
            hydraulics: TestHydraulicSystem::new(),
            force_run_emergency_gen: false,
        }
    }

    fn running_engine(&mut self, number: usize) {
        self.engines[number - 1].run();
    }

    fn stopped_engine(&mut self, number: usize) {
        self.engines[number - 1].stop();
    }

    fn running_apu(&mut self) {
        self.apu.set_available(true);
    }

    fn failed_apu_gen(&mut self) {
        self.apu.fail_generator();
    }

    fn set_apu_master_sw_pb_on(&mut self) {
        self.apu_overhead.set_apu_master_sw_pb_on();
    }

    fn set_apu_start_pb_on(&mut self) {
        self.apu_overhead.set_start_pb_on();
    }

    fn command_closing_of_start_contactors(&mut self) {
        self.apu.command_closing_of_start_contactors();
    }

    fn apu_start_motor_is_powered(&self) -> bool {
        self.apu.start_motor_is_powered()
    }

    fn empty_battery_1(&mut self) {
        self.elec.empty_battery_1();
    }

    fn empty_battery_2(&mut self) {
        self.elec.empty_battery_2();
    }

    fn running_emergency_generator(&mut self) {
        self.force_run_emergency_gen = true;
    }

    fn static_inverter_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
        electricity.input_of(self.elec.direct_current.static_inverter())
    }

    fn tr_1_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
        electricity.input_of(self.elec.tr_1())
    }

    fn tr_2_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
        electricity.input_of(self.elec.tr_2())
    }

    fn tr_ess_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
        electricity.input_of(self.elec.tr_ess())
    }

    fn battery_1_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
        electricity.input_of(self.elec.battery_1())
    }

    fn battery_2_input<'a>(&self, electricity: &'a Electricity) -> Ref<'a, Potential> {
        electricity.input_of(self.elec.battery_2())
    }

    fn release_engine_fire_push_button(&mut self, engine_number: usize) {
        self.engine_fire_push_buttons.release(engine_number);
    }
}
impl Aircraft for A320ElectricalTestAircraft {
    fn update_before_power_distribution(
        &mut self,
        context: &UpdateContext,
        electricity: &mut Electricity,
    ) {
        self.hydraulics.update(
            context.indicated_airspeed() > Velocity::new::<knot>(100.),
            self.elec.in_emergency_elec(),
            self.force_run_emergency_gen || self.emergency_overhead.is_pressed(),
            context,
        );

        self.elec.update(
            context,
            electricity,
            &self.ext_pwr,
            &self.overhead,
            &self.emergency_overhead,
            &mut self.apu,
            &self.apu_overhead,
            &self.engine_fire_push_buttons,
            [&self.engines[0], &self.engines[1]],
            &self.hydraulics,
            &TestLandingGear::new(context.is_on_ground()),
            &TestAdirs::new(context.indicated_airspeed()),
        );
        self.overhead
            .update_after_electrical(&self.elec, electricity, &self.apu);
        self.emergency_overhead
            .update_after_electrical(context, &self.elec);
    }
}
impl SimulationElement for A320ElectricalTestAircraft {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.ext_pwr.accept(visitor);
        self.elec.accept(visitor);
        self.overhead.accept(visitor);
        self.emergency_overhead.accept(visitor);
        self.apu.accept(visitor);

        visitor.visit(self);
    }
}

pub struct A320ElectricalTestBed {
    test_bed: SimulationTestBed<A320ElectricalTestAircraft>,
}
impl A320ElectricalTestBed {
    pub fn new() -> Self {
        Self {
            test_bed: SimulationTestBed::new(A320ElectricalTestAircraft::new),
        }
    }

    pub fn running_engine(mut self, number: usize) -> Self {
        self.command(|a| a.running_engine(number));

        self = self.without_triggering_emergency_elec(|x| {
            x.run_waiting_for(INTEGRATED_DRIVE_GENERATOR_STABILIZATION_TIME)
        });

        self
    }

    pub fn stopped_engine(mut self, number: usize) -> Self {
        self.command(|a| a.stopped_engine(number));
        self
    }

    pub fn running_engines(self) -> Self {
        self.running_engine(1).and().running_engine(2)
    }

    pub fn running_apu(mut self) -> Self {
        self.command(|a| a.running_apu());
        self
    }

    pub fn connected_external_power(mut self) -> Self {
        self.write_by_name("EXT_PWR_AVAIL:1", true);

        self.without_triggering_emergency_elec(|x| x.run())
    }

    pub fn empty_battery_1(mut self) -> Self {
        self.command(|a| a.empty_battery_1());
        self
    }

    pub fn empty_battery_2(mut self) -> Self {
        self.command(|a| a.empty_battery_2());
        self
    }

    pub fn airspeed(mut self, ias: Velocity) -> Self {
        self.set_indicated_airspeed(ias);
        self
    }

    pub fn on_the_ground(mut self) -> Self {
        self.set_pressure_altitude(Length::new::<foot>(0.));
        self.set_on_ground(true);
        self
    }

    pub fn run_for_start_contactor_test(self) -> Self {
        self.airspeed(Velocity::new::<knot>(0.))
            .on_the_ground()
            .apu_master_sw_pb_on()
            .and()
            .apu_start_pb_on()
            .run()
    }

    pub fn and(self) -> Self {
        self
    }

    pub fn then_continue_with(self) -> Self {
        self
    }

    pub fn failed_tr_1(mut self) -> Self {
        self.test_bed.fail(FailureType::TransformerRectifier(1));
        self
    }

    pub fn failed_tr_2(mut self) -> Self {
        self.test_bed.fail(FailureType::TransformerRectifier(2));
        self
    }

    pub fn failed_apu_gen(mut self) -> Self {
        self.command(|a| a.failed_apu_gen());
        self
    }

    pub fn running_emergency_generator(mut self) -> Self {
        self.command(|a| a.running_emergency_generator());
        self.run_waiting_for(Duration::from_secs(100))
    }

    pub fn gen_off(mut self, number: usize) -> Self {
        self.write_by_name(&format!("OVHD_ELEC_ENG_GEN_{}_PB_IS_ON", number), false);
        self
    }

    pub fn released_engine_fire_push_button(mut self, engine_number: usize) -> Self {
        self.command(|a| a.release_engine_fire_push_button(engine_number));
        self
    }

    pub fn gen_1_line_off(mut self) -> Self {
        self.write_by_name("OVHD_EMER_ELEC_GEN_1_LINE_PB_IS_ON", false);
        self
    }

    pub fn apu_gen_off(mut self) -> Self {
        self.write_by_name("OVHD_ELEC_APU_GEN_PB_IS_ON", false);
        self
    }

    pub fn ext_pwr_on(mut self) -> Self {
        self.write_by_name("OVHD_ELEC_EXT_PWR_PB_IS_ON", true);
        self
    }

    pub fn ext_pwr_off(mut self) -> Self {
        self.write_by_name("OVHD_ELEC_EXT_PWR_PB_IS_ON", false);
        self
    }

    pub fn ac_ess_feed_altn(mut self) -> Self {
        self.write_by_name("OVHD_ELEC_AC_ESS_FEED_PB_IS_NORMAL", false);
        self
    }

    pub fn bat_off(self, number: usize) -> Self {
        self.bat(number, false)
    }

    pub fn bat_auto(self, number: usize) -> Self {
        self.bat(number, true)
    }

    pub fn bat(mut self, number: usize, auto: bool) -> Self {
        self.write_by_name(&format!("OVHD_ELEC_BAT_{}_PB_IS_AUTO", number), auto);
        self
    }

    pub fn bus_tie(mut self, auto: bool) -> Self {
        self.write_by_name("OVHD_ELEC_BUS_TIE_PB_IS_AUTO", auto);
        self
    }

    pub fn bus_tie_auto(self) -> Self {
        self.bus_tie(true)
    }

    pub fn bus_tie_off(self) -> Self {
        self.bus_tie(false)
    }

    pub fn commercial_off(mut self) -> Self {
        self.write_by_name("OVHD_ELEC_COMMERCIAL_PB_IS_ON", false);
        self
    }

    pub fn galy_and_cab_off(mut self) -> Self {
        self.write_by_name("OVHD_ELEC_GALY_AND_CAB_PB_IS_AUTO", false);
        self
    }

    pub fn apu_master_sw_pb_on(mut self) -> Self {
        self.command(|a| a.set_apu_master_sw_pb_on());
        self
    }

    pub fn apu_start_pb_on(mut self) -> Self {
        self.command(|a| a.set_apu_start_pb_on());
        self
    }

    pub fn rat_and_emer_gen_man_on_pressed(mut self) -> Self {
        self.write_by_name("OVHD_EMER_ELEC_RAT_AND_EMER_GEN_IS_PRESSED", true);
        self
    }

    pub fn command_closing_of_start_contactors(mut self) -> Self {
        self.command(|a| a.command_closing_of_start_contactors());
        self
    }

    pub fn apu_start_contactors_closed(&mut self) -> bool {
        self.read_by_name("ELEC_CONTACTOR_10KA_AND_5KA_IS_CLOSED")
    }

    pub fn apu_start_motor_is_powered(&self) -> bool {
        self.query(|a| a.apu_start_motor_is_powered())
    }

    pub fn ac_bus_output(&'_ self, number: u8) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| {
            elec.potential_of(ElectricalBusType::AlternatingCurrent(number))
        })
    }

    pub fn ac_ess_bus_output(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| {
            elec.potential_of(ElectricalBusType::AlternatingCurrentEssential)
        })
    }

    pub fn ac_ess_shed_bus_output(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| {
            elec.potential_of(ElectricalBusType::AlternatingCurrentEssentialShed)
        })
    }

    pub fn ac_stat_inv_bus_output(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| {
            elec.potential_of(ElectricalBusType::AlternatingCurrentStaticInverter)
        })
    }

    pub fn ac_gnd_flt_service_bus_output(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| {
            elec.potential_of(ElectricalBusType::AlternatingCurrentGndFltService)
        })
    }

    pub fn static_inverter_input(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|a, elec| a.static_inverter_input(elec))
    }

    pub fn tr_1_input(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|a, elec| a.tr_1_input(elec))
    }

    pub fn tr_2_input(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|a, elec| a.tr_2_input(elec))
    }

    pub fn tr_ess_input(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|a, elec| a.tr_ess_input(elec))
    }

    pub fn battery_1_input(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|a, elec| a.battery_1_input(elec))
    }

    pub fn battery_2_input(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|a, elec| a.battery_2_input(elec))
    }

    pub fn dc_bus_output(&'_ self, number: u8) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| elec.potential_of(ElectricalBusType::DirectCurrent(number)))
    }

    pub fn dc_bat_bus_output(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| elec.potential_of(ElectricalBusType::DirectCurrentBattery))
    }

    pub fn dc_ess_bus_output(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| elec.potential_of(ElectricalBusType::DirectCurrentEssential))
    }

    pub fn dc_ess_shed_bus_output(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| {
            elec.potential_of(ElectricalBusType::DirectCurrentEssentialShed)
        })
    }

    pub fn hot_bus_output(&'_ self, number: u8) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| {
            elec.potential_of(ElectricalBusType::DirectCurrentHot(number))
        })
    }

    pub fn dc_gnd_flt_service_bus_output(&'_ self) -> Ref<'_, Potential> {
        self.query_elec_ref(|_, elec| {
            elec.potential_of(ElectricalBusType::DirectCurrentGndFltService)
        })
    }

    pub fn ac_ess_feed_has_fault(&mut self) -> bool {
        self.read_by_name("OVHD_ELEC_AC_ESS_FEED_PB_HAS_FAULT")
    }

    pub fn gen_has_fault(&mut self, number: usize) -> bool {
        self.read_by_name(&format!("OVHD_ELEC_ENG_GEN_{}_PB_HAS_FAULT", number))
    }

    pub fn apu_gen_has_fault(&mut self) -> bool {
        self.read_by_name("OVHD_ELEC_APU_GEN_PB_HAS_FAULT")
    }

    pub fn apu_gen_contactor_is_open(&mut self) -> bool {
        !ReadByName::<A320ElectricalTestBed, bool>::read_by_name(
            self,
            "ELEC_CONTACTOR_3XS_IS_CLOSED",
        )
    }

    pub fn ext_pwr_contactor_is_closed(&mut self) -> bool {
        self.read_by_name("ELEC_CONTACTOR_3XG_IS_CLOSED")
    }

    pub fn both_engine_gen_contactors_are_closed(&mut self) -> bool {
        self.read_by_name("ELEC_CONTACTOR_9XU1_IS_CLOSED")
            && self.read_by_name("ELEC_CONTACTOR_9XU2_IS_CLOSED")
    }

    pub fn rat_and_emer_gen_has_fault(&mut self) -> bool {
        self.read_by_name("OVHD_EMER_ELEC_RAT_AND_EMER_GEN_HAS_FAULT")
    }

    pub fn galley_is_shed(&mut self) -> bool {
        self.read_by_name("ELEC_GALLEY_IS_SHED")
    }

    pub fn both_ac_ess_feed_contactors_open(&mut self) -> bool {
        !ReadByName::<A320ElectricalTestBed, bool>::read_by_name(
            self,
            "ELEC_CONTACTOR_3XC1_IS_CLOSED",
        ) && !ReadByName::<A320ElectricalTestBed, bool>::read_by_name(
            self,
            "ELEC_CONTACTOR_3XC2_IS_CLOSED",
        )
    }

    pub fn dc_bus_2_tie_contactor_is_open(&mut self) -> bool {
        !ReadByName::<A320ElectricalTestBed, bool>::read_by_name(
            self,
            "ELEC_CONTACTOR_1PC2_IS_CLOSED",
        )
    }

    pub fn run(self) -> Self {
        self.run_waiting_for(Duration::from_secs(1))
    }

    pub fn run_waiting_for(mut self, delta: Duration) -> Self {
        self.run_with_delta(delta);

        // Sadly it's impossible for some electrical origins such as
        // the generators to know their output potential before a single
        // simulation tick has passed, as the output potential among other
        // things depends on electrical load which is only known near the
        // end of a tick. As the electrical system disallows e.g. an engine
        // generator contactor to close when its electrical parameters are
        // outside of normal parameters, we have to run a second tick before
        // the potential has flown through the system in the way we expected.
        self.run_with_delta(Duration::from_secs(0));

        self
    }

    /// Runs the simulation a single time with a delta of 1 second.
    /// This particular is useful for tests that want to verify behaviour
    /// which only occurs in a single tick and would be hidden by
    /// run or run_waiting_for, which executes two ticks.
    pub fn run_once(mut self) -> Self {
        self.run_with_delta(Duration::from_secs(1));

        self
    }

    /// Tests assume they start at altitude with high velocity.
    /// As power sources can take some time before they become available
    /// by wrapping the functions that enable such a power sources we ensure it cannot
    /// trigger the deployment of the RAT or start of EMER GEN.
    pub fn without_triggering_emergency_elec(mut self, mut func: impl FnMut(Self) -> Self) -> Self {
        let ias = self.indicated_airspeed();
        self.set_indicated_airspeed(Velocity::new::<knot>(0.));

        self = func(self);

        self.set_indicated_airspeed(ias);

        self
    }

    pub fn run_waiting_for_ac_ess_feed_transition(self) -> Self {
        self.run_waiting_for(A320AcEssFeedContactors::AC_ESS_FEED_TO_AC_BUS_2_DELAY_IN_SECONDS)
    }

    pub fn run_waiting_until_just_before_ac_ess_feed_transition(self) -> Self {
        self.run_waiting_for(
            A320AcEssFeedContactors::AC_ESS_FEED_TO_AC_BUS_2_DELAY_IN_SECONDS
                - Duration::from_millis(1),
        )
    }
}
impl Default for A320ElectricalTestBed {
    fn default() -> Self {
        Self::new()
    }
}
impl TestBed for A320ElectricalTestBed {
    type Aircraft = A320ElectricalTestAircraft;

    fn test_bed(&self) -> &SimulationTestBed<A320ElectricalTestAircraft> {
        &self.test_bed
    }

    fn test_bed_mut(&mut self) -> &mut SimulationTestBed<A320ElectricalTestAircraft> {
        &mut self.test_bed
    }
}
