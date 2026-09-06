/**
 * @file fbw_a380_trent900_bridge.c
 * @brief Implementation of the FlyByWire Simulations A380 MSFS 2024 Trent 900 WASM Bridge
 */

// Copyright (c) 2026 Trent 900 Digital Twin Team & FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
//
// Integrated into the FlyByWire A380X fadec-a380x WASM module. On top of the
// original package two integration patches were applied:
//   1. Shutdown wind-down: with the master switch OFF, the fire pushbutton
//      released or on flameout the engine now winds down and cools down
//      (the original bridge left the engine state frozen in that case).
//   2. Windmilling: a stopped engine in forward flight windmills the fan
//      (asymptote of ~35% N1) and produces windmill drag.
//   3. Light-off sequence: the start sequence now runs through an explicit
//      start phase (cold -> motoring -> light-off -> settle -> running). The
//      original code set the flame flag inside the start branch, which made
//      the running branch take over on the same frame and skip the flare
//      and the 50% N3 starter cutout.

#include "fbw_a380_trent900_bridge.h"
#include <math.h>
#include <string.h>
#include <stdio.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

/* Internal Engine State Structure (Persisted Across WASM Frames) */
typedef struct {
    double N1_pct;
    double N2_pct;
    double N3_pct;
    double T_egt_c;
    double T_tet_k;
    double fuel_flow_kgs;
    double net_thrust_kn;
    double tip_clearance_mm;
    double rotor_bow_um;
    double casing_temp_c;
    double oil_temp_c;
    double oil_pres_psi;
    double fuel_pres_psi;
    double creep_damage_frac;
    double lcf_damage_frac;
    double reverser_pos_pct;
    double starter_time_s;
    bool   is_lit;
    bool   starter_engaged;
    /* Start sequence phase: 0=cold, 1=motoring (<16% N3), 2=light-off (<50% N3),
     * 3=settling to ground idle (<60% N3), 4=running (FADEC tracking) */
    int    start_phase;
} InternalEngineState;

static InternalEngineState g_engines[FBW_A380_NUM_ENGINES];
static bool g_initialized = false;

/* =========================================================================
 * INITIALIZATION
 * ========================================================================= */
void FBW_A380_Trent900_Initialize(double ambient_oat_c) {
    for (int i = 0; i < FBW_A380_NUM_ENGINES; i++) {
        g_engines[i].N1_pct            = 0.0;
        g_engines[i].N2_pct            = 0.0;
        g_engines[i].N3_pct            = 0.0;
        g_engines[i].T_egt_c           = ambient_oat_c;
        g_engines[i].T_tet_k           = ambient_oat_c + 273.15;
        g_engines[i].fuel_flow_kgs     = 0.0;
        g_engines[i].net_thrust_kn     = 0.0;
        g_engines[i].tip_clearance_mm  = 1.250; /* Cold assembly gap */
        g_engines[i].rotor_bow_um      = 0.0;
        g_engines[i].casing_temp_c     = ambient_oat_c;
        g_engines[i].oil_temp_c        = ambient_oat_c;
        g_engines[i].oil_pres_psi      = 0.0;
        g_engines[i].fuel_pres_psi     = 0.0;
        g_engines[i].creep_damage_frac = 0.0;
        g_engines[i].lcf_damage_frac   = 0.0;
        g_engines[i].reverser_pos_pct  = 0.0;
        g_engines[i].starter_time_s    = 0.0;
        g_engines[i].is_lit            = false;
        g_engines[i].starter_engaged   = false;
        g_engines[i].start_phase       = 0;
    }
    g_initialized = true;
}

void FBW_A380_Trent900_ResetEngine(int engine_index, bool start_at_idle) {
    if (engine_index < 0 || engine_index >= FBW_A380_NUM_ENGINES) return;
    
    if (start_at_idle) {
        g_engines[engine_index].N1_pct            = 17.52;
        g_engines[engine_index].N2_pct            = 47.52;
        g_engines[engine_index].N3_pct            = 60.00;
        g_engines[engine_index].T_egt_c           = 380.0;
        g_engines[engine_index].T_tet_k           = 820.0;
        g_engines[engine_index].fuel_flow_kgs     = 0.155;
        g_engines[engine_index].net_thrust_kn     = 22.0;
        g_engines[engine_index].tip_clearance_mm  = 0.650;
        g_engines[engine_index].rotor_bow_um      = 0.0;
        g_engines[engine_index].casing_temp_c     = 180.0;
        g_engines[engine_index].oil_temp_c        = 65.0;
        g_engines[engine_index].oil_pres_psi      = 45.0;
        g_engines[engine_index].fuel_pres_psi    = 450.0;
        g_engines[engine_index].is_lit            = true;
        g_engines[engine_index].starter_engaged   = false;
        g_engines[engine_index].start_phase       = 4;
    } else {
        g_engines[engine_index].N1_pct            = 0.0;
        g_engines[engine_index].N2_pct            = 0.0;
        g_engines[engine_index].N3_pct            = 0.0;
        g_engines[engine_index].T_egt_c           = 15.0;
        g_engines[engine_index].T_tet_k           = 288.15;
        g_engines[engine_index].fuel_flow_kgs     = 0.0;
        g_engines[engine_index].net_thrust_kn     = 0.0;
        g_engines[engine_index].tip_clearance_mm  = 1.250;
        g_engines[engine_index].rotor_bow_um      = 0.0;
        g_engines[engine_index].casing_temp_c     = 15.0;
        g_engines[engine_index].oil_temp_c        = 15.0;
        g_engines[engine_index].oil_pres_psi      = 0.0;
        g_engines[engine_index].fuel_pres_psi    = 0.0;
        g_engines[engine_index].is_lit            = false;
        g_engines[engine_index].starter_engaged   = false;
        g_engines[engine_index].start_phase       = 0;
    }
}

/* =========================================================================
 * SHUTDOWN WIND-DOWN & WINDMILLING
 * Called when the engine is not burning fuel (master switch OFF, fire
 * pushbutton released or flameout): the core spools wind down to a stop and
 * the fan is driven by the airflow (windmilling) while in forward flight.
 * ========================================================================= */
static void UpdateWindmillAndWindDown(InternalEngineState*           s,
                                      const FBW_A380_EnvironmentInputs* env,
                                      double                          dt,
                                      char*                           state_str) {
    const double oat_c = (env && env->ambient_static_temp_k > 100.0) ? (env->ambient_static_temp_k - 273.15) : 15.0;
    const double mach  = (env) ? env->flight_mach : 0.0;

    /* Fan windmills with the airflow: asymptote of 35% N1 reached at Mach 0.35+ */
    double n1_windmill = 0.0;
    if (mach > 0.12) {
        n1_windmill = 35.0 * fmin(1.0, (mach - 0.12) / 0.23);
    }
    const double tau_windmill = 20.0;
    if (s->N1_pct >= n1_windmill) {
        s->N1_pct = n1_windmill + (s->N1_pct - n1_windmill) * exp(-dt / tau_windmill);
    } else {
        s->N1_pct = n1_windmill - (n1_windmill - s->N1_pct) * exp(-dt / tau_windmill);
    }

    /* Core spools wind down to a stop (no combustion, no starter torque) */
    const double tau_spool = 15.0;
    s->N2_pct = (s->N2_pct < 0.05) ? 0.0 : s->N2_pct * exp(-dt / tau_spool);
    s->N3_pct = (s->N3_pct < 0.05) ? 0.0 : s->N3_pct * exp(-dt / tau_spool);

    /* No fuel; thrust decays to windmill drag */
    s->fuel_flow_kgs = 0.0;
    const double thrust_target = (n1_windmill > 0.0) ? (-8.0 * (s->N1_pct / 35.0)) : 0.0;
    s->net_thrust_kn += (thrust_target - s->net_thrust_kn) * (dt / 2.0);

    /* EGT cools towards OAT */
    const double tau_egt = 25.0;
    s->T_egt_c = oat_c + (s->T_egt_c - oat_c) * exp(-dt / tau_egt);
    s->T_tet_k = s->T_egt_c + 273.15;

    if (s->N3_pct < 0.5 && s->N1_pct < 1.0) {
        strcpy(state_str, "COLD_DARK");
    } else {
        strcpy(state_str, "SHUTDOWN");
    }
}

/* =========================================================================
 * MAIN WASM PER-FRAME UPDATE STEP (30-60 Hz)
 * ========================================================================= */
void FBW_A380_Trent900_Update(
    const FBW_A380_EnvironmentInputs* env,
    const FBW_A380_EngineInputs       inputs[FBW_A380_NUM_ENGINES],
    FBW_A380_FleetOutputs*            out
) {
    if (!g_initialized) {
        FBW_A380_Trent900_Initialize(env ? (env->ambient_static_temp_k - 273.15) : 15.0);
    }
    
    double dt = (env && env->sim_delta_time_s > 0.001 && env->sim_delta_time_s < 0.5) 
                ? env->sim_delta_time_s : 0.0333;
    
    double P0 = (env && env->ambient_static_press_pa > 5000.0) ? env->ambient_static_press_pa : 101325.0;
    double T0 = (env && env->ambient_static_temp_k > 100.0)     ? env->ambient_static_temp_k : 288.15;
    double Mach = (env) ? env->flight_mach : 0.0;
    
    /* Altitude thrust lapse factor theta & delta */
    double delta_amb = P0 / 101325.0;
    double theta_amb = T0 / 288.15;
    double ram_factor = 1.0 + 0.2 * Mach * Mach;
    double P02_P0 = pow(ram_factor, 3.5) * 0.994; /* 0.994 ram recovery */
    
    double fleet_thrust_kn = 0.0;
    double fleet_wf_kgs = 0.0;
    bool   any_oei = false;
    
    for (int i = 0; i < FBW_A380_NUM_ENGINES; i++) {
        InternalEngineState* s = &g_engines[i];
        const FBW_A380_EngineInputs* in = &inputs[i];
        FBW_A380_EngineOutputs* o = &out->engines[i];
        
        bool reverser_avail = (i == 1 || i == 2); /* Inboard engines only (Engines 2 & 3) */
        o->reverser_equipped = reverser_avail;
        
        const bool fuel_available = in->master_switch_on && !in->fire_pushbutton_released && !in->fault_flameout;
        
        /* 1. MASTER SWITCH & FLAMEOUT CHECK */
        if (!fuel_available) {
            s->is_lit      = false;
            s->start_phase = 0;
        }
        
        /* 2. SHUTDOWN WIND-DOWN / WINDMILL (fuel cut) */
        if (!fuel_available) {
            UpdateWindmillAndWindDown(s, env, dt, o->engine_state_str);
        }
        /* 3. START SEQUENCING: cold -> motoring -> light-off flare -> settle to idle */
        else if (s->start_phase < 4) {
            if (s->start_phase == 0) {
                if (in->starter_valve_open || in->igniters_armed) {
                    /* Start command: begin pneumatic motoring */
                    s->starter_engaged = true;
                    s->starter_time_s  += dt;
                    s->start_phase     = 1;
                } else {
                    /* No start command: wind down and windmill with the airflow */
                    s->starter_engaged = false;
                    UpdateWindmillAndWindDown(s, env, dt, o->engine_state_str);
                }
            }
            if (s->start_phase == 1) {
                /* Pneumatic torque accelerates HP spool (N3) to light-off */
                s->N3_pct += 1.8 * dt; /* Motoring to 16% */
                s->N2_pct += 0.8 * dt;
                s->N1_pct += 0.2 * dt;
                if (s->N3_pct >= 16.0) {
                    /* Fuel introduction & light-off flare */
                    s->is_lit        = true;
                    s->start_phase   = 2;
                    s->fuel_flow_kgs = 0.180;
                    strcpy(o->engine_state_str, "LIGHT_OFF");
                } else {
                    strcpy(o->engine_state_str, "CRANKING");
                }
            } else if (s->start_phase == 2) {
                /* Light-off: EGT flare up to 635°C peak, spool up to starter cutout at 50% N3 */
                s->N3_pct += 2.2 * dt;
                s->N2_pct += 1.6 * dt;
                s->N1_pct += 0.8 * dt;
                s->T_egt_c += (635.0 - s->T_egt_c) * (dt / 4.0);
                s->fuel_flow_kgs = 0.180;
                if (s->N3_pct >= 50.0) {
                    s->start_phase = 3;
                    strcpy(o->engine_state_str, "IDLE_STABILIZING");
                } else {
                    strcpy(o->engine_state_str, "LIGHT_OFF");
                }
            } else if (s->start_phase == 3) {
                /* Starter cutout at 50% N3, settling to 60% Ground Idle */
                s->N3_pct += 1.2 * dt;
                s->N2_pct += 1.0 * dt;
                s->N1_pct += 0.6 * dt;
                s->T_egt_c += (380.0 - s->T_egt_c) * (dt / 5.0);
                s->fuel_flow_kgs += (0.155 - s->fuel_flow_kgs) * (dt / 5.0);
                s->net_thrust_kn += (22.0 - s->net_thrust_kn) * (dt / 5.0);
                if (s->N3_pct >= 60.0) {
                    s->start_phase = 4;
                }
                strcpy(o->engine_state_str, "IDLE_STABILIZING");
            }
        }
        
        /* 4. RUNNING ENGINE FADEC & AEROTHERMODYNAMICS */
        if (s->is_lit && s->start_phase >= 4) {
                /* FADEC Throttle Command Tracking */
                double tla = in->throttle_lever_angle_deg;
                double target_n1 = 17.52;
                double target_thrust = 22.0;
                
                if (tla < 0.0 && reverser_avail) {
                    /* Reverse Thrust Command (Inboard Engines) */
                    double rev_frac = fmin(1.0, -tla / 20.0);
                    s->reverser_pos_pct = fmin(100.0, s->reverser_pos_pct + (100.0 / 1.8) * dt); /* 1.8s deploy */
                    target_n1 = 17.52 + rev_frac * 55.0;
                    target_thrust = -(rev_frac * 65.0); /* Up to -65 kN reverse thrust */
                    strcpy(o->engine_state_str, "REVERSE_THRUST");
                } else {
                    /* Forward Thrust Command */
                    s->reverser_pos_pct = fmax(0.0, s->reverser_pos_pct - (100.0 / 1.8) * dt);
                    if (tla <= 20.0) {
                        target_n1 = 17.52; target_thrust = 22.0;
                        strcpy(o->engine_state_str, "GROUND_IDLE");
                    } else if (tla <= 35.0) {
                        double frac = (tla - 20.0) / 15.0;
                        target_n1 = 17.52 + frac * (76.0 - 17.52);
                        target_thrust = 22.0 + frac * (265.0 - 22.0);
                        strcpy(o->engine_state_str, "CLIMB_THRUST");
                    } else if (tla <= 45.0) {
                        double frac = (tla - 35.0) / 10.0;
                        target_n1 = 76.0 + frac * (84.0 - 76.0);
                        target_thrust = 265.0 + frac * (335.0 - 265.0);
                        strcpy(o->engine_state_str, "FLEX_TAKEOFF");
                    } else {
                        double frac = fmin(1.0, (tla - 45.0) / 8.0);
                        target_n1 = 84.0 + frac * (88.4 - 84.0);
                        target_thrust = 335.0 + frac * (375.0 - 335.0);
                        strcpy(o->engine_state_str, "MAX_TOGA");
                    }
                }
                
                /* Apply ECS Pack Bleed Penalty (-2.8% Thrust when Packs ON) */
                if (in->ecs_pack_bleed_active) {
                    target_thrust *= 0.972;
                }
                /* Apply Anti-Ice Bleed Penalty (-3.5% Thrust when Anti-Ice ON) */
                if (in->anti_ice_bleed_active) {
                    target_thrust *= 0.965;
                }
                /* Altitude Thrust Lapse with density & ram recovery */
                target_thrust *= delta_amb * sqrt(theta_amb) * P02_P0;
                
                /* Spool Kinematics Lag (tau_N1 = 1.4s, tau_N3 = 0.8s) */
                double tau_n1 = 1.4;
                double tau_n3 = 0.8;
                s->N1_pct += (target_n1 - s->N1_pct) * (dt / tau_n1);
                s->N2_pct = 47.52 + (s->N1_pct - 17.52) * 0.75;
                s->N3_pct += (60.0 + (s->N1_pct - 17.52) * 0.55 - s->N3_pct) * (dt / tau_n3);
                
                s->net_thrust_kn += (target_thrust - s->net_thrust_kn) * (dt / 0.5);
                
                /* Specific Fuel Consumption Model */
                double tsfc = 0.560; /* kg/kN·h */
                if (s->net_thrust_kn > 0.0) {
                    s->fuel_flow_kgs = (s->net_thrust_kn * tsfc) / 3600.0;
                    /* TSFC rises steeply near idle: hold the ground-idle fuel flow at low thrust */
                    if (s->net_thrust_kn <= 25.0) {
                        s->fuel_flow_kgs = 0.155; /* kg/s ground idle */
                    }
                } else {
                    s->fuel_flow_kgs = 0.350 * (fabs(s->net_thrust_kn) / 65.0);
                }
                
                /* EGT & TET Temperatures */
                double egt_target = 380.0 + (s->N1_pct - 17.52) * 6.5;
                if (in->fault_egt_sensor_drift) egt_target += 45.0;
                s->T_egt_c += (egt_target - s->T_egt_c) * (dt / 1.5);
                s->T_tet_k = s->T_egt_c + 273.15 + 380.0;
                
                /* CMSX-4 Larson-Miller Creep Accumulation */
                if (s->T_tet_k > 1400.0) {
                    double lmp = (s->T_tet_k) * (20.0 + log10(100.0)) / 1000.0;
                    double creep_rate = pow(10.0, (lmp - 42.0) / 2.5) * 1e-6;
                    s->creep_damage_frac += creep_rate * dt;
                }
        }
        
        /* 5. FLUIDS, PRESSURES & CLEARANCE */
        s->oil_pres_psi = (s->N3_pct / 100.0) * 55.0;
        s->oil_temp_c   = 60.0 + (s->N3_pct / 100.0) * 35.0;
        s->fuel_pres_psi= (s->N3_pct / 100.0) * 1250.0;
        s->tip_clearance_mm = 1.250 - (s->N3_pct / 100.0) * 0.600;
        
        /* 6. POPULATE OUTPUT TELEMETRY */
        o->N1_pct                   = s->N1_pct;
        o->N2_pct                   = s->N2_pct;
        o->N3_pct                   = s->N3_pct;
        o->net_thrust_kn            = s->net_thrust_kn;
        o->net_thrust_lbf           = s->net_thrust_kn * 224.809;
        o->fuel_flow_kgs            = s->fuel_flow_kgs;
        o->fuel_flow_kgh            = s->fuel_flow_kgs * 3600.0;
        o->fuel_flow_pph            = s->fuel_flow_kgs * 7936.64;
        o->egt_c                    = s->T_egt_c;
        o->tet_t4_k                 = s->T_tet_k;
        o->epr_actual               = 1.0 + (s->N1_pct / 100.0) * 0.45;
        o->opr_overall              = 8.7 + (s->N3_pct / 100.0) * 32.0;
        o->oil_pressure_psi         = s->oil_pres_psi;
        o->oil_temperature_c        = s->oil_temp_c;
        o->fuel_pressure_psi        = s->fuel_pres_psi;
        o->fuel_nozzle_temp_c       = s->oil_temp_c - 15.0;
        o->vibration_n1_mils        = 0.4 + (s->N1_pct / 100.0) * 0.8;
        o->vibration_n2_mils        = 0.3 + (s->N2_pct / 100.0) * 0.6;
        o->vibration_n3_mils        = 0.2 + (s->N3_pct / 100.0) * 0.5;
        o->reverser_deploy_pct      = s->reverser_pos_pct;
        o->hpt_tip_clearance_mm     = s->tip_clearance_mm;
        o->acc_cooling_flow_pct     = (s->N3_pct > 75.0) ? 100.0 : 0.0;
        o->rotor_bow_displacement_um= s->rotor_bow_um;
        o->hpt_blade_stress_mpa     = (s->N3_pct / 100.0) * 580.0;
        o->cumulative_creep_pct     = s->creep_damage_frac * 100.0;
        o->cumulative_lcf_damage_pct= s->lcf_damage_frac * 100.0;
        o->time_on_wing_rem_efh     = 25000.0 - (s->creep_damage_frac * 25000.0);
        o->fadec_channel_a_healthy  = true;
        o->fadec_channel_b_healthy  = true;
        o->hpc_surge_margin_pct     = 22.5 - (s->N3_pct / 100.0) * 4.0;
        
        fleet_thrust_kn += s->net_thrust_kn;
        fleet_wf_kgs += s->fuel_flow_kgs;
        if (!s->is_lit) any_oei = true;
    }
    
    out->total_fleet_thrust_kn   = fleet_thrust_kn;
    out->total_fleet_thrust_lbf  = fleet_thrust_kn * 224.809;
    out->total_fuel_flow_kgh     = fleet_wf_kgs * 3600.0;
    out->specific_fuel_cons_tsfc = (fleet_thrust_kn > 1.0) ? (fleet_wf_kgs * 3600.0) / fleet_thrust_kn : 0.0;
    out->oei_condition_active    = any_oei;
}
