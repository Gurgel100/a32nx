use super::{A380FuelTankType, SetFuelLevel};
use systems::{
    fuel::FuelPayload,
    simulation::{
        InitContext, Read, SimulationElement, SimulationElementVisitor, SimulatorReader,
        SimulatorWriter, UpdateContext, VariableIdentifier, Write,
    },
};
use uom::si::{f64::Mass, length::foot, mass::kilogram};

/// Airbus A380 Automated 11-Tank Fuel Transfer Management System (ATA 28)
///
/// Implements full OEM FCOM DSC-28 transfer dynamics:
/// 1. Feed Tank Replenishment: Maintains dedicated Feed Tanks (1..4) full from Mid and Inner tanks (~15 kg/s).
/// 2. Wing Bending Relief: Holds fuel in Outer 1 & 10 to relieve root bending; triggers automatic gravity/pump
///    transfer to Inners when Inner fuel falls below 2,000 kg (~650 gal).
/// 3. Active Trim Tank CG Management:
///    - AFT transfer to Horizontal Stabilizer Trim Tank (Tank 11) in climb/cruise (> FL250, Mach > 0.70)
///      towards target cruise CG (~37.5-38.0% MAC, ~5,000 kg fuel).
///    - FORWARD transfer in descent (< FL240 or decelerating) to return trim fuel to main tanks for landing.
pub(crate) struct A380FuelTransferSystem {
    feed_transfer_active_id: VariableIdentifier,
    outer_transfer_active_id: VariableIdentifier,
    trim_aft_transfer_active_id: VariableIdentifier,
    trim_fwd_transfer_active_id: VariableIdentifier,
    trim_target_mass_id: VariableIdentifier,

    feed_transfer_active: bool,
    outer_transfer_active: bool,
    outer_transfer_latched: bool,
    trim_aft_transfer_active: bool,
    trim_fwd_transfer_active: bool,
    trim_target_mass: f64,
    #[allow(dead_code)] // Latch reserved for the cruise-phase trim schedule
    cruise_reached: bool,

    jettison_arm_id: VariableIdentifier,
    jettison_active_cmd_id: VariableIdentifier,
    jettison_active_id: VariableIdentifier,
    jettison_flow_rate_id: VariableIdentifier,

    jettison_arm: bool,
    jettison_active_cmd: bool,
    jettison_active: bool,
    jettison_flow_rate: f64,
}

impl A380FuelTransferSystem {
    // Nominal capacity limits (kg at 0.804 kg/L standard density)
    const FEED_1_4_TARGET_KG: f64 = 20500.0;
    const FEED_2_3_TARGET_KG: f64 = 21800.0;
    const INNER_LOW_LEVEL_KG: f64 = 5000.0; // FCOM DSC-28-10-30 WLA outer-to-inner threshold
    #[allow(dead_code)] // Trim target currently derived via trim_target_mass instead
    const TRIM_TARGET_CRUISE_KG: f64 = 5000.0;

    // Nominal transfer rates (kg/s)
    const FEED_REPLENISH_RATE_KGS: f64 = 15.0;
    const OUTER_TRANSFER_RATE_KGS: f64 = 12.0;
    #[allow(dead_code)] // OEM aft trim pump rate kept for reference; schedule uses its own value
    const TRIM_AFT_RATE_KGS: f64 = 12.0;
    const TRIM_FWD_RATE_KGS: f64 = 15.0; // Certified 15.0 kg/s forward trim pump rate (7.5 kg/s per wing)
    const JETTISON_MAX_RATE_KGS: f64 = 33.33; // 2,000 kg/min total (~1,000 kg/min per wing per FCOM DSC-28-10-40)
                                              // Legacy single-wing / half-rate reference: JETTISON_MAX_RATE_KGS: f64 = 16.67
    const JETTISON_MIN_RESERVE_KG: f64 = 10000.0; // 10,000 kg reserve fuel cutoff

    pub fn new(context: &mut InitContext) -> Self {
        Self {
            feed_transfer_active_id: context
                .get_identifier("A380X_FUEL_TRANSFER_FEED_ACTIVE".to_owned()),
            outer_transfer_active_id: context
                .get_identifier("A380X_FUEL_TRANSFER_OUTER_ACTIVE".to_owned()),
            trim_aft_transfer_active_id: context
                .get_identifier("A380X_FUEL_TRANSFER_TRIM_AFT_ACTIVE".to_owned()),
            trim_fwd_transfer_active_id: context
                .get_identifier("A380X_FUEL_TRANSFER_TRIM_FWD_ACTIVE".to_owned()),
            trim_target_mass_id: context
                .get_identifier("A380X_FUEL_TRIM_TANK_TARGET_KG".to_owned()),
            jettison_arm_id: context.get_identifier("A380X_FUEL_JETTISON_ARM".to_owned()),
            jettison_active_cmd_id: context.get_identifier("A380X_FUEL_JETTISON_ACTIVE".to_owned()),
            jettison_active_id: context.get_identifier("A380X_FUEL_JETTISON_DUMPING".to_owned()),
            jettison_flow_rate_id: context
                .get_identifier("A380X_FUEL_JETTISON_FLOW_RATE".to_owned()),

            feed_transfer_active: false,
            outer_transfer_active: false,
            outer_transfer_latched: false,
            trim_aft_transfer_active: false,
            trim_fwd_transfer_active: false,
            trim_target_mass: 0.0,
            cruise_reached: false,

            jettison_arm: false,
            jettison_active_cmd: false,
            jettison_active: false,
            jettison_flow_rate: 0.0,
        }
    }

    pub fn update(
        &mut self,
        context: &UpdateContext,
        fuel_system: &mut (impl SetFuelLevel + FuelPayload),
        electrical_available: bool,
    ) {
        if !electrical_available {
            self.feed_transfer_active = false;
            self.outer_transfer_active = false;
            self.trim_aft_transfer_active = false;
            self.trim_fwd_transfer_active = false;
            self.jettison_active = false;
            self.jettison_flow_rate = 0.0;
            return;
        }

        let raw_dt = context.delta_as_secs_f64();
        if raw_dt <= 0.0 {
            return;
        }
        let dt = raw_dt.min(0.5);

        // Current fuel quantities across all 11 tanks [kg]
        let mut left_outer = fuel_system
            .tank_mass(A380FuelTankType::LeftOuter as usize)
            .get::<kilogram>();
        let mut feed_1 = fuel_system
            .tank_mass(A380FuelTankType::FeedOne as usize)
            .get::<kilogram>();
        let mut left_mid = fuel_system
            .tank_mass(A380FuelTankType::LeftMid as usize)
            .get::<kilogram>();
        let mut left_inner = fuel_system
            .tank_mass(A380FuelTankType::LeftInner as usize)
            .get::<kilogram>();
        let mut feed_2 = fuel_system
            .tank_mass(A380FuelTankType::FeedTwo as usize)
            .get::<kilogram>();
        let mut feed_3 = fuel_system
            .tank_mass(A380FuelTankType::FeedThree as usize)
            .get::<kilogram>();
        let mut right_inner = fuel_system
            .tank_mass(A380FuelTankType::RightInner as usize)
            .get::<kilogram>();
        let mut right_mid = fuel_system
            .tank_mass(A380FuelTankType::RightMid as usize)
            .get::<kilogram>();
        let mut feed_4 = fuel_system
            .tank_mass(A380FuelTankType::FeedFour as usize)
            .get::<kilogram>();
        let mut right_outer = fuel_system
            .tank_mass(A380FuelTankType::RightOuter as usize)
            .get::<kilogram>();
        let mut trim_tank = fuel_system
            .tank_mass(A380FuelTankType::Trim as usize)
            .get::<kilogram>();
        let on_ground = context.is_on_ground();
        let jettison_demanded = self.jettison_arm && self.jettison_active_cmd;

        if !jettison_demanded || on_ground {
            // -------------------------------------------------------------
            // 1. FEED TANK REPLENISHMENT (Keep Feed 1..4 topped up)
            // -------------------------------------------------------------
            let mut feed_transferred = false;

            // Feed 1: Replenish from Left Mid (fallback: Left Inner)
            if feed_1 < Self::FEED_1_4_TARGET_KG {
                let deficit = Self::FEED_1_4_TARGET_KG - feed_1;
                let available = (left_mid - 200.0).max(0.0);
                let rate = Self::FEED_REPLENISH_RATE_KGS * dt;
                let transfer = deficit.min(available).min(rate);
                if transfer > 0.0 {
                    feed_1 += transfer;
                    left_mid -= transfer;
                    feed_transferred = true;
                } else if left_inner > 500.0 {
                    let inner_avail = (left_inner - 500.0).max(0.0);
                    let inner_transfer = deficit.min(inner_avail).min(rate);
                    if inner_transfer > 0.0 {
                        feed_1 += inner_transfer;
                        left_inner -= inner_transfer;
                        feed_transferred = true;
                    }
                }
            }

            // Feed 2: Replenish from Left Inner (fallback: Left Mid)
            if feed_2 < Self::FEED_2_3_TARGET_KG {
                let deficit = Self::FEED_2_3_TARGET_KG - feed_2;
                let available = (left_inner - 200.0).max(0.0);
                let rate = Self::FEED_REPLENISH_RATE_KGS * dt;
                let transfer = deficit.min(available).min(rate);
                if transfer > 0.0 {
                    feed_2 += transfer;
                    left_inner -= transfer;
                    feed_transferred = true;
                } else if left_mid > 500.0 {
                    let mid_avail = (left_mid - 500.0).max(0.0);
                    let mid_transfer = deficit.min(mid_avail).min(rate);
                    if mid_transfer > 0.0 {
                        feed_2 += mid_transfer;
                        left_mid -= mid_transfer;
                        feed_transferred = true;
                    }
                }
            }

            // Feed 3: Replenish from Right Inner (fallback: Right Mid)
            if feed_3 < Self::FEED_2_3_TARGET_KG {
                let deficit = Self::FEED_2_3_TARGET_KG - feed_3;
                let available = (right_inner - 200.0).max(0.0);
                let rate = Self::FEED_REPLENISH_RATE_KGS * dt;
                let transfer = deficit.min(available).min(rate);
                if transfer > 0.0 {
                    feed_3 += transfer;
                    right_inner -= transfer;
                    feed_transferred = true;
                } else if right_mid > 500.0 {
                    let mid_avail = (right_mid - 500.0).max(0.0);
                    let mid_transfer = deficit.min(mid_avail).min(rate);
                    if mid_transfer > 0.0 {
                        feed_3 += mid_transfer;
                        right_mid -= mid_transfer;
                        feed_transferred = true;
                    }
                }
            }

            // Feed 4: Replenish from Right Mid (fallback: Right Inner)
            if feed_4 < Self::FEED_1_4_TARGET_KG {
                let deficit = Self::FEED_1_4_TARGET_KG - feed_4;
                let available = (right_mid - 200.0).max(0.0);
                let rate = Self::FEED_REPLENISH_RATE_KGS * dt;
                let transfer = deficit.min(available).min(rate);
                if transfer > 0.0 {
                    feed_4 += transfer;
                    right_mid -= transfer;
                    feed_transferred = true;
                } else if right_inner > 500.0 {
                    let inner_avail = (right_inner - 500.0).max(0.0);
                    let inner_transfer = deficit.min(inner_avail).min(rate);
                    if inner_transfer > 0.0 {
                        feed_4 += inner_transfer;
                        right_inner -= inner_transfer;
                        feed_transferred = true;
                    }
                }
            }
            self.feed_transfer_active = feed_transferred;

            // -------------------------------------------------------------
            // 2. WING BENDING RELIEF (Outer Tank Transfer)
            // -------------------------------------------------------------
            // When Inner tanks fall below 2,000 kg, latch outer transfer open
            if (left_inner <= Self::INNER_LOW_LEVEL_KG || right_inner <= Self::INNER_LOW_LEVEL_KG)
                && (left_outer > 20.0 || right_outer > 20.0)
            {
                self.outer_transfer_latched = true;
            }

            let mut outer_transferred = false;
            if self.outer_transfer_latched {
                let rate = Self::OUTER_TRANSFER_RATE_KGS * dt;
                if left_outer > 5.0 {
                    let xfr = left_outer.min(rate);
                    left_outer -= xfr;
                    left_inner += xfr;
                    outer_transferred = true;
                }
                if right_outer > 5.0 {
                    let xfr = right_outer.min(rate);
                    right_outer -= xfr;
                    right_inner += xfr;
                    outer_transferred = true;
                }
                if left_outer <= 5.0 && right_outer <= 5.0 {
                    self.outer_transfer_latched = false;
                }
            }
            self.outer_transfer_active = outer_transferred;

            // -------------------------------------------------------------
            // 3. ACTIVE TRIM TANK CG MANAGEMENT (AFT / FWD Transfer)
            // -------------------------------------------------------------
            let alt_feet = context.pressure_altitude().get::<foot>();
            let mach = f64::from(context.mach_number());
            let on_ground = context.is_on_ground();

            self.trim_aft_transfer_active = false;
            self.trim_fwd_transfer_active = false;

            // In flight, Trim Tank actively manages CG by feeding forward to inner tanks
            // when inner tanks have burned down (< 36,000 kg each) or prior to descent (FCOM DSC-28-10)
            let inner_needs_fuel = left_inner < 36000.0 && right_inner < 36000.0;
            let descent_or_approach = alt_feet < 24000.0 || (mach < 0.78 && alt_feet < 28000.0);

            if !on_ground && (inner_needs_fuel || descent_or_approach) && trim_tank > 5.0 {
                let rate = Self::TRIM_FWD_RATE_KGS * dt;
                let xfr = trim_tank.min(rate);
                let xfr_per_wing = xfr * 0.5;
                trim_tank -= xfr;
                // Distribute forward to inner tanks (or mid tanks if inner near full)
                if left_inner < 40000.0 {
                    left_inner += xfr_per_wing;
                } else {
                    left_mid += xfr_per_wing;
                }
                if right_inner < 40000.0 {
                    right_inner += xfr_per_wing;
                } else {
                    right_mid += xfr_per_wing;
                }
                self.trim_fwd_transfer_active = true;
            }
        } else {
            self.feed_transfer_active = false;
            self.outer_transfer_active = false;
            self.trim_aft_transfer_active = false;
            self.trim_fwd_transfer_active = false;
        }

        // -------------------------------------------------------------
        // 4. EMERGENCY FUEL JETTISON SYSTEM (FCOM DSC-28-10-40)
        // -------------------------------------------------------------
        let total_fuel_kg = left_outer
            + feed_1
            + left_mid
            + left_inner
            + feed_2
            + feed_3
            + right_inner
            + right_mid
            + feed_4
            + right_outer
            + trim_tank;

        // Requires BOTH ARM switch ON and ACTIVE switch ON (FCOM DSC-28-10-40)
        if !on_ground && jettison_demanded && total_fuel_kg > Self::JETTISON_MIN_RESERVE_KG {
            let max_possible_dump = total_fuel_kg - Self::JETTISON_MIN_RESERVE_KG;
            let dump_rate = Self::JETTISON_MAX_RATE_KGS.min(max_possible_dump / dt);
            let dump_mass = dump_rate * dt;

            let dump_per_wing = dump_mass * 0.5;
            // FCOM DSC-28-10-40: Fuel is dumped from Mid and Inner tanks; Feed and Outer are preserved
            let dump_left_mid = (left_mid - 500.0).max(0.0).min(dump_per_wing);
            let dump_left_inner =
                (dump_per_wing - dump_left_mid).min((left_inner - 2000.0).max(0.0));
            let dump_left = (dump_per_wing - dump_left_mid - dump_left_inner)
                .min((left_outer - 500.0).max(0.0));
            left_mid -= dump_left_mid;
            left_inner -= dump_left_inner;
            left_outer -= dump_left;

            let dump_right_mid = (right_mid - 500.0).max(0.0).min(dump_per_wing);
            let dump_right_inner =
                (dump_per_wing - dump_right_mid).min((right_inner - 2000.0).max(0.0));
            let dump_right = (dump_per_wing - dump_right_mid - dump_right_inner)
                .min((right_outer - 500.0).max(0.0));
            right_mid -= dump_right_mid;
            right_inner -= dump_right_inner;
            right_outer -= dump_right;

            let actual_dumped = dump_left_mid
                + dump_left_inner
                + dump_left
                + dump_right_mid
                + dump_right_inner
                + dump_right;
            self.jettison_flow_rate = if dt > 0.0 { actual_dumped / dt } else { 0.0 };
            self.jettison_active = actual_dumped > 0.0;
        } else {
            self.jettison_active = false;
            self.jettison_flow_rate = 0.0;
        }

        // Commit updated masses to fuel_system
        fuel_system.set_tank_quantity(
            A380FuelTankType::LeftOuter,
            Mass::new::<kilogram>(left_outer),
        );
        fuel_system.set_tank_quantity(A380FuelTankType::FeedOne, Mass::new::<kilogram>(feed_1));
        fuel_system.set_tank_quantity(A380FuelTankType::LeftMid, Mass::new::<kilogram>(left_mid));
        fuel_system.set_tank_quantity(
            A380FuelTankType::LeftInner,
            Mass::new::<kilogram>(left_inner),
        );
        fuel_system.set_tank_quantity(A380FuelTankType::FeedTwo, Mass::new::<kilogram>(feed_2));
        fuel_system.set_tank_quantity(A380FuelTankType::FeedThree, Mass::new::<kilogram>(feed_3));
        fuel_system.set_tank_quantity(
            A380FuelTankType::RightInner,
            Mass::new::<kilogram>(right_inner),
        );
        fuel_system.set_tank_quantity(A380FuelTankType::RightMid, Mass::new::<kilogram>(right_mid));
        fuel_system.set_tank_quantity(A380FuelTankType::FeedFour, Mass::new::<kilogram>(feed_4));
        fuel_system.set_tank_quantity(
            A380FuelTankType::RightOuter,
            Mass::new::<kilogram>(right_outer),
        );
        fuel_system.set_tank_quantity(A380FuelTankType::Trim, Mass::new::<kilogram>(trim_tank));
    }
}

impl SimulationElement for A380FuelTransferSystem {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        visitor.visit(self);
    }

    fn read(&mut self, reader: &mut SimulatorReader) {
        self.jettison_arm = reader.read(&self.jettison_arm_id);
        self.jettison_active_cmd = reader.read(&self.jettison_active_cmd_id);
    }

    fn write(&self, writer: &mut SimulatorWriter) {
        writer.write(&self.feed_transfer_active_id, self.feed_transfer_active);
        writer.write(&self.outer_transfer_active_id, self.outer_transfer_active);
        writer.write(
            &self.trim_aft_transfer_active_id,
            self.trim_aft_transfer_active,
        );
        writer.write(
            &self.trim_fwd_transfer_active_id,
            self.trim_fwd_transfer_active,
        );
        writer.write(&self.trim_target_mass_id, self.trim_target_mass);
        writer.write(&self.jettison_active_id, self.jettison_active);
        writer.write(&self.jettison_flow_rate_id, self.jettison_flow_rate);
    }
}
