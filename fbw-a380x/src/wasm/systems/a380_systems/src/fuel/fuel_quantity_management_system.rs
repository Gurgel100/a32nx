use nalgebra::Vector3;
use systems::{
    fuel::{self, FuelInfo, FuelSystem, FuelTank},
    simulation::{
        InitContext, Read, SimulationElementVisitor, SimulatorReader, VariableIdentifier,
    },
};

use uom::si::{f64::*, mass::kilogram};

use crate::systems::{
    shared::{ElectricalBusType, ElectricalBuses},
    simulation::SimulationElement,
};

enum PowerSupplySwitch {
    BATTERY,
    NORMAL,
}

enum PreSelectSwitch {
    INCREASE,
    DECREASE,
}

#[derive(Clone, Copy)]
enum ModeSelect {
    AutoRefuel,
    Off,
    ManualRefuel,
    Defuel,
    Transfer,
}

pub struct RefuelPanelInput {
    total_desired_fuel_id: VariableIdentifier,
    total_desired_fuel_input: Mass,
}
impl RefuelPanelInput {
    pub fn new(context: &mut InitContext) -> Self {
        Self {
            total_desired_fuel_id: context.get_identifier("FUEL_DESIRED".to_owned()),
            total_desired_fuel_input: Mass::default(),
        }
    }

    fn total_desired_fuel_input(&self) -> Mass {
        self.total_desired_fuel_input
    }
}
impl SimulationElement for RefuelPanelInput {
    fn read(&mut self, reader: &mut SimulatorReader) {
        self.total_desired_fuel_input = reader.read(&self.total_desired_fuel_id);
    }
    /*
    fn write(&self, writer: &mut SimulatorWriter) {

    }
     */
}

pub struct IntegratedRefuelPanel {
    powered_by: ElectricalBusType,
    is_powered: bool,
    // power_supply_switch: PowerSupplySwitch,
    // fault_light: bool,
    overflow_light: bool,
    // preselect_switch: PreSelectSwitch,
    mode_select: ModeSelect,
    input: RefuelPanelInput,
}
impl IntegratedRefuelPanel {
    pub fn new(context: &mut InitContext, powered_by: ElectricalBusType) -> Self {
        Self {
            powered_by,
            is_powered: false,
            // power_supply_switch: PowerSupplySwitch::BATTERY,
            // fault_light: false,
            overflow_light: false,
            // preselect_switch: PreSelectSwitch::INCREASE,
            mode_select: ModeSelect::AutoRefuel,
            input: RefuelPanelInput::new(context),
        }
    }

    fn total_desired_fuel(&self) -> Mass {
        self.input.total_desired_fuel_input()
    }

    fn selected_mode(&self) -> ModeSelect {
        self.mode_select
    }

    fn update_overflow_light(&mut self, status: bool) {
        self.overflow_light = status;
    }
}
impl SimulationElement for IntegratedRefuelPanel {
    fn receive_power(&mut self, buses: &impl ElectricalBuses) {
        self.is_powered = buses.is_powered(self.powered_by);
    }

    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.input.accept(visitor);
        visitor.visit(self);
    }
}

pub struct CoreProcessingInputsOutputsCommandModule {
    max_fuel: Mass,
    fuel_center_of_gravity: Vector3<f64>,
    fuel_overflow_status: bool,
    // TODO: Replace with simulated fuel probe
    fuel_total_weight: Mass,
    fuel_total_weight_id: VariableIdentifier,
}
impl CoreProcessingInputsOutputsCommandModule {
    pub fn new(context: &mut InitContext, max_fuel: Mass) -> Self {
        Self {
            max_fuel,
            fuel_center_of_gravity: Vector3::zeros(),
            fuel_overflow_status: false,
            fuel_total_weight_id: context.get_identifier("FUEL TOTAL QUANTITY WEIGHT".to_owned()),
            fuel_total_weight: Mass::default(),
        }
    }

    pub fn update(&mut self, fuel_system_input: &FuelSystem<11>, total_desired_fuel: Mass) {
        self.fuel_center_of_gravity = self.calculate_center_of_gravity(fuel_system_input);
        self.fuel_overflow_status = total_desired_fuel > self.max_fuel;
    }

    fn calculate_center_of_gravity(&self, fuel_system_input: &FuelSystem<11>) -> Vector3<f64> {
        fuel_system_input.center_of_gravity()
    }
}
impl SimulationElement for CoreProcessingInputsOutputsCommandModule {
    fn read(&mut self, reader: &mut SimulatorReader) {
        self.fuel_total_weight = reader.read(&self.fuel_total_weight_id);
    }
}

pub struct CoreProcessingInputsOutputsMonitorModule {
    max_fuel: Mass,
    fuel_center_of_gravity: Vector3<f64>,
    fuel_overflow_status: bool,
}
impl CoreProcessingInputsOutputsMonitorModule {
    pub fn new(max_fuel: Mass) -> Self {
        Self {
            max_fuel,
            fuel_center_of_gravity: Vector3::zeros(),
            fuel_overflow_status: false,
        }
    }

    pub fn update(&mut self, fuel_system_input: &FuelSystem<11>, total_desired_fuel: Mass) {
        self.fuel_center_of_gravity = self.calculate_center_of_gravity(fuel_system_input);
        self.fuel_overflow_status = total_desired_fuel > self.max_fuel;
    }

    fn calculate_center_of_gravity(&self, fuel_system_input: &FuelSystem<11>) -> Vector3<f64> {
        fuel_system_input.center_of_gravity()
    }
}
impl SimulationElement for CoreProcessingInputsOutputsMonitorModule {}

pub struct FuelQuantityDataConcentrator {
    powered_by: ElectricalBusType,
    is_powered: bool,
    fuel_overflow_status: bool,
    cpiom_command: CoreProcessingInputsOutputsCommandModule,
    cpiom_monitor: CoreProcessingInputsOutputsMonitorModule,
}
impl FuelQuantityDataConcentrator {
    pub fn new(
        powered_by: ElectricalBusType,
        cpiom_command: CoreProcessingInputsOutputsCommandModule,
        cpiom_monitor: CoreProcessingInputsOutputsMonitorModule,
    ) -> Self {
        Self {
            powered_by,
            is_powered: false,
            cpiom_command,
            cpiom_monitor,
            fuel_overflow_status: false,
        }
    }

    pub fn update(&mut self, fuel_system_input: &FuelSystem<11>, total_desired_fuel: Mass) {
        self.cpiom_command
            .update(fuel_system_input, total_desired_fuel);
        self.cpiom_monitor
            .update(fuel_system_input, total_desired_fuel);

        self.fuel_overflow_status =
            self.cpiom_command.fuel_overflow_status || self.cpiom_monitor.fuel_overflow_status;
    }
}
impl SimulationElement for FuelQuantityDataConcentrator {
    fn receive_power(&mut self, buses: &impl ElectricalBuses) {
        self.is_powered = buses.is_powered(self.powered_by);
    }

    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.cpiom_command.accept(visitor);
        self.cpiom_monitor.accept(visitor);
        visitor.visit(self);
    }
}

pub struct A380FuelQuantityManagementSystem {
    fqdc_1: FuelQuantityDataConcentrator,
    fqdc_2: FuelQuantityDataConcentrator,
    integrated_refuel_panel: IntegratedRefuelPanel,
    fuel_system: FuelSystem<11>,
}
impl A380FuelQuantityManagementSystem {
    pub fn new(context: &mut InitContext, fuel_tanks_info: [FuelInfo; 11]) -> Self {
        let total_max_fuel_quantity = Mass::new::<kilogram>(
            fuel_tanks_info
                .iter()
                .map(|f| f.total_capacity_gallons)
                .sum::<f64>()
                * fuel::FUEL_GALLONS_TO_KG,
        );

        let fuel_tanks = fuel_tanks_info.map(|f| {
            FuelTank::new(
                context,
                f.fuel_tank_id,
                Vector3::new(f.position.0, f.position.1, f.position.2),
            )
        });
        let fuel_system = FuelSystem::new(context, fuel_tanks);

        Self {
            integrated_refuel_panel: IntegratedRefuelPanel::new(
                context,
                // TODO FIXME: Find correct elec bus?
                ElectricalBusType::DirectCurrentBattery,
            ),
            fqdc_1: FuelQuantityDataConcentrator::new(
                // TODO FIXME: Find correct elec bus?
                ElectricalBusType::DirectCurrentBattery,
                CoreProcessingInputsOutputsCommandModule::new(context, total_max_fuel_quantity),
                CoreProcessingInputsOutputsMonitorModule::new(total_max_fuel_quantity),
            ),
            fqdc_2: FuelQuantityDataConcentrator::new(
                // TODO FIXME: Find correct elec bus?
                ElectricalBusType::DirectCurrentBattery,
                CoreProcessingInputsOutputsCommandModule::new(context, total_max_fuel_quantity),
                CoreProcessingInputsOutputsMonitorModule::new(total_max_fuel_quantity),
            ),
            fuel_system,
        }
    }

    pub(crate) fn update(&mut self) {
        let total_desired_fuel: Mass = self.integrated_refuel_panel.total_desired_fuel();
        self.fqdc_1.update(&self.fuel_system, total_desired_fuel);
        self.fqdc_2.update(&self.fuel_system, total_desired_fuel);

        match self.integrated_refuel_panel.selected_mode() {
            ModeSelect::AutoRefuel => {
                self.command_auto_refuel(total_desired_fuel);
            }
            _ => {}
        }

        self.update_status()
    }

    fn calculate_auto_refuel(&self, total_desired_fuel: Mass) -> [Mass; 11] {
        let a: Mass = Mass::new::<kilogram>(18000.);
        let b: Mass = Mass::new::<kilogram>(26000.);
        let c: Mass = Mass::new::<kilogram>(36000.);
        let d: Mass = Mass::new::<kilogram>(47000.);
        let e: Mass = Mass::new::<kilogram>(103788.);
        let f: Mass = Mass::new::<kilogram>(158042.);
        let g: Mass = Mass::new::<kilogram>(215702.);
        let h: Mass = Mass::new::<kilogram>(223028.);

        // TODO FIXME: Trim tank logic
        let trim_fuel: Mass = if total_desired_fuel <= e {
            Mass::default()
        } else if total_desired_fuel <= f {
            total_desired_fuel - e
        } else if total_desired_fuel <= h {
            total_desired_fuel - f
        } else {
            total_desired_fuel - h
        };

        let wing_fuel: Mass = total_desired_fuel - trim_fuel;

        let feed_a: Mass = Mass::new::<kilogram>(4500.);
        let feed_c: Mass = Mass::new::<kilogram>(7000.);
        let outer_feed_e: Mass = Mass::new::<kilogram>(20558.);
        let inner_feed_e: Mass = Mass::new::<kilogram>(21836.);
        let total_feed_e: Mass = outer_feed_e * 2. + inner_feed_e * 2.;

        let outer_feed: Mass = if wing_fuel <= a {
            wing_fuel / 4.
        } else if wing_fuel <= b {
            feed_a
        } else if wing_fuel <= c {
            feed_a + (wing_fuel - b) / 4.
        } else if wing_fuel <= d {
            feed_c
        } else if wing_fuel <= e {
            feed_c + (wing_fuel - d) * (outer_feed_e / total_feed_e)
        } else if wing_fuel <= h {
            outer_feed_e
        } else {
            outer_feed_e + (wing_fuel - h) / 10.
        };

        let inner_feed: Mass = if wing_fuel <= a {
            wing_fuel / 4.
        } else if wing_fuel <= b {
            feed_a
        } else if wing_fuel <= c {
            feed_a + (wing_fuel - b) / 4.
        } else if wing_fuel <= d {
            feed_c
        } else if wing_fuel <= e {
            feed_c + (wing_fuel - d) * (inner_feed_e / total_feed_e)
        } else if wing_fuel <= h {
            inner_feed_e
        } else {
            inner_feed_e + (wing_fuel - h) / 10.
        };

        let outer_tank_b: Mass = Mass::new::<kilogram>(4000.);
        let outer_tank_h: Mass = Mass::new::<kilogram>(7693.);

        let outer_tank: Mass = if wing_fuel <= a {
            Mass::default()
        } else if wing_fuel <= b {
            (wing_fuel - a) / 2.
        } else if wing_fuel <= g {
            outer_tank_b
        } else if wing_fuel <= h {
            outer_tank_b + (wing_fuel - g) / 2.
        } else {
            outer_tank_h + (wing_fuel - h) / 10.
        };

        let inner_tank_d: Mass = Mass::new::<kilogram>(5500.);
        let inner_tank_g: Mass = Mass::new::<kilogram>(34300.);

        let inner_tank: Mass = if wing_fuel <= c {
            Mass::default()
        } else if wing_fuel <= d {
            (wing_fuel - c) / 2.
        } else if wing_fuel <= f {
            inner_tank_d
        } else if wing_fuel <= g {
            inner_tank_d + (wing_fuel - f) / 2.
        } else if wing_fuel <= h {
            inner_tank_g
        } else {
            inner_tank_g + (wing_fuel - h) / 10.
        };

        let mid_tank_f: Mass = Mass::new::<kilogram>(27127.);

        let mid_tank: Mass = if wing_fuel <= e {
            Mass::default()
        } else if wing_fuel <= f {
            (wing_fuel - e) / 2.
        } else if wing_fuel <= h {
            mid_tank_f
        } else {
            mid_tank_f + (wing_fuel - h) / 10.
        };

        [
            outer_tank, // LEFT_OUTER
            outer_feed, // FEED_ONE
            mid_tank,   // LEFT_MID
            inner_tank, // LEFT_INNER
            inner_feed, // FEED_TWO
            inner_feed, // FEED_THREE
            inner_tank, // RIGHT_INNER
            mid_tank,   // RIGHT_MID
            outer_feed, // FEED_FOUR
            outer_tank, // RIGHT_OUTER
            trim_fuel,  // TRIM
        ]
    }

    fn command_auto_refuel(&mut self, total_desired_fuel: Mass) {
        let total_fuel: Mass = self.fuel_system.total_load();
        let desired_fuel_levels: [Mass; 11] = self.calculate_auto_refuel(total_desired_fuel);
    }

    fn update_status(&mut self) {
        self.integrated_refuel_panel.update_overflow_light(
            self.fqdc_1.fuel_overflow_status || self.fqdc_2.fuel_overflow_status,
        );
    }

    pub fn fuel_system(&self) -> &FuelSystem<11> {
        &self.fuel_system
    }
}
impl SimulationElement for A380FuelQuantityManagementSystem {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.fqdc_1.accept(visitor);
        self.fqdc_2.accept(visitor);
        self.integrated_refuel_panel.accept(visitor);
        self.fuel_system.accept(visitor);
        visitor.visit(self);
    }
}
