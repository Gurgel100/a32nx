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

use super::A380FuelTankType;

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
}

pub struct IntegratedRefuelPanel {
    powered_by: ElectricalBusType,
    is_powered: bool,
    mode_select: ModeSelect,
    input: RefuelPanelInput,
}
impl IntegratedRefuelPanel {
    pub fn new(context: &mut InitContext, powered_by: ElectricalBusType) -> Self {
        Self {
            powered_by,
            is_powered: false,
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

enum RefuelRate {
    Instant,
    Fast,
    Real,
}

pub struct RefuelRateInput {
    refuel_rate: i8,
    refuel_rate_id: VariableIdentifier,

    ground_speed_id: VariableIdentifier,
    ground_speed: Velocity,

    is_on_ground_id: VariableIdentifier,
    is_on_ground: bool,
}
impl RefuelRateInput {
    pub fn new(context: &mut InitContext) -> Self {
        Self {
            refuel_rate_id: context.get_identifier("EFB_REFUEL_RATE_SETTING".to_owned()),
            refuel_rate: 0,
            ground_speed_id: context.get_identifier("GPS GROUND SPEED".to_owned()),
            ground_speed: Velocity::default(),
            is_on_ground_id: context.get_identifier("SIM ON GROUND".to_owned()),
            is_on_ground: false,
        }
    }

    fn calculate_refuel_rate() {}
}
impl SimulationElement for RefuelRateInput {
    fn read(&mut self, reader: &mut SimulatorReader) {
        self.refuel_rate = reader.read(&self.refuel_rate_id);
        self.ground_speed = reader.read(&self.ground_speed_id);
        self.is_on_ground = reader.read(&self.is_on_ground_id);
    }
}

pub struct CoreProcessingInputsOutputsCommandModule {
    is_powered: bool,
    powered_by: ElectricalBusType,
    max_fuel: Mass,
    fuel_center_of_gravity: Vector3<f64>,
    fuel_overflow_status: bool,
    // TODO: Replace with simulated fuel probe
    fuel_total_weight: Mass,
    fuel_total_weight_id: VariableIdentifier,

    refuel_rate: RefuelRateInput,
}
impl CoreProcessingInputsOutputsCommandModule {
    pub fn new(context: &mut InitContext, powered_by: ElectricalBusType, max_fuel: Mass) -> Self {
        Self {
            is_powered: false,
            powered_by,
            max_fuel,
            fuel_center_of_gravity: Vector3::zeros(),
            fuel_overflow_status: false,
            fuel_total_weight_id: context.get_identifier("FUEL TOTAL QUANTITY WEIGHT".to_owned()),
            fuel_total_weight: Mass::default(),
            refuel_rate: RefuelRateInput::new(context),
        }
    }

    pub fn update(
        &mut self,
        fuel_system_input: &mut FuelSystem<11>,
        total_desired_fuel: Mass,
        auto_refuel: bool,
    ) {
        if self.is_powered {
            self.fuel_center_of_gravity = self.calculate_center_of_gravity(fuel_system_input);
            self.fuel_overflow_status = total_desired_fuel > self.max_fuel;
            if auto_refuel {
                self.automatic_refuel(fuel_system_input, total_desired_fuel);
            }
        }
    }

    pub fn automatic_refuel(
        &mut self,
        fuel_system_input: &mut FuelSystem<11>,
        total_desired_fuel: Mass,
    ) {
        self.command_auto_refuel(fuel_system_input, total_desired_fuel);
    }

    fn calculate_center_of_gravity(&self, fuel_system: &FuelSystem<11>) -> Vector3<f64> {
        fuel_system.center_of_gravity()
    }

    // TODO: Move into a separate module away from CPIOM logic
    fn command_auto_refuel(&mut self, fuel_system: &mut FuelSystem<11>, total_desired_fuel: Mass) {
        let a: Mass = Mass::new::<kilogram>(18000.);
        let b: Mass = Mass::new::<kilogram>(26000.);
        let c: Mass = Mass::new::<kilogram>(36000.);
        let d: Mass = Mass::new::<kilogram>(47000.);
        let e: Mass = Mass::new::<kilogram>(103788.);
        let f: Mass = Mass::new::<kilogram>(158042.);
        let g: Mass = Mass::new::<kilogram>(215702.);
        let h: Mass = Mass::new::<kilogram>(223028.);

        // TODO FIXME: Trim tank logic
        let trim_fuel: Mass = match total_desired_fuel {
            x if x <= e => Mass::default(),
            x if x <= f => total_desired_fuel - e,
            x if x <= h => total_desired_fuel - f,
            _ => total_desired_fuel - h,
        };

        let wing_fuel: Mass = total_desired_fuel - trim_fuel;

        let feed_a: Mass = Mass::new::<kilogram>(4500.);
        let feed_c: Mass = Mass::new::<kilogram>(7000.);
        let outer_feed_e: Mass = Mass::new::<kilogram>(20558.);
        let inner_feed_e: Mass = Mass::new::<kilogram>(21836.);
        let total_feed_e: Mass = outer_feed_e * 2. + inner_feed_e * 2.;

        let outer_feed: Mass = match wing_fuel {
            x if x <= a => wing_fuel / 4.,
            x if x <= b => feed_a,
            x if x <= c => feed_a + (wing_fuel - b) / 4.,
            x if x <= d => feed_c,
            x if x <= e => feed_c + (wing_fuel - d) * (outer_feed_e / total_feed_e),
            x if x <= h => outer_feed_e,
            _ => outer_feed_e + (wing_fuel - h) / 10.,
        };

        let inner_feed: Mass = match wing_fuel {
            x if x <= a => wing_fuel / 4.,
            x if x <= b => feed_a,
            x if x <= c => feed_a + (wing_fuel - b) / 4.,
            x if x <= d => feed_c,
            x if x <= e => feed_c + (wing_fuel - d) * (inner_feed_e / total_feed_e),
            x if x <= h => inner_feed_e,
            _ => inner_feed_e + (wing_fuel - h) / 10.,
        };

        let outer_tank_b: Mass = Mass::new::<kilogram>(4000.);
        let outer_tank_h: Mass = Mass::new::<kilogram>(7693.);

        let outer_tank: Mass = match wing_fuel {
            x if x <= a => Mass::default(),
            x if x <= b => (wing_fuel - a) / 2.,
            x if x <= g => outer_tank_b,
            x if x <= h => outer_tank_b + (wing_fuel - g) / 2.0,
            _ => outer_tank_h + (wing_fuel - h) / 10.,
        };

        let inner_tank_d: Mass = Mass::new::<kilogram>(5500.);
        let inner_tank_g: Mass = Mass::new::<kilogram>(34300.);

        let inner_tank: Mass = match wing_fuel {
            x if x <= c => Mass::default(),
            x if x <= d => (wing_fuel - c) / 2.,
            x if x <= f => inner_tank_d,
            x if x <= g => inner_tank_d + (wing_fuel - f) / 2.,
            x if x <= h => inner_tank_g,
            _ => inner_tank_g + (wing_fuel - h) / 10.,
        };

        let mid_tank_f: Mass = Mass::new::<kilogram>(27127.);

        let mid_tank: Mass = match wing_fuel {
            x if x <= e => Mass::default(),
            x if x <= f => (wing_fuel - e) / 2.,
            x if x <= h => mid_tank_f,
            _ => mid_tank_f + (wing_fuel - h) / 10.,
        };
        // TODO: maximum amount per tick and use efb refueling rate

        fuel_system.set_tank_quantity(A380FuelTankType::LeftOuter as usize, outer_tank);
        fuel_system.set_tank_quantity(A380FuelTankType::RightOuter as usize, outer_tank);
        fuel_system.set_tank_quantity(A380FuelTankType::LeftMid as usize, mid_tank);
        fuel_system.set_tank_quantity(A380FuelTankType::RightMid as usize, mid_tank);
        fuel_system.set_tank_quantity(A380FuelTankType::LeftInner as usize, inner_tank);
        fuel_system.set_tank_quantity(A380FuelTankType::RightInner as usize, inner_tank);
        fuel_system.set_tank_quantity(A380FuelTankType::FeedOne as usize, outer_feed);
        fuel_system.set_tank_quantity(A380FuelTankType::FeedFour as usize, outer_feed);
        fuel_system.set_tank_quantity(A380FuelTankType::FeedTwo as usize, inner_feed);
        fuel_system.set_tank_quantity(A380FuelTankType::FeedThree as usize, inner_feed);
        fuel_system.set_tank_quantity(A380FuelTankType::Trim as usize, trim_fuel);
    }

    fn is_powered(&self) -> bool {
        self.is_powered
    }
}
impl SimulationElement for CoreProcessingInputsOutputsCommandModule {
    fn read(&mut self, reader: &mut SimulatorReader) {
        self.fuel_total_weight = reader.read(&self.fuel_total_weight_id);
    }
    fn receive_power(&mut self, buses: &impl ElectricalBuses) {
        self.is_powered = buses.is_powered(self.powered_by)
    }
}

pub struct CoreProcessingInputsOutputsMonitorModule {
    powered_by: ElectricalBusType,
    is_powered: bool,
}
impl CoreProcessingInputsOutputsMonitorModule {
    pub fn new(powered_by: ElectricalBusType) -> Self {
        Self {
            is_powered: false,
            powered_by,
        }
    }

    pub fn _update(&mut self) {
        if self.is_powered {
            // TODO: Implement logic here
        }
    }
}
impl SimulationElement for CoreProcessingInputsOutputsMonitorModule {
    fn receive_power(&mut self, buses: &impl ElectricalBuses) {
        self.is_powered = buses.is_powered(self.powered_by)
    }
}

pub struct FuelQuantityDataConcentrator {
    powered_by: ElectricalBusType,
    is_powered: bool,
}
impl FuelQuantityDataConcentrator {
    pub fn new(powered_by: ElectricalBusType) -> Self {
        Self {
            powered_by,
            is_powered: false,
        }
    }

    // TODO: Implement logic here
}
impl SimulationElement for FuelQuantityDataConcentrator {
    fn receive_power(&mut self, buses: &impl ElectricalBuses) {
        self.is_powered = buses.is_powered(self.powered_by);
    }

    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        visitor.visit(self);
    }
}

pub struct A380FuelQuantityManagementSystem {
    fqdc_1: FuelQuantityDataConcentrator,
    cpiom_command_f1: CoreProcessingInputsOutputsCommandModule,
    cpiom_monitor_f3: CoreProcessingInputsOutputsMonitorModule,

    fqdc_2: FuelQuantityDataConcentrator,
    cpiom_command_f2: CoreProcessingInputsOutputsCommandModule,
    cpiom_monitor_f4: CoreProcessingInputsOutputsMonitorModule,

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
                ElectricalBusType::DirectCurrentEssential, // 501PP
            ),
            fqdc_1: FuelQuantityDataConcentrator::new(
                ElectricalBusType::DirectCurrentEssential, // 501PP
            ),
            fqdc_2: FuelQuantityDataConcentrator::new(
                ElectricalBusType::DirectCurrent(1), // 109PP 101PP 107PP
            ),
            cpiom_command_f1: CoreProcessingInputsOutputsCommandModule::new(
                context,
                ElectricalBusType::DirectCurrentEssential,
                total_max_fuel_quantity,
            ),
            cpiom_monitor_f3: CoreProcessingInputsOutputsMonitorModule::new(
                ElectricalBusType::DirectCurrentEssential,
            ),
            cpiom_command_f2: CoreProcessingInputsOutputsCommandModule::new(
                context,
                ElectricalBusType::DirectCurrent(1),
                total_max_fuel_quantity,
            ),
            cpiom_monitor_f4: CoreProcessingInputsOutputsMonitorModule::new(
                ElectricalBusType::DirectCurrent(1),
            ),
            fuel_system,
        }
    }

    pub(crate) fn update(&mut self) {
        let total_desired_fuel: Mass = self.integrated_refuel_panel.total_desired_fuel();

        let auto_refuel = match self.integrated_refuel_panel.selected_mode() {
            ModeSelect::AutoRefuel => true,
            // TODO: Manual fuel tank refueling
            _ => false,
        };

        if self.cpiom_command_f1.is_powered() {
            self.cpiom_command_f1
                .update(&mut self.fuel_system, total_desired_fuel, auto_refuel);
        } else {
            self.cpiom_command_f2
                .update(&mut self.fuel_system, total_desired_fuel, auto_refuel);
        }
    }

    pub fn fuel_system(&self) -> &FuelSystem<11> {
        &self.fuel_system
    }
}
impl SimulationElement for A380FuelQuantityManagementSystem {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.fqdc_1.accept(visitor);
        self.fqdc_2.accept(visitor);
        self.cpiom_command_f1.accept(visitor);
        self.cpiom_command_f2.accept(visitor);
        self.cpiom_monitor_f3.accept(visitor);
        self.cpiom_monitor_f4.accept(visitor);
        self.integrated_refuel_panel.accept(visitor);
        self.fuel_system.accept(visitor);
        visitor.visit(self);
    }
}
