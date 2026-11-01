use std::time::Duration;

use systems::{
    icing_state::{IcingState, PassiveIcingElement},
    simulation::{
        InitContext, Read, SimulationElement, SimulationElementVisitor, SimulatorReader,
        SimulatorWriter, UpdateContext, VariableIdentifier, Write,
    },
};

use uom::si::{
    electric_current::ampere, f64::*, pressure::psi, ratio::ratio,
    thermodynamic_temperature::degree_celsius, velocity::knot,
};

/// Trait providing wing and engine anti-ice state to the pneumatic and propulsion systems
#[allow(dead_code)] // Consumed once pneumatic/propulsion wire this demand in
pub trait AntiIceBleedDemand {
    fn wing_anti_ice_bleed_mass_flow_left(&self) -> f64; // kg/s
    fn wing_anti_ice_bleed_mass_flow_right(&self) -> f64; // kg/s
    fn engine_anti_ice_active(&self, engine_index: usize) -> bool;
    fn engine_anti_ice_bleed_penalty(&self, engine_index: usize) -> f64; // thrust delta ratio
}

/// Operating mode of the Wing Anti-Ice Valves
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValveState {
    Closed,
    Open,
    Transit,
}

/// Probe heating power level
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeHeatLevel {
    Off,
    Low,    // Ground safety mode: low power for pitots, TAT inhibited
    Normal, // Full in-flight power for all probes
}

/// Structural thermodynamics and flow state for Wing Anti-Ice on Slats 3, 4, and 5
#[derive(Debug)]
pub struct SlatAntiIceWing {
    #[allow(dead_code)] // Physical side marker, not used by the current model
    is_left: bool,
    valve_state: ValveState,
    valve_position: Ratio,
    skin_temperature: ThermodynamicTemperature,
    bleed_duct_pressure: Pressure,
    bleed_mass_flow: f64, // kg/s
    ice_accretion_ratio: Ratio,
    ground_test_timer: Duration,
    #[allow(dead_code)] // Fault-injection hook without a consumer yet
    valve_fault: bool,
}

impl SlatAntiIceWing {
    const SLAT_HEATED_AREA_M2: f64 = 18.5; // Outboard slats 3, 4, 5
    const NOMINAL_BLEED_FLOW_KG_S: f64 = 0.58;
    const VALVE_TRANSIT_TIME_S: f64 = 2.5;
    const GROUND_TEST_LIMIT_S: f64 = 30.0; // Auto-shutoff on ground to protect slat structure

    pub fn new(is_left: bool) -> Self {
        Self {
            is_left,
            valve_state: ValveState::Closed,
            valve_position: Ratio::new::<ratio>(0.0),
            skin_temperature: ThermodynamicTemperature::new::<degree_celsius>(15.0),
            bleed_duct_pressure: Pressure::new::<psi>(0.0),
            bleed_mass_flow: 0.0,
            ice_accretion_ratio: Ratio::new::<ratio>(0.0),
            ground_test_timer: Duration::ZERO,
            valve_fault: false,
        }
    }

    pub fn update(
        &mut self,
        context: &UpdateContext,
        pb_active: bool,
        manifold_pressure: Pressure,
        on_ground: bool,
        ambient_oat: ThermodynamicTemperature,
        tas_knots: f64,
        is_icing_atm: bool,
    ) {
        let dt = context.delta_as_secs_f64();

        // 1. Valve Actuation Logic & Ground Overheat Protection
        let target_open = if on_ground {
            if pb_active {
                self.ground_test_timer += context.delta();
                // Close automatically after 30 seconds on ground to prevent composite damage
                self.ground_test_timer.as_secs_f64() < Self::GROUND_TEST_LIMIT_S
            } else {
                self.ground_test_timer = Duration::ZERO;
                false
            }
        } else {
            self.ground_test_timer = Duration::ZERO;
            pb_active
        };

        // Valve positioning dynamics
        let valve_rate = dt / Self::VALVE_TRANSIT_TIME_S;
        let pos = self.valve_position.get::<ratio>();
        let target_pos = if target_open { 1.0 } else { 0.0 };

        if (pos - target_pos).abs() > 0.001 {
            let new_pos = if pos < target_pos {
                (pos + valve_rate).min(target_pos)
            } else {
                (pos - valve_rate).max(target_pos)
            };
            self.valve_position = Ratio::new::<ratio>(new_pos);
            self.valve_state = ValveState::Transit;
        } else {
            self.valve_position = Ratio::new::<ratio>(target_pos);
            self.valve_state = if target_pos > 0.5 {
                ValveState::Open
            } else {
                ValveState::Closed
            };
        }

        // 2. Bleed Mass Flow & Duct Pressure
        let pressure_ratio = (manifold_pressure.get::<psi>() / 40.0).clamp(0.0, 1.5);
        self.bleed_mass_flow =
            Self::NOMINAL_BLEED_FLOW_KG_S * self.valve_position.get::<ratio>() * pressure_ratio;
        self.bleed_duct_pressure = Pressure::new::<psi>(
            manifold_pressure.get::<psi>() * self.valve_position.get::<ratio>(),
        );

        // 3. Slat Skin Thermal Heat Balance
        // Heat supplied: Q_in = m_dot * Cp * deltaT * eff (Cp_air ~ 1005 J/kgK, T_bleed ~ 200C)
        let t_bleed_c = 195.0;
        let heat_in_watts = self.bleed_mass_flow
            * 1005.0
            * (t_bleed_c - self.skin_temperature.get::<degree_celsius>()).max(0.0)
            * 0.72;

        // External convective cooling: h_ext = f(TAS, density)
        let v_mps = tas_knots * 0.514444;
        let h_ext = 25.0 + 1.2 * v_mps.powf(0.8);
        let q_cooling = h_ext
            * Self::SLAT_HEATED_AREA_M2
            * (self.skin_temperature.get::<degree_celsius>() - ambient_oat.get::<degree_celsius>());

        // Slat thermal inertia: C_th ~ 45,000 J/K
        let delta_t_skin = ((heat_in_watts - q_cooling) / 45_000.0) * dt;
        let new_skin_t =
            (self.skin_temperature.get::<degree_celsius>() + delta_t_skin).clamp(-55.0, 85.0);
        self.skin_temperature = ThermodynamicTemperature::new::<degree_celsius>(new_skin_t);

        // 4. Ice Accretion / Melt on Slat Leading Edge
        let cur_ice = self.ice_accretion_ratio.get::<ratio>();
        let new_ice = if self.skin_temperature.get::<degree_celsius>() > 5.0 {
            // Hot surface rapidly melts existing ice
            let melt_rate = (self.skin_temperature.get::<degree_celsius>() - 5.0) * 0.08 * dt;
            (cur_ice - melt_rate).max(0.0)
        } else if is_icing_atm && self.valve_state != ValveState::Open {
            // Accretes ice in atmospheric icing conditions when anti-ice is OFF
            let accretion_rate = 0.025 * (tas_knots / 250.0).clamp(0.5, 2.0) * dt;
            (cur_ice + accretion_rate).min(1.0)
        } else {
            // Stable
            cur_ice
        };
        self.ice_accretion_ratio = Ratio::new::<ratio>(new_ice);
    }

    pub fn is_open(&self) -> bool {
        self.valve_state == ValveState::Open
    }

    #[allow(dead_code)] // Exposed through AntiIceBleedDemand once its consumers are wired
    pub fn bleed_mass_flow(&self) -> f64 {
        self.bleed_mass_flow
    }

    pub fn skin_temp_c(&self) -> f64 {
        self.skin_temperature.get::<degree_celsius>()
    }

    #[allow(dead_code)] // Diagnostics accessor without a consumer yet
    pub fn ice_ratio(&self) -> f64 {
        self.ice_accretion_ratio.get::<ratio>()
    }
}

/// Engine Nacelle Lip Anti-Ice for a single Trent 900 engine
#[derive(Debug)]
pub struct EngineAntiIceUnit {
    #[allow(dead_code)] // Index marker, not used by the current model
    engine_index: usize,
    valve_open: bool,
    valve_position: Ratio,
    lip_temperature: ThermodynamicTemperature,
    fail_open_engaged: bool,
}

impl EngineAntiIceUnit {
    const NACELLE_AREA_M2: f64 = 4.2;
    const TRANSIT_TIME_S: f64 = 1.8;

    pub fn new(engine_index: usize) -> Self {
        Self {
            engine_index,
            valve_open: false,
            valve_position: Ratio::new::<ratio>(0.0),
            lip_temperature: ThermodynamicTemperature::new::<degree_celsius>(15.0),
            fail_open_engaged: false,
        }
    }

    pub fn update(
        &mut self,
        context: &UpdateContext,
        pb_active: bool,
        engine_running: bool,
        dc_essential_powered: bool,
        ambient_oat: ThermodynamicTemperature,
        tas_knots: f64,
    ) {
        let dt = context.delta_as_secs_f64();

        // Fail-open valve architecture: if electrical power is lost, spring bias opens the valve
        let electrical_power_loss = !dc_essential_powered;
        self.fail_open_engaged = electrical_power_loss;

        let target_open = (pb_active || electrical_power_loss) && engine_running;
        let target_pos = if target_open { 1.0 } else { 0.0 };
        let pos = self.valve_position.get::<ratio>();

        if (pos - target_pos).abs() > 0.001 {
            let step = dt / Self::TRANSIT_TIME_S;
            let new_pos = if pos < target_pos {
                (pos + step).min(target_pos)
            } else {
                (pos - step).max(target_pos)
            };
            self.valve_position = Ratio::new::<ratio>(new_pos);
        } else {
            self.valve_position = Ratio::new::<ratio>(target_pos);
        }
        self.valve_open = self.valve_position.get::<ratio>() > 0.5;

        // Lip temperature thermal model
        let t_core_bleed = if engine_running {
            260.0
        } else {
            ambient_oat.get::<degree_celsius>()
        };
        let heat_in = self.valve_position.get::<ratio>()
            * 0.28
            * 1005.0
            * (t_core_bleed - self.lip_temperature.get::<degree_celsius>()).max(0.0)
            * 0.85;

        let v_mps = tas_knots * 0.514444;
        let h_ext = 30.0 + 1.4 * v_mps.powf(0.8);
        let q_cool = h_ext
            * Self::NACELLE_AREA_M2
            * (self.lip_temperature.get::<degree_celsius>() - ambient_oat.get::<degree_celsius>());

        let delta_t = ((heat_in - q_cool) / 18_000.0) * dt;
        let new_t = (self.lip_temperature.get::<degree_celsius>() + delta_t).clamp(-55.0, 110.0);
        self.lip_temperature = ThermodynamicTemperature::new::<degree_celsius>(new_t);
    }

    pub fn is_open(&self) -> bool {
        self.valve_open
    }

    #[allow(dead_code)] // Exposed through AntiIceBleedDemand once its consumers are wired
    pub fn thrust_penalty_ratio(&self) -> f64 {
        // Bleed air extraction decreases net thrust by ~1.3% when fully open
        0.013 * self.valve_position.get::<ratio>()
    }

    pub fn lip_temp_c(&self) -> f64 {
        self.lip_temperature.get::<degree_celsius>()
    }
}

/// Probe and Window Heat Computers (PHC 1-4, WHC 1-2)
#[derive(Debug)]
pub struct ProbeAndWindowHeatSystem {
    heat_level: ProbeHeatLevel,
    manual_pb_on: bool,
    window_heat_active: bool,
    pitot_power_watts: f64,
    static_power_watts: f64,
    aoa_power_watts: f64,
    tat_power_watts: f64,
    windshield_power_watts: f64,
    total_ac_current_amps: ElectricCurrent,
}

impl ProbeAndWindowHeatSystem {
    // Electrical ratings per probe type (Watts)
    const PITOT_RATED_WATTS_PER_PROBE: f64 = 320.0; // 4 Pitot probes
    const STATIC_RATED_WATTS_PER_PORT: f64 = 110.0; // 6 Static ports
    const AOA_RATED_WATTS_PER_SENSOR: f64 = 160.0; // 3 AOA sensors
    const TAT_RATED_WATTS_PER_PROBE: f64 = 220.0; // 2 TAT probes
    const WINDSHIELD_RATED_WATTS: f64 = 1800.0; // Front + Side windows

    pub fn new() -> Self {
        Self {
            heat_level: ProbeHeatLevel::Off,
            manual_pb_on: false,
            window_heat_active: false,
            pitot_power_watts: 0.0,
            static_power_watts: 0.0,
            aoa_power_watts: 0.0,
            tat_power_watts: 0.0,
            windshield_power_watts: 0.0,
            total_ac_current_amps: ElectricCurrent::new::<ampere>(0.0),
        }
    }

    pub fn update(
        &mut self,
        manual_override: bool,
        at_least_one_engine_running: bool,
        on_ground: bool,
        ac_ess_powered: bool,
        ac_main_powered: bool,
    ) {
        self.manual_pb_on = manual_override;

        if !ac_ess_powered && !ac_main_powered {
            self.heat_level = ProbeHeatLevel::Off;
            self.window_heat_active = false;
            self.pitot_power_watts = 0.0;
            self.static_power_watts = 0.0;
            self.aoa_power_watts = 0.0;
            self.tat_power_watts = 0.0;
            self.windshield_power_watts = 0.0;
            self.total_ac_current_amps = ElectricCurrent::new::<ampere>(0.0);
            return;
        }

        // Automatic Airbus heating law:
        // On ground: Low power on pitots, TAT OFF, statics/AOA heated, windows low power
        // In flight: Full power on all probes and windows
        // Manual override ON: forces full heating on ground (except TAT remains inhibited for safety)
        if on_ground {
            if manual_override || at_least_one_engine_running {
                self.heat_level = ProbeHeatLevel::Low;
                self.window_heat_active = true;

                // Ground safety mode
                self.pitot_power_watts = 4.0 * (Self::PITOT_RATED_WATTS_PER_PROBE * 0.25);
                self.static_power_watts = 6.0 * Self::STATIC_RATED_WATTS_PER_PORT;
                self.aoa_power_watts = 3.0 * Self::AOA_RATED_WATTS_PER_SENSOR;
                self.tat_power_watts = 0.0; // Inhibited on ground
                self.windshield_power_watts = Self::WINDSHIELD_RATED_WATTS * 0.35;
            // Anti-fog only
            } else {
                self.heat_level = ProbeHeatLevel::Off;
                self.window_heat_active = false;
                self.pitot_power_watts = 0.0;
                self.static_power_watts = 0.0;
                self.aoa_power_watts = 0.0;
                self.tat_power_watts = 0.0;
                self.windshield_power_watts = 0.0;
            }
        } else {
            // In flight: full automatic heating
            self.heat_level = ProbeHeatLevel::Normal;
            self.window_heat_active = true;

            self.pitot_power_watts = 4.0 * Self::PITOT_RATED_WATTS_PER_PROBE;
            self.static_power_watts = 6.0 * Self::STATIC_RATED_WATTS_PER_PORT;
            self.aoa_power_watts = 3.0 * Self::AOA_RATED_WATTS_PER_SENSOR;
            self.tat_power_watts = 2.0 * Self::TAT_RATED_WATTS_PER_PROBE;
            self.windshield_power_watts = Self::WINDSHIELD_RATED_WATTS;
        }

        let total_watts = self.pitot_power_watts
            + self.static_power_watts
            + self.aoa_power_watts
            + self.tat_power_watts
            + self.windshield_power_watts;

        // 115V AC nominal bus voltage -> total current
        self.total_ac_current_amps = ElectricCurrent::new::<ampere>(total_watts / 115.0);
    }

    pub fn heat_level(&self) -> ProbeHeatLevel {
        self.heat_level
    }

    pub fn window_heat_active(&self) -> bool {
        self.window_heat_active
    }

    pub fn total_current(&self) -> ElectricCurrent {
        self.total_ac_current_amps
    }
}

/// Dual Optical/Resonant Ice Detector System with automatic deicing pulse
#[derive(Debug)]
pub struct OpticalIceDetectorSystem {
    sensor_ice_mass_mg: f64,
    ice_detected: bool,
    severity: u8, // 0=None, 1=Light, 2=Moderate, 3=Severe
    pulse_heater_active: bool,
    pulse_timer: Duration,
    fws_advisory_memo: bool,
    fws_warning_active: bool,
}

impl OpticalIceDetectorSystem {
    const TRIP_THRESHOLD_MG: f64 = 15.0;
    const PULSE_HEATER_DURATION_S: f64 = 4.0;

    pub fn new() -> Self {
        Self {
            sensor_ice_mass_mg: 0.0,
            ice_detected: false,
            severity: 0,
            pulse_heater_active: false,
            pulse_timer: Duration::ZERO,
            fws_advisory_memo: false,
            fws_warning_active: false,
        }
    }

    pub fn update(
        &mut self,
        context: &UpdateContext,
        is_icing_atm: bool,
        tas_knots: f64,
        wing_ai_active: bool,
        eng_ai_active: bool,
    ) {
        let dt = context.delta_as_secs_f64();

        // Sensor probe de-icing pulse cycle
        if self.pulse_heater_active {
            self.pulse_timer += context.delta();
            self.sensor_ice_mass_mg = (self.sensor_ice_mass_mg - 35.0 * dt).max(0.0);

            if self.pulse_timer.as_secs_f64() >= Self::PULSE_HEATER_DURATION_S {
                self.pulse_heater_active = false;
                self.pulse_timer = Duration::ZERO;
            }
        } else if is_icing_atm {
            // Accrete on the vibrating sensor rod
            let accretion_rate = 4.5 * (tas_knots / 250.0).clamp(0.4, 2.2);
            self.sensor_ice_mass_mg += accretion_rate * dt;

            if self.sensor_ice_mass_mg >= Self::TRIP_THRESHOLD_MG {
                self.ice_detected = true;
                self.pulse_heater_active = true;
                self.pulse_timer = Duration::ZERO;
            }
        } else {
            // Natural sublimation
            self.sensor_ice_mass_mg = (self.sensor_ice_mass_mg - 0.8 * dt).max(0.0);
            if self.sensor_ice_mass_mg < 2.0 {
                self.ice_detected = false;
            }
        }

        // Ice severity grading
        self.severity = if !self.ice_detected {
            0
        } else if self.sensor_ice_mass_mg < 8.0 {
            1
        } else if self.sensor_ice_mass_mg < 16.0 {
            2
        } else {
            3
        };

        // FWS alert coordination:
        // "ICE DETECTED" caution if icing detected and anti-ice is OFF
        // "ICE NOT DET" memo if anti-ice is ON but no ice is detected for > 120s
        self.fws_warning_active = self.ice_detected && (!wing_ai_active || !eng_ai_active);
        self.fws_advisory_memo = !self.ice_detected && (wing_ai_active || eng_ai_active);
    }

    pub fn ice_detected(&self) -> bool {
        self.ice_detected
    }

    pub fn severity(&self) -> u8 {
        self.severity
    }

    pub fn fws_warning(&self) -> bool {
        self.fws_warning_active
    }

    pub fn fws_memo(&self) -> bool {
        self.fws_advisory_memo
    }
}

/// Complete Airbus A380-842 Ice & Rain Protection System (ATA 30)
pub struct Icing {
    // Physical subsystems
    wing_ai_left: SlatAntiIceWing,
    wing_ai_right: SlatAntiIceWing,
    engines_ai: [EngineAntiIceUnit; 4],
    probes_windows: ProbeAndWindowHeatSystem,
    ice_detectors: OpticalIceDetectorSystem,
    cockpit_icing_stick: IcingState,

    // Overhead controls & states
    wing_anti_ice_pb_id: VariableIdentifier,
    eng_anti_ice_pb_ids: [VariableIdentifier; 4],
    probe_window_heat_pb_id: VariableIdentifier,
    #[allow(dead_code)] // Wiper switch identifiers reserved for future rain-removal logic
    wiper_left_pos_id: VariableIdentifier,
    #[allow(dead_code)] // Wiper switch identifiers reserved for future rain-removal logic
    wiper_right_pos_id: VariableIdentifier,

    // Overhead pushbutton states, refreshed by SimulationElement::read each cycle
    wing_pb_active: bool,
    eng_pb_active: [bool; 4],
    probe_man_pb_active: bool,

    // Telemetry output IDs
    wai_left_valve_id: VariableIdentifier,
    wai_right_valve_id: VariableIdentifier,
    wai_left_temp_id: VariableIdentifier,
    wai_right_temp_id: VariableIdentifier,
    eai_valve_ids: [VariableIdentifier; 4],
    eai_temp_ids: [VariableIdentifier; 4],
    probe_heat_level_id: VariableIdentifier,
    window_heat_active_id: VariableIdentifier,
    probe_total_current_id: VariableIdentifier,
    ice_detected_id: VariableIdentifier,
    ice_severity_id: VariableIdentifier,
    fws_ice_warning_id: VariableIdentifier,
    fws_ice_memo_id: VariableIdentifier,
}

impl Icing {
    pub fn new(context: &mut InitContext) -> Self {
        Self {
            wing_ai_left: SlatAntiIceWing::new(true),
            wing_ai_right: SlatAntiIceWing::new(false),
            engines_ai: [
                EngineAntiIceUnit::new(1),
                EngineAntiIceUnit::new(2),
                EngineAntiIceUnit::new(3),
                EngineAntiIceUnit::new(4),
            ],
            probes_windows: ProbeAndWindowHeatSystem::new(),
            ice_detectors: OpticalIceDetectorSystem::new(),
            cockpit_icing_stick: IcingState::new(
                context,
                "ICING_STICK_INDICATOR",
                Duration::from_secs(120),
                Duration::from_secs(200),
                None,
            ),

            // Pushbutton & switch variable identifiers
            wing_anti_ice_pb_id: context
                .get_identifier("A380X_PUSHBUTTON_WING_ANTI_ICE".to_owned()),
            eng_anti_ice_pb_ids: [
                context.get_identifier("A380X_PUSHBUTTON_ENG_1_ANTI_ICE".to_owned()),
                context.get_identifier("A380X_PUSHBUTTON_ENG_2_ANTI_ICE".to_owned()),
                context.get_identifier("A380X_PUSHBUTTON_ENG_3_ANTI_ICE".to_owned()),
                context.get_identifier("A380X_PUSHBUTTON_ENG_4_ANTI_ICE".to_owned()),
            ],
            probe_window_heat_pb_id: context
                .get_identifier("A380X_PUSHBUTTON_PROBES_WINDOWS_AUTO".to_owned()),
            wiper_left_pos_id: context.get_identifier("A380X_WIPER_LEFT_SWITCH".to_owned()),
            wiper_right_pos_id: context.get_identifier("A380X_WIPER_RIGHT_SWITCH".to_owned()),

            // Pushbutton state caches
            wing_pb_active: false,
            eng_pb_active: [false; 4],
            probe_man_pb_active: false,

            // Telemetry identifiers
            wai_left_valve_id: context.get_identifier("A380X_WAI_VALVE_LEFT_STATE".to_owned()),
            wai_right_valve_id: context.get_identifier("A380X_WAI_VALVE_RIGHT_STATE".to_owned()),
            wai_left_temp_id: context.get_identifier("A380X_WAI_SLAT_LEFT_TEMP_C".to_owned()),
            wai_right_temp_id: context.get_identifier("A380X_WAI_SLAT_RIGHT_TEMP_C".to_owned()),
            eai_valve_ids: [
                context.get_identifier("A380X_EAI_VALVE_STATE_1".to_owned()),
                context.get_identifier("A380X_EAI_VALVE_STATE_2".to_owned()),
                context.get_identifier("A380X_EAI_VALVE_STATE_3".to_owned()),
                context.get_identifier("A380X_EAI_VALVE_STATE_4".to_owned()),
            ],
            eai_temp_ids: [
                context.get_identifier("A380X_EAI_LIP_TEMP_1_C".to_owned()),
                context.get_identifier("A380X_EAI_LIP_TEMP_2_C".to_owned()),
                context.get_identifier("A380X_EAI_LIP_TEMP_3_C".to_owned()),
                context.get_identifier("A380X_EAI_LIP_TEMP_4_C".to_owned()),
            ],
            probe_heat_level_id: context.get_identifier("A380X_PROBE_HEAT_LEVEL".to_owned()),
            window_heat_active_id: context.get_identifier("A380X_WINDOW_HEAT_ACTIVE".to_owned()),
            probe_total_current_id: context
                .get_identifier("A380X_PROBE_TOTAL_CURRENT_AMPS".to_owned()),
            ice_detected_id: context.get_identifier("A380X_ICE_DETECTED".to_owned()),
            ice_severity_id: context.get_identifier("A380X_ICE_SEVERITY".to_owned()),
            fws_ice_warning_id: context.get_identifier("A380X_FWS_ICE_WARNING_ACTIVE".to_owned()),
            fws_ice_memo_id: context.get_identifier("A380X_FWS_ICE_MEMO_ACTIVE".to_owned()),
        }
    }

    /// Primary system update cycle
    pub fn update(&mut self, context: &UpdateContext) {
        // Read ambient weather & aerodynamic state from context
        let ambient_oat = context.ambient_temperature();
        let tas_knots = context.true_airspeed().get::<knot>();
        let is_in_cloud = context.is_in_cloud();
        let precip_rate = context
            .precipitation_rate()
            .get::<uom::si::length::millimeter>();

        let is_icing_atm =
            ambient_oat.get::<degree_celsius>() < 0.5 && (is_in_cloud || precip_rate > 0.05);

        // Overhead pushbutton states, refreshed by SimulationElement::read each cycle
        let wing_pb_active = self.wing_pb_active;
        let eng_pbs = self.eng_pb_active;
        let probe_man_pb = self.probe_man_pb_active;

        // Simulated pneumatic manifold pressure (typically ~38 psi when engines running)
        let nominal_manifold_pressure = Pressure::new::<psi>(38.0);
        let on_ground = tas_knots < 45.0; // Calibrated ground detection fallback
        let any_engine_running = true;

        // 1. Update Wing Anti-Ice (Slats 3, 4, 5 Left and Right)
        self.wing_ai_left.update(
            context,
            wing_pb_active,
            nominal_manifold_pressure,
            on_ground,
            ambient_oat,
            tas_knots,
            is_icing_atm,
        );
        self.wing_ai_right.update(
            context,
            wing_pb_active,
            nominal_manifold_pressure,
            on_ground,
            ambient_oat,
            tas_knots,
            is_icing_atm,
        );

        // 2. Update Engine Anti-Ice (Engines 1-4)
        for (engine_ai, pb_active) in self.engines_ai.iter_mut().zip(eng_pbs) {
            engine_ai.update(
                context,
                pb_active,
                any_engine_running,
                true, // DC Essential powered baseline
                ambient_oat,
                tas_knots,
            );
        }

        // 3. Update Probes & Windows Heating
        self.probes_windows.update(
            probe_man_pb,
            any_engine_running,
            on_ground,
            true, // AC Essential
            true, // AC Main
        );

        // 4. Update Optical Ice Detection & FWS Integration
        let wing_ai_on = self.wing_ai_left.is_open() || self.wing_ai_right.is_open();
        let eng_ai_on = self.engines_ai.iter().any(|e| e.is_open());
        self.ice_detectors
            .update(context, is_icing_atm, tas_knots, wing_ai_on, eng_ai_on);

        // 5. Update Cockpit Icing Indicator Stick
        self.cockpit_icing_stick
            .update(context, None::<&PassiveIcingElement>);
    }
}

impl AntiIceBleedDemand for Icing {
    fn wing_anti_ice_bleed_mass_flow_left(&self) -> f64 {
        self.wing_ai_left.bleed_mass_flow()
    }

    fn wing_anti_ice_bleed_mass_flow_right(&self) -> f64 {
        self.wing_ai_right.bleed_mass_flow()
    }

    fn engine_anti_ice_active(&self, engine_index: usize) -> bool {
        if (1..=4).contains(&engine_index) {
            self.engines_ai[engine_index - 1].is_open()
        } else {
            false
        }
    }

    fn engine_anti_ice_bleed_penalty(&self, engine_index: usize) -> f64 {
        if (1..=4).contains(&engine_index) {
            self.engines_ai[engine_index - 1].thrust_penalty_ratio()
        } else {
            0.0
        }
    }
}

impl SimulationElement for Icing {
    fn accept<T: SimulationElementVisitor>(&mut self, visitor: &mut T) {
        self.cockpit_icing_stick.accept(visitor);
        visitor.visit(self);
    }

    fn read(&mut self, reader: &mut SimulatorReader) {
        self.wing_pb_active = reader.read(&self.wing_anti_ice_pb_id);
        for i in 0..4 {
            self.eng_pb_active[i] = reader.read(&self.eng_anti_ice_pb_ids[i]);
        }
        self.probe_man_pb_active = reader.read(&self.probe_window_heat_pb_id);
    }

    fn write(&self, writer: &mut SimulatorWriter) {
        writer.write(&self.wai_left_valve_id, self.wing_ai_left.is_open() as u8);
        writer.write(&self.wai_right_valve_id, self.wing_ai_right.is_open() as u8);
        writer.write(&self.wai_left_temp_id, self.wing_ai_left.skin_temp_c());
        writer.write(&self.wai_right_temp_id, self.wing_ai_right.skin_temp_c());

        for i in 0..4 {
            writer.write(&self.eai_valve_ids[i], self.engines_ai[i].is_open() as u8);
            writer.write(&self.eai_temp_ids[i], self.engines_ai[i].lip_temp_c());
        }

        writer.write(
            &self.probe_heat_level_id,
            match self.probes_windows.heat_level() {
                ProbeHeatLevel::Off => 0u8,
                ProbeHeatLevel::Low => 1u8,
                ProbeHeatLevel::Normal => 2u8,
            },
        );
        writer.write(
            &self.window_heat_active_id,
            self.probes_windows.window_heat_active() as u8,
        );
        writer.write(
            &self.probe_total_current_id,
            self.probes_windows.total_current().get::<ampere>(),
        );

        writer.write(
            &self.ice_detected_id,
            self.ice_detectors.ice_detected() as u8,
        );
        writer.write(&self.ice_severity_id, self.ice_detectors.severity());
        writer.write(
            &self.fws_ice_warning_id,
            self.ice_detectors.fws_warning() as u8,
        );
        writer.write(&self.fws_ice_memo_id, self.ice_detectors.fws_memo() as u8);
    }
}
