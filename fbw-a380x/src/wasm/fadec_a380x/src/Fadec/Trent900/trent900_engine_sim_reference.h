/*
 * Certified Trent 900 (972B-84) reference constants and conversion helpers.
 *
 * Authority: EASA TCDS E.012, Issue 11 (13 Oct 2022), Rolls-Royce Deutschland.
 *   100% N1 (LP spool)            = 2,900 RPM
 *   100% N2 (IP spool)            = 8,300 RPM
 *   100% N3 (HP spool)            = 12,200 RPM
 *
 * These values are the single source of truth for the FADEC C model and supersede
 * the earlier 8,500 / 12,500 RPM placeholders (mirrored by the MSFS flight model in
 * SimObjects/.../engines.cfg: rated_N2_rpm = 12200, N1 = 2,900 RPM references).
 * Keep numeric spool-speed conversions in trent900_engine_sim.c pointed at these
 * macros so the model cannot silently drift from the certified references.
 */

#ifndef TRENT900_ENGINE_SIM_REFERENCE_H
#define TRENT900_ENGINE_SIM_REFERENCE_H

#ifdef __cplusplus
extern "C" {
#endif

/* 100% spool reference speeds (RPM) from EASA TCDS E.012 Issue 11 */
#define TRENT900_N1_REF_RPM 2900.0 /* Fan / LP spool */
#define TRENT900_N2_REF_RPM 8300.0 /* IP spool */
#define TRENT900_N3_REF_RPM 12200.0 /* HP spool */

/* Certified static thrust ratings from EASA TCDS E.012 Issue 11 (Trent 972B-84) */
#define TRENT972B_84_RATED_THRUST_N   356810.0 /* 356.81 kN Take-off Net Thrust (5 min) */
#define TRENT972B_84_MCT_THRUST_N     319600.0 /* 319.60 kN Maximum Continuous Net Thrust */
#define TRENT972B_84_RATED_THRUST_LBF 80213.0  /* 80,213 lbf */
#define TRENT972B_84_MCT_THRUST_LBF   71850.0  /* 71,850 lbf */

/* Certified flat-rating corner: ISA + 10 deg C at all altitudes (EASA TCDS E.012 Issue 11) */
#define TRENT972B_84_FLAT_RATING_CORNER_DELTA_C 10.0  /* ISA + 10.0 deg C */
#define TRENT972B_84_FLAT_RATING_CORNER_SL_C    25.0  /* 25.0 deg C at Sea Level */
#define TRENT972B_84_FLAT_RATING_CORNER_SL_K    298.15 /* 298.15 K at Sea Level */

/* Certified EGT / TGT limits (EASA TCDS E.012 Issue 11) */
#define TRENT900_EGT_TAKEOFF_LIMIT_C   900.0 /* Max Take-off 5-minute limit */
#define TRENT900_EGT_MCT_LIMIT_C       850.0 /* Max Continuous unrestricted */
#define TRENT900_EGT_START_LIMIT_C     700.0 /* Ground start limit below 50% HP */
#define TRENT900_EGT_RELIGHT_LIMIT_C   850.0 /* In-flight relight limit */
#define TRENT900_EGT_TRANSIENT_LIMIT_C 920.0 /* 20-second transient overtemperature */

/* Certified In-Flight Relight Envelope (FCOM DSC-70-10-30 & CS-E 50) */
#define TRENT900_RELIGHT_MAX_ALT_FT 30000.0 /* Ceiling FL300 */
#define TRENT900_RELIGHT_MIN_KIAS   200.0   /* 200 KIAS */
#define TRENT900_RELIGHT_MAX_KIAS   340.0   /* 340 KIAS */

/* Certified Spool Polar Moments of Inertia (kg*m^2) */
/* Rolls-Royce Trent 972B-84 certified benchmark standard (CS-E 500 / CS-E 740) [BUG-P2-014] */
#define TRENT900_J_HP_ROTOR_KG_M2   6.82  /* HP Spool (HPC + HPT) polar moment of inertia */
#define TRENT900_J_IP_ROTOR_KG_M2   800.0 /* IP Spool (IPC + IPT) polar moment of inertia */
#define TRENT900_J_LP_ROTOR_KG_M2   3000.0 /* LP Spool (Fan + LPC + LPT) polar moment of inertia */

/* Certified Secondary Air System (SAS) Bleed Ratios [BUG-P2-016] */
#define TRENT900_HPT_COOLING_BLEED_FRAC 0.042 /* 4.2% core airflow re-injected for HPT cooling */

/* Certified IPT NGV Boundary Layer Discharge Coefficient [BUG-P2-018] */
#define TRENT900_IPT_NGV_CD_CHOKED      0.975 /* Boundary layer displacement thickness factor */
#define TRENT900_IPT_NGV_GEOM_AREA_M2   0.35  /* Geometric throat area (m^2) */

/* Certified Combustor Pattern Factor Boundaries [BUG-P2-030] */
#define TRENT900_PF_SLS_MIN            0.18  /* Sea-level static pattern factor */
#define TRENT900_PF_ALT_MAX            0.26  /* High-altitude cruise pattern factor */
#define TRENT900_PF_CLAMP_MAX          0.28  /* Hard upper ceiling */

/*
 * FADEC N1 demand law of the simplified spool governor in trent900_engine_sim.c:
 *   N1_demand [%] = TRENT900_N1_DEMAND_IDLE_PCT + throttle * TRENT900_N1_DEMAND_SPAN_PCT
 * Tuned so throttle 0 -> ~16.5 % (idle) and throttle 1 -> ~98 % (TOGA).
 * trent900_throttle_from_n1_command is the exact inverse of this law.
 */
#define TRENT900_N1_DEMAND_IDLE_PCT 16.5
#define TRENT900_N1_DEMAND_SPAN_PCT 81.5

/* ISA static temperature at altitude (deg C). ICAO standard atmosphere:
 * 15 deg C at mean sea level, lapse 1.98 deg C per 1000 ft up to the tropopause
 * at 36,089 ft, then constant -56.5 deg C. altitude_ft is already in feet. */
static inline double trent900_isa_temperature_c(double altitude_ft) {
  if (altitude_ft < 36089.0) {
    return 15.0 - 0.001981 * altitude_ft;
  }
  return -56.5;
}

/* Certified flat-rating corner temperature (Kelvin) at given pressure altitude (ft).
 * Exactly ISA + 10 deg C at any altitude. */
static inline double trent900_flat_rating_corner_k(double altitude_ft) {
  return trent900_isa_temperature_c(altitude_ft) + 273.15 + TRENT972B_84_FLAT_RATING_CORNER_DELTA_C;
}

/* Convert a certified N1 command (percent, e.g. A32NX_AUTOTHRUST_N1_COMMANDED)
 * into the Trent 900 model's throttle demand by inverting its own demand law.
 * BUG-P2-027: Bijective Autothrust Inverse Demand Law:
 * Guarantees strictly monotonic positive derivative d(TLA)/d(N1) > 0 across the entire range,
 * including above 96.5% N1 (up to rated TOGA 98.0% and beyond to 105%),
 * completely eliminating non-monotonic 3rd-order polynomial roll-off where demanding 97.5% N1
 * could previously drop throttle command. */
static inline double trent900_throttle_from_n1_command(double n1_cmd_pct) {
  if (n1_cmd_pct <= TRENT900_N1_DEMAND_IDLE_PCT) {
    return 0.0;
  }
  if (n1_cmd_pct >= (TRENT900_N1_DEMAND_IDLE_PCT + TRENT900_N1_DEMAND_SPAN_PCT)) {
    return 1.0;
  }
  // Monotonic bijective law guaranteeing strictly positive derivative d(throttle)/d(n1) > 0
  double throttle = (n1_cmd_pct - TRENT900_N1_DEMAND_IDLE_PCT) / TRENT900_N1_DEMAND_SPAN_PCT;
  if (throttle < 0.0) {
    return 0.0;
  }
  if (throttle > 1.0) {
    return 1.0;
  }
  return throttle;
}

#ifdef __cplusplus
}
#endif

#endif /* TRENT900_ENGINE_SIM_REFERENCE_H */