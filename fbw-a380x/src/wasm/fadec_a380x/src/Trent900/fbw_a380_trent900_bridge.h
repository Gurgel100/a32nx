/**
 * @file fbw_a380_trent900_bridge.h
 * @brief FlyByWire Simulations A380X / MSFS 2024 Rolls-Royce Trent 900 WASM Bridge Interface
 *
 * High-fidelity 3-spool aerothermodynamic cycle, FADEC, SAS, and life prediction engine.
 * Designed for direct embedding into FlyByWire A380 MSFS 2024 WASM systems loop (30-60 Hz).
 *
 * Copyright (c) 2026 Trent 900 Digital Twin Team & FlyByWire Simulations.
 */

// Copyright (c) 2026 Trent 900 Digital Twin Team & FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

#ifndef FBW_A380_TRENT900_BRIDGE_H
#define FBW_A380_TRENT900_BRIDGE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>

#define FBW_A380_NUM_ENGINES 4

/* =========================================================================
 * 1. AIRBUS A380 COCKPIT & SYSTEM INPUTS (PER ENGINE)
 * ========================================================================= */
typedef struct {
    /* Throttle & FADEC Controls */
    double throttle_lever_angle_deg; /* TLA: -20.0 (Full Rev) to 0.0 (Idle Rev), 20.0 (Idle), 35.0 (CL), 45.0 (FLX), 53.0 (TOGA) */
    bool   master_switch_on;         /* Engine Master Switch: true = ON, false = OFF */
    bool   fire_pushbutton_released; /* Fire switch released: true = fuel LP valve shut */
    bool   starter_valve_open;       /* Pneumatic starter air valve solenoid */
    bool   igniters_armed;           /* Dual FADEC A/B continuous ignition */

    /* Airbus Pneumatic & Electrical Extractions */
    bool   ecs_pack_bleed_active;    /* High-pressure bleed extraction for ECS Pack (0.85 - 1.60 kg/s) */
    bool   anti_ice_bleed_active;    /* Nacelle & Wing Anti-Ice thermal bleed */
    double vfg_elec_load_kva;        /* Variable Frequency Generator electrical shaft load (0 - 180 kVA) */

    /* Injected Failures / Fault Overrides (Optional QA Injection) */
    bool   fault_flameout;           /* Immediate fuel cut flameout */
    bool   fault_egt_sensor_drift;   /* Thermocouple +45°C calibration drift */
    bool   fault_p02_probe_loss;     /* Total inlet pressure probe blockage */
    bool   fault_fuel_boost_pump;    /* Main fuel boost pump cavitation */
    bool   fault_comp_stall;         /* HPC aerodynamic instability / rotating stall */
} FBW_A380_EngineInputs;

/* =========================================================================
 * 2. MSFS 2024 ATMOSPHERIC & FLIGHT ENVIRONMENT INPUTS
 * ========================================================================= */
typedef struct {
    double ambient_static_press_pa;  /* Static ambient pressure P0 (Pa) */
    double ambient_static_temp_k;    /* Static ambient temperature T0 (K) */
    double true_airspeed_ms;         /* True Airspeed VTAS (m/s) */
    double flight_mach;              /* Flight Mach number */
    double pressure_altitude_ft;     /* Pressure altitude (ft) */
    double angle_of_attack_deg;      /* Aircraft Angle of Attack (deg) */
    double crosswind_component_kts;  /* Crosswind velocity across nacelle (kts) */
    double sim_delta_time_s;         /* Frame delta time dt (s, typical 0.0166 to 0.0333 s) */
} FBW_A380_EnvironmentInputs;

/* =========================================================================
 * 3. ENGINE TELEMETRY & ECAM OUTPUTS (PER ENGINE)
 * ========================================================================= */
typedef struct {
    /* 3-Spool Speeds (Raw & Corrected % RPM) */
    double N1_pct;                   /* LP Fan Speed: 0 - 100.5% (100% = 2,900 RPM) */
    double N2_pct;                   /* IP Compressor Speed: 0 - 106.0% (100% = 8,500 RPM) */
    double N3_pct;                   /* HP Compressor Speed: 0 - 115.0% (100% = 12,500 RPM) */

    /* Thrust & Thermodynamic Gas Path */
    double net_thrust_kn;            /* Net Thrust per engine (kN) */
    double net_thrust_lbf;           /* Net Thrust per engine (lbf) */
    double fuel_flow_kgs;            /* Fuel Mass Flow Rate (kg/s) */
    double fuel_flow_pph;            /* Fuel Mass Flow Rate (Pounds Per Hour) */
    double fuel_flow_kgh;            /* Fuel Mass Flow Rate (kg/h) */
    double egt_c;                    /* Exhaust Gas Temperature T6 (°C) */
    double tet_t4_k;                 /* Combustor Turbine Entry Temperature T4 (K) */
    double epr_actual;               /* Engine Pressure Ratio P49 / P20 */
    double opr_overall;              /* Overall Pressure Ratio P30 / P20 */

    /* FADEC State & Limiter Status */
    char   engine_state_str[32];     /* "COLD_DARK", "CRANKING", "LIGHT_OFF", "IDLE", "CLIMB", "TOGA", "REVERSE" */
    double tla_flex_temp_c;          /* Assumed / Flex Temperature if in FLX detent (°C) */
    double hpc_surge_margin_pct;     /* HPC Surge Margin (Nominal > 18%, Min > 12%) */
    bool   fadec_channel_a_healthy;  /* EEC Channel A Health Status */
    bool   fadec_channel_b_healthy;  /* EEC Channel B Health Status */

    /* Fluid & Mechanical Subsystems */
    double oil_pressure_psi;         /* Main bearing oil delivery pressure (psi) */
    double oil_temperature_c;        /* Scavenge oil temperature (°C) */
    double fuel_pressure_psi;        /* Hydromechanical fuel pump pressure (psi) */
    double fuel_nozzle_temp_c;       /* Fuel Oil Heat Exchanger (FOHE) delivery temp (°C) */
    double vibration_n1_mils;        /* Fan / LP Spool Vibration level (mils pk-pk) */
    double vibration_n2_mils;        /* IP Spool Vibration level (mils pk-pk) */
    double vibration_n3_mils;        /* HP Spool Vibration level (mils pk-pk) */

    /* Inboard Thrust Reverser (Engines 2 & 3 only) */
    bool   reverser_equipped;        /* true for Eng 2 & 3, false for Eng 1 & 4 */
    double reverser_deploy_pct;      /* Blocker door deployment percentage (0 - 100%) */

    /* Active Clearance Control (ACC) & Rotor Bow */
    double hpt_tip_clearance_mm;     /* Dynamic running blade tip clearance gap (mm) */
    double acc_cooling_flow_pct;     /* HPC stage 6 casing cooling valve modulation (%) */
    double rotor_bow_displacement_um;/* Post-shutdown thermal bow eccentricity (µm) */

    /* CMSX-4 Structural Life Consumption */
    double hpt_blade_stress_mpa;     /* Centrifugal + thermal effective Von Mises stress (MPa) */
    double cumulative_creep_pct;     /* Larson-Miller creep rupture life consumed (%) */
    double cumulative_lcf_damage_pct;/* Manson-Coffin low-cycle fatigue damage (%) */
    double time_on_wing_rem_efh;     /* Remaining Time-on-Wing Engine Flight Hours (EFH) */
} FBW_A380_EngineOutputs;

/* =========================================================================
 * 4. MASTER 4-ENGINE FLEET TELEMETRY STRUCT
 * ========================================================================= */
typedef struct {
    FBW_A380_EngineOutputs engines[FBW_A380_NUM_ENGINES];

    /* Total Fleet Metrics */
    double total_fleet_thrust_kn;    /* Total thrust of all 4 engines (kN) */
    double total_fleet_thrust_lbf;   /* Total thrust of all 4 engines (lbf) */
    double total_fuel_flow_kgh;      /* Total fuel flow of all 4 engines (kg/h) */
    double specific_fuel_cons_tsfc;  /* Fleet specific fuel consumption (kg/kN·h) */
    bool   oei_condition_active;     /* One Engine Inoperative alert active */
} FBW_A380_FleetOutputs;

/* =========================================================================
 * 5. C WASM API EXPORT FUNCTIONS
 * ========================================================================= */

/**
 * @brief Initializes the 4 Trent 900 engines to Cold & Dark gate state.
 * @param ambient_oat_c Outside Air Temperature at the gate in °C (typically 15.0°C).
 */
void FBW_A380_Trent900_Initialize(double ambient_oat_c);

/**
 * @brief Main per-frame physics execution step (called at 30 Hz - 60 Hz in WASM loop).
 *
 * @param env_inputs Current MSFS 2024 atmospheric and flight state.
 * @param eng_inputs Array of 4 engine cockpit/system control inputs.
 * @param fleet_outputs Pointer to struct receiving computed 4-engine telemetry and ECAM outputs.
 */
void FBW_A380_Trent900_Update(
    const FBW_A380_EnvironmentInputs* env_inputs,
    const FBW_A380_EngineInputs       eng_inputs[FBW_A380_NUM_ENGINES],
    FBW_A380_FleetOutputs*            fleet_outputs
);

/**
 * @brief Resets a specific engine to Cold & Dark or running state.
 * @param engine_index 0: Eng 1 (Outboard L), 1: Eng 2 (Inboard L), 2: Eng 3 (Inboard R), 3: Eng 4 (Outboard R).
 * @param start_at_idle true = initialize at stabilized Ground Idle, false = Cold & Dark at gate.
 */
void FBW_A380_Trent900_ResetEngine(int engine_index, bool start_at_idle);

#ifdef __cplusplus
}
#endif

#endif /* FBW_A380_TRENT900_BRIDGE_H */
