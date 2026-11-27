mod alternating_current;
#[cfg(any(test, feature = "bench"))]
pub mod bench_support;
mod direct_current;
mod galley;

use self::{
    alternating_current::A320AlternatingCurrentElectrical,
    direct_current::A320DirectCurrentElectrical,
    galley::{MainGalley, SecondaryGalley},
};
pub(super) use direct_current::APU_START_MOTOR_BUS_TYPE;

use uom::si::{angular_velocity::revolution_per_minute, f64::*};

#[cfg(any(test, feature = "bench"))]
use systems::electrical::Battery;

use systems::{
    accept_iterable,
    electrical::{
        AlternatingCurrentElectricalSystem, BatteryPushButtons, Electricity, EmergencyElectrical,
        EmergencyGenerator, EngineGeneratorPushButtons, ExternalPowerSource, StaticInverter,
        TransformerRectifier,
    },
    engine::Engine,
    overhead::{
        AutoOffFaultPushButton, FaultDisconnectReleasePushButton, FaultIndication,
        MomentaryPushButton, NormalAltnFaultPushButton, OnOffAvailablePushButton,
        OnOffFaultPushButton,
    },
    shared::{
        AdirsDiscreteOutputs, ApuMaster, ApuStart, AuxiliaryPowerUnitElectrical,
        EmergencyElectricalRatPushButton, EmergencyElectricalState, EmergencyGeneratorControlUnit,
        EmergencyGeneratorPower, EngineFirePushButtons, LgciuWeightOnWheels,
    },
    simulation::{
        InitContext, SimulationElement, SimulationElementVisitor, SimulatorWriter, UpdateContext,
        VariableIdentifier, Write,
    },
};

pub(super) struct A320Electrical {
    galley_is_shed_id: VariableIdentifier,

    alternating_current: A320AlternatingCurrentElectrical,
    direct_current: A320DirectCurrentElectrical,
    main_galley: MainGalley,
    secondary_galley: SecondaryGalley,
    emergency_elec: EmergencyElectrical,
    emergency_gen: EmergencyGenerator,
}
impl A320Electrical {
    const MIN_EMERGENCY_GENERATOR_RPM_TO_ALLOW_CURRENT_SUPPLY: f64 = 10000.;
    pub fn new(context: &mut InitContext) -> A320Electrical {
        A320Electrical {
            galley_is_shed_id: context.get_identifier("ELEC_GALLEY_IS_SHED".to_owned()),
            alternating_current: A320AlternatingCurrentElectrical::new(context),
            direct_current: A320DirectCurrentElectrical::new(context),
            main_galley: MainGalley::new(),
            secondary_galley: SecondaryGalley::new(),
            emergency_elec: EmergencyElectrical::new(),
            emergency_gen: EmergencyGenerator::new(
                context,
                AngularVelocity::new::<revolution_per_minute>(
                    Self::MIN_EMERGENCY_GENERATOR_RPM_TO_ALLOW_CURRENT_SUPPLY,
                ),
            ),
        }
    }

    pub fn update(
        &mut self,
        context: &UpdateContext,
        electricity: &mut Electricity,
        ext_pwr: &ExternalPowerSource,
        overhead: &A320ElectricalOverheadPanel,
        emergency_overhead: &A320EmergencyElectricalOverheadPanel,
        apu: &mut impl AuxiliaryPowerUnitElectrical,
        apu_overhead: &(impl ApuMaster + ApuStart),
        engine_fire_push_buttons: &impl EngineFirePushButtons,
        engines: [&impl Engine; 2],
        gcu: &impl EmergencyGeneratorControlUnit,
        lgciu1: &impl LgciuWeightOnWheels,
        adirs: &impl AdirsDiscreteOutputs,
    ) {
        self.alternating_current.update_main_power_sources(
            context,
            electricity,
            ext_pwr,
            overhead,
            emergency_overhead,
            apu,
            engine_fire_push_buttons,
            engines,
        );

        self.emergency_elec
            .update(context, electricity, &self.alternating_current);

        self.emergency_gen.update(gcu);

        self.alternating_current.update(
            context,
            electricity,
            ext_pwr,
            overhead,
            &self.emergency_gen,
        );

        // Relay 7XB
        // TODO: should also be true (closed) when emer gen test P/B is pressed
        let spd_cond = adirs.low_speed_warning_2(1);

        // Elec using LGCIU1 L&R compressed (14A output  ASM 32_62_00)
        self.direct_current.update(
            context,
            electricity,
            overhead,
            &self.alternating_current,
            &self.emergency_elec,
            &self.emergency_gen,
            apu,
            apu_overhead,
            lgciu1,
            adirs,
            spd_cond,
        );

        self.alternating_current.update_after_direct_current(
            electricity,
            &self.emergency_gen,
            &self.direct_current,
            spd_cond,
        );

        self.alternating_current
            .update_auxiliary(electricity, emergency_overhead);

        self.main_galley
            .update(context, electricity, &self.alternating_current, overhead);
        self.secondary_galley
            .update(electricity, &self.alternating_current, overhead);

        self.debug_assert_invariants();
    }

    fn emergency_generator_contactor_is_closed(&self) -> bool {
        self.alternating_current
            .emergency_generator_contactor_is_closed()
    }

    fn ac_ess_bus_is_powered(&self, electricity: &Electricity) -> bool {
        self.alternating_current.ac_ess_bus_is_powered(electricity)
    }

    fn galley_is_shed(&self) -> bool {
        self.main_galley.is_shed() || self.secondary_galley.is_shed()
    }

    fn debug_assert_invariants(&self) {
        self.alternating_current.debug_assert_invariants();
        self.direct_current.debug_assert_invariants();
    }

    #[cfg(any(test, feature = "bench"))]
    fn tr_1(&self) -> &TransformerRectifier {
        self.alternating_current.tr_1()
    }

    #[cfg(any(test, feature = "bench"))]
    fn tr_2(&self) -> &TransformerRectifier {
        self.alternating_current.tr_2()
    }

    #[cfg(any(test, feature = "bench"))]
    fn tr_ess(&self) -> &TransformerRectifier {
        self.alternating_current.tr_ess()
    }

    #[cfg(any(test, feature = "bench"))]
    fn battery_1(&self) -> &Battery {
        self.direct_current.battery_1()
    }

    #[cfg(any(test, feature = "bench"))]
    fn battery_2(&self) -> &Battery {
        self.direct_current.battery_2()
    }

    #[cfg(any(test, feature = "bench"))]
    pub fn empty_battery_1(&mut self) {
        self.direct_current.empty_battery_1();
    }

    #[cfg(any(test, feature = "bench"))]
    pub fn empty_battery_2(&mut self) {
        self.direct_current.empty_battery_2();
    }

    pub fn gen_contactor_open(&self, number: usize) -> bool {
        self.alternating_current.gen_contactor_open(number)
    }

    fn apu_gen_contactor_open(&self) -> bool {
        self.alternating_current.apu_gen_contactor_open()
    }

    fn ext_pwr_contactor_closed(&self) -> bool {
        self.alternating_current.ext_pwr_contactor_closed()
    }

    fn both_engine_gen_contactors_closed(&self) -> bool {
        self.alternating_current.both_engine_gen_contactors_closed()
    }

    pub fn in_emergency_elec(&self) -> bool {
        self.emergency_elec.is_active()
    }
}
impl SimulationElement for A320Electrical {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.alternating_current.accept(visitor);
        self.direct_current.accept(visitor);
        self.emergency_gen.accept(visitor);

        visitor.visit(self);
    }

    fn write(&self, writer: &mut SimulatorWriter) {
        writer.write(&self.galley_is_shed_id, self.galley_is_shed())
    }
}
impl EmergencyElectricalState for A320Electrical {
    fn is_in_emergency_elec(&self) -> bool {
        self.in_emergency_elec()
    }
}
impl EmergencyGeneratorPower for A320Electrical {
    fn generated_power(&self) -> Power {
        self.emergency_gen.generated_power()
    }
}

trait A320DirectCurrentElectricalSystem {
    fn static_inverter(&self) -> &StaticInverter;
}

trait A320AlternatingCurrentElectricalSystem: AlternatingCurrentElectricalSystem {
    fn ac_bus_2_powered(&self, electricity: &Electricity) -> bool;
    fn tr_1_and_2_available(&self, electricity: &Electricity) -> bool;
    fn tr_1(&self) -> &TransformerRectifier;
    fn tr_2(&self) -> &TransformerRectifier;
    fn tr_ess(&self) -> &TransformerRectifier;
}

pub(super) struct A320ElectricalOverheadPanel {
    batteries: [AutoOffFaultPushButton; 2],
    idgs: [FaultDisconnectReleasePushButton; 2],
    generators: [OnOffFaultPushButton; 2],
    apu_gen: OnOffFaultPushButton,
    bus_tie: AutoOffFaultPushButton,
    ac_ess_feed: NormalAltnFaultPushButton,
    galy_and_cab: AutoOffFaultPushButton,
    ext_pwr: OnOffAvailablePushButton,
    commercial: OnOffFaultPushButton,
}
impl A320ElectricalOverheadPanel {
    pub fn new(context: &mut InitContext) -> A320ElectricalOverheadPanel {
        A320ElectricalOverheadPanel {
            batteries: [
                AutoOffFaultPushButton::new_auto(context, "ELEC_BAT_1"),
                AutoOffFaultPushButton::new_auto(context, "ELEC_BAT_2"),
            ],
            idgs: [
                FaultDisconnectReleasePushButton::new_in(context, "ELEC_IDG_1"),
                FaultDisconnectReleasePushButton::new_in(context, "ELEC_IDG_2"),
            ],
            generators: [
                OnOffFaultPushButton::new_on(context, "ELEC_ENG_GEN_1"),
                OnOffFaultPushButton::new_on(context, "ELEC_ENG_GEN_2"),
            ],
            apu_gen: OnOffFaultPushButton::new_on(context, "ELEC_APU_GEN"),
            bus_tie: AutoOffFaultPushButton::new_auto(context, "ELEC_BUS_TIE"),
            ac_ess_feed: NormalAltnFaultPushButton::new_normal(context, "ELEC_AC_ESS_FEED"),
            galy_and_cab: AutoOffFaultPushButton::new_auto(context, "ELEC_GALY_AND_CAB"),
            ext_pwr: OnOffAvailablePushButton::new_off(context, "ELEC_EXT_PWR"),
            commercial: OnOffFaultPushButton::new_on(context, "ELEC_COMMERCIAL"),
        }
    }

    pub fn update_after_electrical(
        &mut self,
        electrical: &A320Electrical,
        electricity: &Electricity,
        apu: &impl AuxiliaryPowerUnitElectrical,
    ) {
        self.ac_ess_feed
            .set_fault(!electrical.ac_ess_bus_is_powered(electricity));

        self.generators
            .iter_mut()
            .enumerate()
            .for_each(|(index, gen)| {
                gen.set_fault(electrical.gen_contactor_open(index + 1) && gen.is_on());
            });

        self.apu_gen.set_fault(
            self.apu_gen.is_on()
                && apu.is_available() // TODO: Check from GCU
                && electrical.apu_gen_contactor_open()
                && !electrical.ext_pwr_contactor_closed()
                && !electrical.both_engine_gen_contactors_closed(),
        );
    }

    fn generator_is_on(&self, number: usize) -> bool {
        self.generators[number - 1].is_on()
    }

    pub fn external_power_is_available(&self) -> bool {
        self.ext_pwr.is_available()
    }

    pub fn external_power_is_on(&self) -> bool {
        self.ext_pwr.is_on()
    }

    pub fn apu_generator_is_on(&self) -> bool {
        self.apu_gen.is_on()
    }

    fn bus_tie_is_auto(&self) -> bool {
        self.bus_tie.is_auto()
    }

    fn ac_ess_feed_is_normal(&self) -> bool {
        self.ac_ess_feed.is_normal()
    }

    fn ac_ess_feed_is_altn(&self) -> bool {
        self.ac_ess_feed.is_altn()
    }

    fn commercial_is_off(&self) -> bool {
        self.commercial.is_off()
    }

    fn galy_and_cab_is_off(&self) -> bool {
        self.galy_and_cab.is_off()
    }
}
impl EngineGeneratorPushButtons for A320ElectricalOverheadPanel {
    fn engine_gen_push_button_is_on(&self, number: usize) -> bool {
        self.generators[number - 1].is_on()
    }

    fn idg_push_button_is_released(&self, number: usize) -> bool {
        self.idgs[number - 1].is_released()
    }
}
impl BatteryPushButtons for A320ElectricalOverheadPanel {
    fn bat_is_auto(&self, number: usize) -> bool {
        self.batteries[number - 1].is_auto()
    }
}
impl SimulationElement for A320ElectricalOverheadPanel {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        accept_iterable!(self.batteries, visitor);
        accept_iterable!(self.idgs, visitor);
        accept_iterable!(self.generators, visitor);

        self.apu_gen.accept(visitor);
        self.bus_tie.accept(visitor);
        self.ac_ess_feed.accept(visitor);
        self.galy_and_cab.accept(visitor);
        self.ext_pwr.accept(visitor);
        self.commercial.accept(visitor);

        visitor.visit(self);
    }
}

pub(super) struct A320EmergencyElectricalOverheadPanel {
    // The GEN 1 line fault represents the SMOKE light illumination state.
    gen_1_line: OnOffFaultPushButton,
    rat_and_emergency_gen_fault: FaultIndication,
    rat_and_emer_gen_man_on: MomentaryPushButton,
}
impl A320EmergencyElectricalOverheadPanel {
    pub fn new(context: &mut InitContext) -> Self {
        Self {
            gen_1_line: OnOffFaultPushButton::new_on(context, "EMER_ELEC_GEN_1_LINE"),
            rat_and_emergency_gen_fault: FaultIndication::new(
                context,
                "EMER_ELEC_RAT_AND_EMER_GEN",
            ),
            rat_and_emer_gen_man_on: MomentaryPushButton::new(
                context,
                "EMER_ELEC_RAT_AND_EMER_GEN",
            ),
        }
    }

    pub fn update_after_electrical(
        &mut self,
        context: &UpdateContext,
        electrical: &A320Electrical,
    ) {
        self.rat_and_emergency_gen_fault.set_fault(
            electrical.in_emergency_elec()
                && !electrical.emergency_generator_contactor_is_closed()
                && !context.is_on_ground(),
        );
    }

    fn generator_1_line_is_on(&self) -> bool {
        self.gen_1_line.is_on()
    }

    fn rat_and_emer_gen_man_on(&self) -> bool {
        self.rat_and_emer_gen_man_on.is_pressed()
    }
}
impl SimulationElement for A320EmergencyElectricalOverheadPanel {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.gen_1_line.accept(visitor);
        self.rat_and_emergency_gen_fault.accept(visitor);
        self.rat_and_emer_gen_man_on.accept(visitor);

        visitor.visit(self);
    }
}
impl EmergencyElectricalRatPushButton for A320EmergencyElectricalOverheadPanel {
    fn is_pressed(&self) -> bool {
        self.rat_and_emer_gen_man_on()
    }
}
#[cfg(test)]
mod a320_electrical {
    use super::*;
    use systems::simulation::test::{ElementCtorFn, SimulationTestBed, TestBed};

    #[test]
    fn writes_its_state() {
        let mut test_bed = SimulationTestBed::from(ElementCtorFn(A320Electrical::new));

        test_bed.run();

        assert!(test_bed.contains_variable_with_name("ELEC_GALLEY_IS_SHED"));
    }
}

#[cfg(test)]
mod a320_electrical_circuit_tests {
    use super::bench_support::*;
    use super::*;
    use rstest::rstest;
    use std::time::Duration;
    use systems::shared::PotentialOrigin;
    use uom::si::velocity::knot;

    #[test]
    fn everything_off_batteries_empty() {
        let test_bed = test_bed_with()
            .bat_off(1)
            .empty_battery_1()
            .bat_off(2)
            .empty_battery_2()
            .and()
            .airspeed(Velocity::new::<knot>(0.))
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed.ac_ess_bus_output().is_unpowered());
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed.dc_ess_bus_output().is_unpowered());
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed.hot_bus_output(1).is_unpowered());
        assert!(test_bed.hot_bus_output(2).is_unpowered());
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    #[test]
    fn everything_off() {
        let test_bed = test_bed_with()
            .bat_off(1)
            .bat_off(2)
            .and()
            .airspeed(Velocity::new::<knot>(0.))
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed.ac_ess_bus_output().is_unpowered());
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed.dc_ess_bus_output().is_unpowered());
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_norm_conf() {
        let test_bed = test_bed_with().running_engines().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_only_gen_1_available() {
        let test_bed = test_bed_with().running_engine(1).run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_only_gen_2_available() {
        let test_bed = test_bed_with().running_engine(2).run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_only_apu_gen_available() {
        let test_bed = test_bed_with().running_apu().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// Derived from A320 manual electrical distribution table
    /// (doesn't list external power, but we'll assume it's the same as other generators).
    #[test]
    fn distribution_table_only_external_power_available_and_on() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .airspeed(Velocity::new::<knot>(0.))
            .on_the_ground()
            .and()
            .ext_pwr_on()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed.tr_1_input().is_single(PotentialOrigin::External));
        assert!(test_bed.tr_2_input().is_single(PotentialOrigin::External));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_emergency_config_before_emergency_gen_available() {
        let test_bed = test_bed().run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .static_inverter_input()
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .ac_stat_inv_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_emergency_config_after_emergency_gen_available() {
        let test_bed = test_bed_with().running_emergency_generator().run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_tr_1_fault() {
        let test_bed = test_bed_with().running_engines().and().failed_tr_1().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .tr_2_input()
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_tr_2_fault() {
        let test_bed = test_bed_with().running_engines().and().failed_tr_2().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .dc_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_bat_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(1)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_tr_1_and_2_fault() {
        let test_bed = test_bed_with()
            .running_engines()
            .failed_tr_1()
            .and()
            .failed_tr_2()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed
            .tr_1_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_on_ground_bat_and_emergency_gen_only_speed_above_100_knots() {
        let test_bed = test_bed_with()
            .running_emergency_generator()
            .airspeed(Velocity::new::<knot>(101.))
            .and()
            .on_the_ground()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed
            .ac_ess_shed_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_stat_inv_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed
            .tr_ess_input()
            .is_single(PotentialOrigin::EmergencyGenerator));
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed.dc_bat_bus_output().is_unpowered());
        assert!(test_bed
            .dc_ess_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .dc_ess_shed_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(3)));
        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_on_ground_bat_only_rat_stall_or_speed_between_50_to_100_knots() {
        let test_bed = test_bed_with()
            .running_emergency_generator()
            .airspeed(Velocity::new::<knot>(51.0))
            .and()
            .on_the_ground()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed
            .static_inverter_input()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .ac_stat_inv_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed
            .dc_bat_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    /// # Source
    /// A320 manual electrical distribution table
    #[test]
    fn distribution_table_on_ground_bat_only_speed_less_than_50_knots() {
        let test_bed = test_bed_with()
            .running_emergency_generator()
            .airspeed(Velocity::new::<knot>(49.9))
            .and()
            .on_the_ground()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(
            test_bed.ac_ess_bus_output().is_unpowered(),
            "AC ESS BUS shouldn't be powered below 50 knots when on batteries only."
        );
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .static_inverter_input()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .ac_stat_inv_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed.ac_gnd_flt_service_bus_output().is_unpowered());
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_unpowered());
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed
            .dc_bat_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_gnd_flt_service_bus_output().is_unpowered());
    }

    #[test]
    fn distribution_table_only_external_power_available_and_off() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .airspeed(Velocity::new::<knot>(0.))
            .and()
            .on_the_ground()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
        assert!(test_bed.ac_ess_bus_output().is_unpowered());
        assert!(test_bed.ac_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .static_inverter_input()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .ac_stat_inv_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed.tr_1_input().is_unpowered());
        assert!(test_bed.tr_2_input().is_single(PotentialOrigin::External));
        assert!(test_bed.tr_ess_input().is_unpowered());
        assert!(test_bed.dc_bus_output(1).is_unpowered());
        assert!(test_bed.dc_bus_output(2).is_unpowered());
        assert!(test_bed
            .dc_bat_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_ess_bus_output()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed.dc_ess_shed_bus_output().is_unpowered());
        assert!(test_bed
            .hot_bus_output(1)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .hot_bus_output(2)
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    #[rstest]
    #[case(1, 2)]
    #[case(2, 1)]
    fn when_single_engine_and_apu_running_apu_powers_other_ac_bus(
        #[case] engine: usize,
        #[case] ac_bus: u8,
    ) {
        let test_bed = test_bed_with()
            .running_engine(engine)
            .and()
            .running_apu()
            .run();

        assert!(test_bed
            .ac_bus_output(ac_bus)
            .is_single(PotentialOrigin::ApuGenerator(1)));
    }

    #[test]
    fn when_only_apu_running_apu_powers_ac_bus_1_and_2() {
        let test_bed = test_bed_with().running_apu().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::ApuGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::ApuGenerator(1)));
    }

    #[rstest]
    #[case(1, 2)]
    #[case(2, 1)]
    fn when_single_engine_running_and_ext_pwr_connected_ext_pwr_powers_other_ac_bus(
        #[case] engine: usize,
        #[case] ac_bus: u8,
    ) {
        let test_bed = test_bed_with()
            .running_engine(engine)
            .connected_external_power()
            .and()
            .ext_pwr_on()
            .run();

        assert!(test_bed
            .ac_bus_output(ac_bus)
            .is_single(PotentialOrigin::External));
    }

    #[test]
    fn when_only_external_power_connected_ext_pwr_powers_ac_bus_1_and_2() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .and()
            .ext_pwr_on()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::External));
    }

    #[test]
    fn when_external_power_connected_and_apu_running_external_power_has_priority() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .ext_pwr_on()
            .and()
            .running_apu()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::External));
    }

    #[test]
    fn when_both_engines_running_and_external_power_connected_engines_power_ac_buses() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .connected_external_power()
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    #[test]
    fn when_both_engines_running_and_apu_running_engines_power_ac_buses() {
        let test_bed = test_bed_with().running_engines().and().running_apu().run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(1)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    #[test]
    fn ac_bus_1_powers_ac_ess_bus_whenever_it_is_powered() {
        let test_bed = test_bed_with().running_engines().run();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
    }

    #[test]
    fn when_ac_bus_1_becomes_unpowered_but_ac_bus_2_powered_nothing_powers_ac_ess_bus_for_a_while()
    {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .bus_tie_off()
            .run_waiting_until_just_before_ac_ess_feed_transition();

        assert!(test_bed.static_inverter_input().is_unpowered());
        assert!(test_bed.ac_ess_bus_output().is_unpowered());
    }

    #[test]
    fn when_ac_bus_1_becomes_unpowered_but_ac_bus_2_powered_nothing_powers_dc_ess_bus_for_a_while()
    {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .bus_tie_off()
            .run_waiting_until_just_before_ac_ess_feed_transition();

        assert!(test_bed.dc_ess_bus_output().is_unpowered());
    }

    #[test]
    fn bat_only_low_airspeed_when_a_single_battery_contactor_closed_static_inverter_has_no_input() {
        let test_bed = test_bed_with()
            .bat_auto(1)
            .bat_off(2)
            .and()
            .airspeed(Velocity::new::<knot>(49.))
            .run_waiting_for(Duration::from_secs(1_000));

        assert!(test_bed.static_inverter_input().is_unpowered());
    }

    #[test]
    fn bat_only_low_airspeed_static_inverter_has_input() {
        let test_bed = test_bed_with()
            .bat_auto(1)
            .bat_auto(2)
            .on_the_ground()
            .and()
            .airspeed(Velocity::new::<knot>(49.))
            .run_waiting_for(Duration::from_secs(1_000));

        assert!(test_bed
            .static_inverter_input()
            .is_pair(PotentialOrigin::Battery(1), PotentialOrigin::Battery(2)));
    }

    #[test]
    fn when_airspeed_above_50_and_ac_bus_1_and_2_unpowered_and_emergency_gen_off_static_inverter_powers_ac_ess_bus(
    ) {
        let test_bed = test_bed_with()
            .airspeed(Velocity::new::<knot>(51.))
            .run_waiting_for(Duration::from_secs(1_000));

        assert!(test_bed
            .static_inverter_input()
            .is_single(PotentialOrigin::Battery(1)));
        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::StaticInverter));
    }

    /// # Source
    /// Discord (komp#1821):
    /// > The fault light will extinguish after 3 seconds. That's the time delay before automatic switching is activated in case of AC BUS 1 loss.
    #[test]
    fn with_ac_bus_1_being_unpowered_after_a_delay_ac_bus_2_powers_ac_ess_bus() {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .bus_tie_off()
            .run_waiting_for_ac_ess_feed_transition();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    /// # Source
    /// Discord (komp#1821):
    /// > When AC BUS 1 is available again, it will switch back automatically without delay, unless the AC ESS FEED button is on ALTN.
    #[test]
    fn ac_bus_1_powers_ac_ess_bus_immediately_when_ac_bus_1_becomes_powered_after_ac_bus_2_was_powering_ac_ess_bus(
    ) {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .bus_tie_off()
            .run_waiting_for_ac_ess_feed_transition()
            .then_continue_with()
            .running_engine(1)
            .and()
            .bus_tie_auto()
            .run();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(1)));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn single_engine_with_generator_off_leaves_ac_buses_unpowered(#[case] engine: usize) {
        let test_bed = test_bed_with()
            .running_engine(engine)
            .and()
            .gen_off(engine)
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[rstest]
    #[case(1, 2)]
    #[case(2, 1)]
    fn when_generator_off_and_both_engines_running_the_other_generator_powers_ac_buses(
        #[case] generator_off: usize,
        #[case] supplying_generator: usize,
    ) {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .gen_off(generator_off)
            .run();

        assert!(test_bed
            .ac_bus_output(1)
            .is_single(PotentialOrigin::EngineGenerator(supplying_generator)));
        assert!(test_bed
            .ac_bus_output(2)
            .is_single(PotentialOrigin::EngineGenerator(supplying_generator)));
    }

    #[test]
    fn when_ac_ess_feed_push_button_altn_engine_gen_2_powers_ac_ess_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .ac_ess_feed_altn()
            .run();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    #[test]
    fn when_only_apu_running_but_apu_gen_push_button_off_nothing_powers_ac_bus_1_and_2() {
        let test_bed = test_bed_with().running_apu().and().apu_gen_off().run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_only_external_power_connected_but_ext_pwr_push_button_off_nothing_powers_ac_bus_1_and_2(
    ) {
        let test_bed = test_bed_with()
            .connected_external_power()
            .and()
            .ext_pwr_off()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_ac_bus_1_and_ac_bus_2_are_lost_neither_ac_ess_feed_contactor_is_closed() {
        let mut test_bed = test_bed_with().run();

        assert!(test_bed.both_ac_ess_feed_contactors_open());
    }

    #[test]
    fn when_battery_1_full_it_is_not_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with().running_engines().run();

        assert!(test_bed
            .battery_1_input()
            .is_single(PotentialOrigin::Battery(1)));
    }

    #[test]
    fn when_battery_1_not_full_it_is_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .empty_battery_1()
            .run();

        assert!(test_bed.battery_1_input().is_powered());
    }

    #[test]
    fn when_battery_1_not_full_and_button_off_it_is_not_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .empty_battery_1()
            .and()
            .bat_off(1)
            .run();

        assert!(test_bed.battery_1_input().is_unpowered())
    }

    #[test]
    fn when_battery_1_has_charge_powers_hot_bus_1() {
        let test_bed = test_bed().run();

        assert!(test_bed.hot_bus_output(1).is_powered());
    }

    #[test]
    fn when_battery_1_is_empty_and_dc_bat_bus_unpowered_hot_bus_1_unpowered() {
        let test_bed = test_bed_with().empty_battery_1().run();

        assert!(test_bed.hot_bus_output(1).is_unpowered());
    }

    #[test]
    fn when_battery_1_is_empty_and_dc_bat_bus_powered_hot_bus_1_powered() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .empty_battery_1()
            .run();

        assert!(test_bed
            .hot_bus_output(1)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
    }

    #[test]
    fn when_battery_2_full_it_is_not_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with().running_engines().run();

        assert!(test_bed
            .battery_2_input()
            .is_single(PotentialOrigin::Battery(2)));
    }

    #[test]
    fn when_battery_2_not_full_it_is_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .empty_battery_2()
            .run();

        assert!(test_bed.battery_2_input().is_powered());
    }

    #[test]
    fn when_battery_2_not_full_and_button_off_it_is_not_powered_by_dc_bat_bus() {
        let test_bed = test_bed_with()
            .running_engines()
            .empty_battery_2()
            .and()
            .bat_off(2)
            .run();

        assert!(test_bed.battery_2_input().is_unpowered())
    }

    #[test]
    fn when_battery_2_has_charge_powers_hot_bus_2() {
        let test_bed = test_bed().run();

        assert!(test_bed.hot_bus_output(2).is_powered());
    }

    #[test]
    fn when_battery_2_is_empty_and_dc_bat_bus_unpowered_hot_bus_2_unpowered() {
        let test_bed = test_bed_with().empty_battery_2().run();

        assert!(test_bed.hot_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_battery_2_is_empty_and_dc_bat_bus_powered_hot_bus_2_powered() {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .empty_battery_2()
            .run();

        assert!(test_bed
            .hot_bus_output(2)
            .is_single(PotentialOrigin::TransformerRectifier(1)));
    }

    #[rstest]
    #[case(1, 2)]
    #[case(2, 1)]
    fn when_bus_tie_off_engine_does_not_power_other_ac_bus(
        #[case] engine: usize,
        #[case] ac_bus: u8,
    ) {
        let test_bed = test_bed_with()
            .running_engine(engine)
            .and()
            .bus_tie_off()
            .run();

        assert!(test_bed.ac_bus_output(ac_bus).is_unpowered());
    }

    #[test]
    fn when_bus_tie_off_apu_does_not_power_ac_buses() {
        let test_bed = test_bed_with().running_apu().and().bus_tie_off().run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_bus_tie_off_external_power_does_not_power_ac_buses() {
        let test_bed = test_bed_with()
            .connected_external_power()
            .and()
            .bus_tie_off()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_dc_bus_1_and_dc_bus_2_unpowered_dc_bus_2_to_dc_bat_remains_open() {
        let mut test_bed = test_bed().run();

        assert!(test_bed.dc_bus_2_tie_contactor_is_open());
    }

    #[test]
    fn when_ac_ess_bus_powered_ac_ess_feed_does_not_have_fault() {
        let mut test_bed = test_bed_with().running_engines().run();

        assert!(!test_bed.ac_ess_feed_has_fault());
    }

    #[test]
    fn when_ac_ess_bus_is_unpowered_ac_ess_feed_has_fault() {
        let mut test_bed = test_bed_with().airspeed(Velocity::new::<knot>(0.)).run();

        assert!(test_bed.ac_ess_feed_has_fault());
    }

    #[test]
    fn when_single_engine_and_apu_galley_is_not_shed() {
        let mut test_bed = test_bed_with().running_engine(1).and().running_apu().run();

        assert!(!test_bed.galley_is_shed());
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_single_engine_gen_galley_is_shed(#[case] engine_number: usize) {
        let mut test_bed = test_bed_with().running_engine(engine_number).run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_on_ground_and_apu_gen_only_galley_is_not_shed() {
        let mut test_bed = test_bed_with().running_apu().and().on_the_ground().run();

        assert!(!test_bed.galley_is_shed());
    }

    #[test]
    fn when_single_engine_gen_with_bus_tie_off_but_apu_running_galley_is_shed() {
        let mut test_bed = test_bed_with()
            .running_engine(1)
            .running_apu()
            .and()
            .bus_tie_off()
            .run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_single_engine_gen_with_bus_tie_off_and_ext_pwr_on_galley_is_shed() {
        let mut test_bed = test_bed_with()
            .running_engine(1)
            .connected_external_power()
            .ext_pwr_on()
            .and()
            .bus_tie_off()
            .run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_on_ground_and_ext_pwr_only_galley_is_not_shed() {
        let mut test_bed = test_bed_with()
            .connected_external_power()
            .ext_pwr_on()
            .and()
            .on_the_ground()
            .run();

        assert!(!test_bed.galley_is_shed());
    }

    #[test]
    fn when_in_flight_and_apu_gen_only_galley_is_shed() {
        let mut test_bed = test_bed_with().running_apu().run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_in_flight_and_emer_gen_only_galley_is_shed() {
        let mut test_bed = test_bed_with().running_emergency_generator().run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_commercial_pb_off_galley_is_shed() {
        let mut test_bed = test_bed_with()
            .running_engines()
            .and()
            .commercial_off()
            .run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    fn when_galy_and_cab_pb_off_galley_is_shed() {
        let mut test_bed = test_bed_with()
            .running_engines()
            .and()
            .galy_and_cab_off()
            .run();

        assert!(test_bed.galley_is_shed());
    }

    #[test]
    #[ignore = "Generator overloading is not yet supported."]
    fn when_aircraft_on_the_ground_and_apu_gen_is_overloaded_galley_is_shed() {}

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_gen_contactor_open_gen_push_button_has_fault(#[case] gen_number: usize) {
        let mut test_bed = test_bed_with().running_apu().run();

        assert!(test_bed.gen_has_fault(gen_number));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_gen_contactor_open_and_gen_push_button_off_does_not_have_fault(
        #[case] gen_number: usize,
    ) {
        let mut test_bed = test_bed_with()
            .running_apu()
            .and()
            .gen_off(gen_number)
            .run();

        assert!(!test_bed.gen_has_fault(gen_number));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_gen_contactor_closed_gen_push_button_does_not_have_fault(#[case] gen_number: usize) {
        let mut test_bed = test_bed_with().running_engine(gen_number).run();

        assert!(!test_bed.gen_has_fault(gen_number));
    }

    #[test]
    fn when_apu_generator_faulted_apu_gen_line_contactor_opens_and_push_button_has_fault() {
        let mut test_bed = test_bed_with().running_apu().and().failed_apu_gen().run();

        assert!(test_bed.apu_gen_contactor_is_open());
        assert!(test_bed.apu_gen_has_fault());
    }

    #[test]
    fn when_apu_not_yet_available_apu_gen_push_button_does_not_have_fault() {
        let mut test_bed = test_bed_with().run();

        assert!(test_bed.apu_gen_contactor_is_open());
        assert!(!test_bed.apu_gen_has_fault());
    }

    #[test]
    fn when_apu_generator_faulted_and_apu_gen_push_button_off_does_not_have_fault() {
        let mut test_bed = test_bed_with()
            .running_apu()
            .and()
            .failed_apu_gen()
            .and()
            .apu_gen_off()
            .run();

        assert!(!test_bed.apu_gen_has_fault());
    }

    #[test]
    fn when_apu_generator_faulted_and_external_power_takes_over_apu_gen_push_button_does_not_have_fault(
    ) {
        let mut test_bed = test_bed_with()
            .connected_external_power()
            .and()
            .ext_pwr_on()
            .and()
            .running_apu()
            .and()
            .failed_apu_gen()
            .run();

        assert!(test_bed.apu_gen_contactor_is_open());
        assert!(test_bed.ext_pwr_contactor_is_closed());
        assert!(!test_bed.apu_gen_has_fault());
    }

    #[test]
    fn when_apu_generator_faulted_and_both_engine_generators_take_over_apu_gen_push_button_does_not_have_fault(
    ) {
        let mut test_bed = test_bed_with()
            .running_engines()
            .and()
            .running_apu()
            .and()
            .failed_apu_gen()
            .run();

        assert!(test_bed.apu_gen_contactor_is_open());
        assert!(test_bed.both_engine_gen_contactors_are_closed());
        assert!(!test_bed.apu_gen_has_fault());
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_apu_start_with_battery_off_start_contactors_remain_open_and_motor_unpowered(
        #[case] battery_number: usize,
    ) {
        let mut test_bed = test_bed_with()
            .bat_off(battery_number)
            .command_closing_of_start_contactors()
            .and()
            .run_for_start_contactor_test();

        assert!(!test_bed.apu_start_contactors_closed());
        assert!(!test_bed.apu_start_motor_is_powered());
    }

    #[test]
    fn when_apu_start_with_both_batteries_auto_and_closing_commanded_start_contactors_close_and_motor_is_powered(
    ) {
        let mut test_bed = test_bed_with()
            .bat_auto(1)
            .bat_auto(2)
            .command_closing_of_start_contactors()
            .and()
            .run_for_start_contactor_test();

        assert!(test_bed.apu_start_contactors_closed());
        assert!(test_bed.apu_start_motor_is_powered());
    }

    #[test]
    fn when_apu_start_with_both_batteries_auto_and_closing_not_commanded_start_contactors_remain_open_and_motor_unpowered(
    ) {
        let mut test_bed = test_bed_with()
            .bat_auto(1)
            .bat_auto(2)
            .and()
            .run_for_start_contactor_test();

        assert!(!test_bed.apu_start_contactors_closed());
        assert!(!test_bed.apu_start_motor_is_powered());
    }

    #[test]
    fn transitions_between_gen_1_and_gen_2_without_interruption() {
        // The current implementation shouldn't include power interruptions.
        let mut test_bed = test_bed_with()
            .running_engine(1)
            .and()
            .running_engine(2)
            .run();
        assert!(
            test_bed.ac_bus_output(1).is_powered(),
            "Precondition: the test assumes the AC 1 bus is powered at this point."
        );

        test_bed = test_bed.then_continue_with().stopped_engine(1).run_once();

        assert!(test_bed.ac_bus_output(1).is_powered());
    }

    #[test]
    fn when_ac_2_bus_is_powered_it_has_priority_over_ext_pwr_gnd_flt_circuit() {
        let test_bed = test_bed_with()
            .running_engine(2)
            .and()
            .connected_external_power()
            .run();

        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::EngineGenerator(2)));
    }

    #[test]
    fn when_ac_2_bus_is_unpowered_and_ac_1_is_powered_ext_pwr_powers_gnd_flt_buses() {
        let test_bed = test_bed_with()
            .running_engine(1)
            .bus_tie_off()
            .and()
            .connected_external_power()
            .run();

        assert!(test_bed
            .ac_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::External));
        assert!(test_bed
            .dc_gnd_flt_service_bus_output()
            .is_single(PotentialOrigin::TransformerRectifier(2)));
    }

    #[test]
    fn when_gen_1_line_off_and_only_engine_1_running_nothing_powers_ac_buses() {
        let test_bed = test_bed_with()
            .running_engine(1)
            .and()
            .gen_1_line_off()
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[test]
    fn when_gen_1_contactor_open_due_to_gen_1_line_being_off_gen_1_push_button_has_fault() {
        let mut test_bed = test_bed_with()
            .running_engine(1)
            .and()
            .gen_1_line_off()
            .run();

        assert!(test_bed.gen_has_fault(1));
    }

    #[test]
    fn when_emergency_generator_not_supplying_while_ac_1_and_2_unavailable_in_flight_rat_and_emer_gen_has_fault(
    ) {
        let mut test_bed = test_bed_with()
            .running_engines()
            .gen_off(1)
            .and()
            .gen_off(2)
            .run();

        assert!(test_bed.rat_and_emer_gen_has_fault());
    }

    #[test]
    fn when_emergency_generator_not_supplying_while_ac_1_and_2_unavailable_during_takeoff_rat_and_emer_gen_does_not_have_fault(
    ) {
        let mut test_bed = test_bed_with()
            .running_engines()
            .on_the_ground()
            .gen_off(1)
            .and()
            .gen_off(2)
            .run();

        assert!(!test_bed.rat_and_emer_gen_has_fault());
    }

    #[test]
    fn when_emergency_generator_not_supplying_while_ac_1_and_2_unavailable_during_low_speed_flight_rat_and_emer_gen_does_not_have_fault(
    ) {
        let mut test_bed = test_bed_with()
            .running_engines()
            .gen_off(1)
            .gen_off(2)
            .and()
            .airspeed(Velocity::new::<knot>(99.))
            .run();

        assert!(!test_bed.rat_and_emer_gen_has_fault());
    }

    #[test]
    fn when_rat_and_emer_gen_man_on_push_button_is_pressed_at_an_earlier_time_in_case_of_ac_1_and_2_unavailable_emergency_generator_provides_power_immediately(
    ) {
        let test_bed = test_bed_with()
            .running_engines()
            .and()
            .rat_and_emer_gen_man_on_pressed()
            .run_waiting_for(Duration::from_secs(100))
            .then_continue_with()
            .gen_off(1)
            .and()
            .gen_off(2)
            .run();

        assert!(test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
    }

    #[test]
    fn when_rat_and_emer_gen_man_on_push_button_is_pressed_in_case_of_ac_1_and_2_unavailable_emergency_generator_does_not_provide_power_immediately(
    ) {
        let test_bed = test_bed_with()
            .running_engines()
            .gen_off(1)
            .gen_off(2)
            .and()
            .rat_and_emer_gen_man_on_pressed()
            .run_waiting_for(Duration::from_secs(0));

        assert!(!test_bed
            .ac_ess_bus_output()
            .is_single(PotentialOrigin::EmergencyGenerator));
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_engine_fire_push_button_released_and_only_that_engine_is_running_nothing_powers_ac_buses(
        #[case] number: usize,
    ) {
        let test_bed = test_bed_with()
            .running_engine(number)
            .and()
            .released_engine_fire_push_button(number)
            .run();

        assert!(test_bed.ac_bus_output(1).is_unpowered());
        assert!(test_bed.ac_bus_output(2).is_unpowered());
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    fn when_gen_contactor_open_due_to_engine_fire_push_button_released_gen_push_button_has_fault(
        #[case] number: usize,
    ) {
        let mut test_bed = test_bed_with()
            .running_engine(number)
            .and()
            .released_engine_fire_push_button(number)
            .run();

        assert!(test_bed.gen_has_fault(number));
    }
}
