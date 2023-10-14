use nalgebra::Vector3;
use systems::{
    fuel::FuelSystem,
    simulation::{InitContext, SimulationElementVisitor},
};

use uom::si::f64::*;

use crate::systems::{
    shared::{ElectricalBusType, ElectricalBuses},
    simulation::SimulationElement,
};

pub struct IntegratedRefuelPanel {
    // powered_by: ElectricalBusType,
    // is_powered: bool,
}
impl IntegratedRefuelPanel {
    pub fn new(/* powered_by: ElectricalBusType */) -> Self {
        Self {
            // powered_by,
            // is_powered: false,
        }
    }

    pub fn update(&mut self) {}

    pub fn backup_manual_fuel_entry(total_fuel: Mass) {}
}
impl SimulationElement for IntegratedRefuelPanel {
    /*
    fn receive_power(&mut self, buses: &impl ElectricalBuses) {
        self.is_powered = buses.is_powered(self.powered_by);
    }
    */
}

pub struct FuelQuantityDataConcentrator {
    powered_by: ElectricalBusType,
    is_powered: bool,
    cpiom_com: CoreProcessingInputsOutputsCommandModule,
    cpiom_mon: CoreProcessingInputsOutputsMonitorModule,
}
impl FuelQuantityDataConcentrator {
    pub fn new(
        powered_by: ElectricalBusType,
        cpiom_com: CoreProcessingInputsOutputsCommandModule,
        cpiom_mon: CoreProcessingInputsOutputsMonitorModule,
    ) -> Self {
        Self {
            powered_by,
            is_powered: false,
            cpiom_com,
            cpiom_mon,
        }
    }

    pub fn update(&mut self, fuel_system_input: &mut FuelSystem<11>) {
        self.cpiom_com.update(fuel_system_input);
        self.cpiom_mon.update(fuel_system_input);
    }
}
impl SimulationElement for FuelQuantityDataConcentrator {
    fn receive_power(&mut self, buses: &impl ElectricalBuses) {
        self.is_powered = buses.is_powered(self.powered_by);
    }

    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.cpiom_com.accept(visitor);
        self.cpiom_mon.accept(visitor);
        visitor.visit(self);
    }
}

pub struct CoreProcessingInputsOutputsCommandModule {}
impl CoreProcessingInputsOutputsCommandModule {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&mut self, fuel_system_input: &mut FuelSystem<11>) {}
}
impl SimulationElement for CoreProcessingInputsOutputsCommandModule {}

pub struct CoreProcessingInputsOutputsMonitorModule {
    center_of_gravity: Vector3<f64>,
}
impl CoreProcessingInputsOutputsMonitorModule {
    pub fn new() -> Self {
        Self {
            center_of_gravity: Vector3::zeros(),
        }
    }

    pub fn update(&mut self, fuel_system_input: &mut FuelSystem<11>) {
        self.center_of_gravity = self.calculate_center_of_gravity(fuel_system_input);
    }

    fn calculate_center_of_gravity(&self, fuel_system_input: &mut FuelSystem<11>) -> Vector3<f64> {
        fuel_system_input.center_of_gravity()
    }
}
impl SimulationElement for CoreProcessingInputsOutputsMonitorModule {}

pub struct FuelQuantityManagementSystem {
    fqdc_1: FuelQuantityDataConcentrator,
    fqdc_2: FuelQuantityDataConcentrator,
    integrated_refuel_panel: IntegratedRefuelPanel,
}
impl FuelQuantityManagementSystem {
    pub fn new() -> Self {
        Self {
            integrated_refuel_panel: IntegratedRefuelPanel::new(),
            fqdc_1: FuelQuantityDataConcentrator::new(
                ElectricalBusType::DirectCurrent(1),
                CoreProcessingInputsOutputsCommandModule::new(),
                CoreProcessingInputsOutputsMonitorModule::new(),
            ),
            fqdc_2: FuelQuantityDataConcentrator::new(
                ElectricalBusType::DirectCurrent(2),
                CoreProcessingInputsOutputsCommandModule::new(),
                CoreProcessingInputsOutputsMonitorModule::new(),
            ),
        }
    }

    pub(crate) fn update(&mut self, fuel_system_input: &mut FuelSystem<11>) {
        self.fqdc_1.update(fuel_system_input);
        self.fqdc_2.update(fuel_system_input);
        self.integrated_refuel_panel.update();
    }
}
impl SimulationElement for FuelQuantityManagementSystem {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.fqdc_1.accept(visitor);
        self.fqdc_2.accept(visitor);
        self.integrated_refuel_panel.accept(visitor);
        visitor.visit(self);
    }
}

/*
Feed 1+4 Tanks
F14 = (T/4) if WT <= 18000
F14 = 4500 if 26000 >= WT > 18000
F14 = 4500 + (T - 26000) / 4 if 36000 >= WT > 26000
F14 = 7000 if 47000 >= WT > 36000
F14 = 7000 + (T - 103788) * (20558/84788) if 103788 >= WT > 47000
F14 = 20558 if 223088 >= WT > 103788
F14 = 20558 + (T - 223088) / 10 if WT > 223088

Feed 2+3 Tanks
F23 = (T/4) if WT <= 18000
F23 = 4500 if 26000 >= WT > 18000
F23 = 4500 + (T - 26000) / 4 if 36000 >= WT > 26000
F23 = 7000 if 47000 >= WT > 36000
F23 = 7000 + (T - 103788) * (21836/84788) if 103788 >= WT > 47000
F23 = 21836 if 223088 >= WT > 103788
F23 = 21836 + (T - 223088) / 10 if WT > 223088

Outer Tanks
O = 0 if WT < 18000
O = (T-18000) / 2 if 26000 >= WT >= 18000
O = 4000 if 215702 >= WT > 26000
O = 4000 + (T - 215702) / 2 if 223088 >= WT  > 215702
O = 7693 + (T-223088) / 10 if WT > 223088

Mid Tanks
M = 0 if WT < 103788
M = (T-103788) / 2 if 158042 >= WT >= 103788
M = 27127 if 223088 >= WT > 158042
M = 27127 + (T-223088) / 10 if WT > 223088

Inner Tanks
I = 0 if WT < 36000
I = (T-36000) / 2 if 47000 >= WT >=  36000
I = 5500 if 158042 >= WT > 47000
I = 5500 + (T - 158042) / 2 if 215702 >= WT  > 158042
I = 34300 if 223088 >= WT > 215702
I = 34300 + (T - 223088) / 10 if WT > 223088
 */
