// Copyright (c) 2023-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

#include <cstdio>
#include "logging.h"
#ifdef PROFILING
#include "ScopedTimer.hpp"
#include "SimpleProfiler.hpp"
#endif

#include "Fadec.h"

#include "EngineControl_A380X.h"
#include "Polynomials_A380X.hpp"

#include <algorithm>
#include <cmath>

void EngineControl_A380X::initialize(MsfsHandler* msfsHandler) {
  this->msfsHandlerPtr = msfsHandler;
  this->dataManagerPtr = &msfsHandler->getDataManager();
  this->simData.initialize(dataManagerPtr);
  fuelConfiguration.setConfigFilename(FILENAME_FADEC_CONF_DIRECTORY + atcId + FILENAME_FADEC_CONF_FILE_EXTENSION);
  LOG_INFO("Fadec::EngineControl_A380X::initialize() - initialized");
}

void EngineControl_A380X::shutdown() {
  LOG_INFO("Fadec::EngineControl_A380X::shutdown()");
}

void EngineControl_A380X::update() {
#ifdef PROFILING
  profilerUpdate.start();
#endif

  bool isSimulationReady = msfsHandlerPtr->getAircraftIsReadyVar();

  if (!isSimulationReady) {
    // still request atc id so it is ready once the initialization starts
    simData.atcIdDataPtr->requestUpdateFromSim(msfsHandlerPtr->getTimeStamp(), msfsHandlerPtr->getTickCounter());
    return;
  }

  if (!fadecInitialized) {
    loadFuelConfigIfPossible();
    initializeEngineControlData();
    fadecInitialized = true;
  }

  const double deltaTime          = std::max(0.002, msfsHandlerPtr->getSimulationDeltaTime());
  const double mach               = simData.simVarsDataPtr->data().airSpeedMach;
  const double pressureAltitude   = simData.simVarsDataPtr->data().pressureAltitude;
  const double ambientTemperature = simData.simVarsDataPtr->data().ambientTemperature;
  const double ambientPressure    = simData.simVarsDataPtr->data().ambientPressure;

  // Atmospheric & flight environment for the Trent 900 cycle
  FBW_A380_EnvironmentInputs env;
  env.ambient_static_press_pa  = ambientPressure * 100.0;  // hPa -> Pa
  env.ambient_static_temp_k    = ambientTemperature + 273.15;
  env.true_airspeed_ms         = mach * std::sqrt(1.4 * 287.05 * env.ambient_static_temp_k);
  env.flight_mach              = mach;
  env.pressure_altitude_ft     = pressureAltitude;
  env.angle_of_attack_deg      = 0.0;
  env.crosswind_component_kts  = 0.0;
  env.sim_delta_time_s         = deltaTime;

  // Per-engine cockpit & system inputs
  const bool packBleed = simData.packsState[0]->get() || simData.packsState[1]->get();
  const int  wai       = simData.wingAntiIce->getAsInt64();

  FBW_A380_EngineInputs inputs[FBW_A380_NUM_ENGINES];
  for (int i = 0; i < FBW_A380_NUM_ENGINES; i++) {
    // The Rust reverser system owns the blocker doors and the reverse thrust;
    // hold the bridge at the idle detent so it keeps forward idle during reverse.
    double tla = simData.engineTla[i + 1]->get();
    tla        = std::max(-20.0, std::min(53.0, tla));
    inputs[i].throttle_lever_angle_deg = std::max(tla, 20.0);
    inputs[i].master_switch_on         = simData.simVarsDataPtr->data().engineIgniter[i] != 0;
    inputs[i].fire_pushbutton_released = simData.fireButton[i]->getAsBool();
    inputs[i].starter_valve_open       = simData.simVarsDataPtr->data().engineStarter[i] != 0;
    inputs[i].igniters_armed           = simData.simVarsDataPtr->data().engineIgniter[i] == 2;
    inputs[i].ecs_pack_bleed_active    = packBleed;
    inputs[i].anti_ice_bleed_active    = (simData.simVarsDataPtr->data().engineAntiIce[i] > 0.5) || wai != 0;
    inputs[i].vfg_elec_load_kva        = 0.0;
    inputs[i].fault_flameout           = false;
    inputs[i].fault_egt_sensor_drift   = false;
    inputs[i].fault_p02_probe_loss     = false;
    inputs[i].fault_fuel_boost_pump    = false;
    inputs[i].fault_comp_stall         = false;
    printf("eng %i inputs: tla=%.1lf, ms=%s, fpb=%s, sv=%s, ign=%s, bleed=%s, ai=%s\n",
      i,
      inputs[i].throttle_lever_angle_deg,
      inputs[i].master_switch_on ? "on" : "off",
      inputs[i].fire_pushbutton_released ? "off" : "on",
      inputs[i].starter_valve_open ? "open" : "closed",
      inputs[i].igniters_armed ? "on" : "off",
      inputs[i].ecs_pack_bleed_active ? "on" : "off",
      inputs[i].anti_ice_bleed_active ? "on" : "off"
    );
  }

  // Run the Trent 900 cycle for the whole fleet
  FBW_A380_FleetOutputs outputs;
  FBW_A380_Trent900_Update(&env, inputs, &outputs);

  // Write the cycle results back to the simulator and the ECAM LVars
  for (int engine = 1; engine <= 4; engine++) {
    const int engineIdx = engine - 1;

    EngineState engineState = updateEngineState(engine,
                                                outputs.engines[engineIdx],
                                                inputs[engineIdx].master_switch_on,
                                                inputs[engineIdx].starter_valve_open,
                                                inputs[engineIdx].igniters_armed,
                                                inputs[engineIdx].fire_pushbutton_released,
                                                ambientTemperature,
                                                deltaTime);

    // Quick start / quick shutdown for expedited engine handling in Aircraft Presets
    if (simData.fadecQuickMode->getAsBool()) {
      if (engineState == STARTING || engineState == RESTARTING) {
        LOG_INFO("Fadec::EngineControl_A380X::update() - Quick Start");
        FBW_A380_Trent900_ResetEngine(engineIdx, true);
        FBW_A380_Trent900_Update(&env, inputs, &outputs);
        engineState = ON;
        simData.engineTimer[engineIdx]->set(0);
      } else if (engineState == SHUTTING) {
        LOG_INFO("Fadec::EngineControl_A380X::update() - Quick Shutdown");
        FBW_A380_Trent900_ResetEngine(engineIdx, false);
        FBW_A380_Trent900_Update(&env, inputs, &outputs);
        engineState = OFF;
        simData.engineTimer[engineIdx]->set(0);
      }
    }

    const FBW_A380_EngineOutputs& o = outputs.engines[engineIdx];
    simData.engineState[engineIdx]->set(static_cast<int>(engineState));

    simData.engineN1[engineIdx]->set(o.N1_pct);
    simData.engineN2[engineIdx]->set(o.N2_pct);
    simData.engineN3[engineIdx]->set(o.N3_pct);
    // simData.engineN2[engineIdx]->set(o.N3_pct == 0 ? 0 : o.N3_pct + 0.7);  // A380X convention: N2 carries the HP spool
    simData.engineEgt[engineIdx]->set(o.egt_c);
    simData.engineFF[engineIdx]->set(o.fuel_flow_kgh);

    // The corrected N1/N2 drive the sim's flight model (N2 carries the HP spool, as before)
    simData.engineCorrectedN1DataPtr[engineIdx]->data().correctedN1 = o.N1_pct;
    simData.engineCorrectedN1DataPtr[engineIdx]->writeDataToSim();
    simData.engineCorrectedN3DataPtr[engineIdx]->data().correctedN3 = o.N3_pct;
    simData.engineCorrectedN3DataPtr[engineIdx]->writeDataToSim();

    simData.oilTempDataPtr[engineIdx]->data().oilTemp = o.oil_temperature_c;
    simData.oilTempDataPtr[engineIdx]->writeDataToSim();
    simData.oilPsiDataPtr[engineIdx]->data().oilPsi = o.oil_pressure_psi;
    simData.oilPsiDataPtr[engineIdx]->writeDataToSim();

    updateOilQuantity(engine, o.net_thrust_kn * 1000.0, deltaTime);
  }

  // Idle references & thrust limits (the bridge's detent N1 values)
  simData.engineIdleN1->set(TRENT_IDLE_N1_PCT);
  simData.engineIdleN3->set(TRENT_IDLE_N3_PCT);
  simData.engineIdleEGT->set(TRENT_IDLE_EGT_C);
  simData.engineIdleFF->set(TRENT_IDLE_FF_KGH);
  simData.thrustLimitIdle->set(TRENT_IDLE_N1_PCT);
  simData.thrustLimitClimb->set(TRENT_CLIMB_N1_PCT);
  simData.thrustLimitFlex->set(TRENT_FLEX_N1_PCT);
  simData.thrustLimitMct->set(TRENT_MCT_N1_PCT);
  simData.thrustLimitToga->set(TRENT_TOGA_N1_PCT);

  // Update fuel & tank data
  updateFuel(deltaTime);

#ifdef PROFILING
  profilerUpdate.stop();
  if (msfsHandlerPtr->getTickCounter() % 100 == 0) {
    profilerUpdateFuel.print();
    profilerUpdate.print();
  }
#endif
}

// =====================================================================================================================
// Private methods
// =====================================================================================================================

void EngineControl_A380X::loadFuelConfigIfPossible() {
#ifdef PROFILING
  profilerEnsureFadecIsInitialized.start();
#endif
  const FLOAT64 simTime     = msfsHandlerPtr->getSimulationTime();
  const UINT64  tickCounter = msfsHandlerPtr->getTickCounter();

  if (!hasLoadedFuelConfig) {
    bool isSimulationReady = msfsHandlerPtr->getAircraftIsReadyVar();

    simData.atcIdDataPtr->requestUpdateFromSim(msfsHandlerPtr->getTimeStamp(), tickCounter);

    // we only receive the data one tick later as we request it via simconnect. But it should be enought to only perform the check after
    // isSimulationReady as this is set by the JS instruments after spawn
    if (isSimulationReady) {
      if (simData.atcIdDataPtr->data().atcID[0] != '\0') {
        atcId = simData.atcIdDataPtr->data().atcID;
        LOG_INFO("Fadec::EngineControl_A380X::ensureFadecIsInitialized() - received ATC ID: " + atcId);
        initializeFuelTanks(simTime, tickCounter);
      } else {
        LOG_INFO("Fadec::EngineControl_A380X::ensureFadecIsInitialized() - no ATC ID received, taking default: " + atcId);
      }
      // if ATC ID is empty, we take the default and still set hasLoadedFuelConfig to as it won't change anymore
      hasLoadedFuelConfig = true;
    }
  }
#ifdef PROFILING
  profilerEnsureFadecIsInitialized.stop();
  if (msfsHandlerPtr->getTickCounter() % 100 == 0) {
    profilerEnsureFadecIsInitialized.print();
  }
#endif
}

void EngineControl_A380X::initializeEngineControlData() {
  LOG_INFO("Fadec::EngineControl_A380X::initializeEngineControlData()");

#ifdef PROFILING
  ScopedTimer timer("Fadec::EngineControl_A380X::initializeEngineControlData()");
#endif

  const FLOAT64 timeStamp   = msfsHandlerPtr->getTimeStamp();
  const UINT64  tickCounter = msfsHandlerPtr->getTickCounter();

  // Setting initial Oil Quantity and adding some randomness to it
  std::srand(std::time(0));
  simData.engineOilTotal[E1]->set((std::rand() % (MAX_OIL - MIN_OIL + 1) + MIN_OIL) / 10.0);
  simData.engineOilTotal[E2]->set((std::rand() % (MAX_OIL - MIN_OIL + 1) + MIN_OIL) / 10.0);
  simData.engineOilTotal[E3]->set((std::rand() % (MAX_OIL - MIN_OIL + 1) + MIN_OIL) / 10.0);
  simData.engineOilTotal[E4]->set((std::rand() % (MAX_OIL - MIN_OIL + 1) + MIN_OIL) / 10.0);

  // Setting initial Oil Temperature
  const double ambientTemperature            = simData.simVarsDataPtr->data().ambientTemperature;
  simData.oilTempDataPtr[E1]->data().oilTemp = ambientTemperature;
  simData.oilTempDataPtr[E1]->writeDataToSim();
  simData.oilTempDataPtr[E2]->data().oilTemp = ambientTemperature;
  simData.oilTempDataPtr[E2]->writeDataToSim();
  simData.oilTempDataPtr[E3]->data().oilTemp = ambientTemperature;
  simData.oilTempDataPtr[E3]->writeDataToSim();
  simData.oilTempDataPtr[E4]->data().oilTemp = ambientTemperature;
  simData.oilTempDataPtr[E4]->writeDataToSim();

  // Setting initial Engine State
  simData.engineState[E1]->set(OFF);
  simData.engineState[E2]->set(OFF);
  simData.engineState[E3]->set(OFF);
  simData.engineState[E4]->set(OFF);

  // Setting initial Engine Timer
  simData.engineTimer[E1]->set(0);
  simData.engineTimer[E2]->set(0);
  simData.engineTimer[E3]->set(0);
  simData.engineTimer[E4]->set(0);

  initializeFuelTanks(timeStamp, tickCounter);

  // Setting initial Fuel Flow
  simData.fuelPumpState[E1]->set(0);
  simData.fuelPumpState[E2]->set(0);
  simData.fuelPumpState[E3]->set(0);
  simData.fuelPumpState[E4]->set(0);

  // Setting initial Thrust Limits
  simData.thrustLimitIdle->set(0);
  simData.thrustLimitClimb->set(0);
  simData.thrustLimitFlex->set(0);
  simData.thrustLimitMct->set(0);
  simData.thrustLimitToga->set(0);

  // Initialize the Trent 900 bridge to the Cold & Dark gate state
  FBW_A380_Trent900_Initialize(ambientTemperature);

  // If the sim spawns with already running engines (e.g. a running spawn),
  // start the bridge engines at stabilized ground idle so no start sequence is shown
  for (int i = 0; i < FBW_A380_NUM_ENGINES; i++) {
    if (simData.simVarsDataPtr->data().simEngineN1[i] > 10.0) {
      FBW_A380_Trent900_ResetEngine(i, true);
    }
  }
}

void EngineControl_A380X::initializeFuelTanks(FLOAT64 timeStamp, UINT64 tickCounter) {
  // Setting initial Fuel Levels
  const double weightLbsPerGallon = simData.simVarsDataPtr->data().fuelWeightLbsPerGallon;

  // only loads saved fuel quantity on C/D spawn
  if (simData.startState->updateFromSim(timeStamp, tickCounter) == 2) {
    // Load fuel configuration from file
    fuelConfiguration.setConfigFilename(FILENAME_FADEC_CONF_DIRECTORY + atcId + FILENAME_FADEC_CONF_FILE_EXTENSION);
    fuelConfiguration.loadConfigurationFromIni();

    simData.fuelLeftOuterPre->set(fuelConfiguration.getFuelLeftOuterGallons() * weightLbsPerGallon);
    simData.fuelFeedOnePre->set(fuelConfiguration.getFuelFeedOneGallons() * weightLbsPerGallon);
    simData.fuelLeftMidPre->set(fuelConfiguration.getFuelLeftMidGallons() * weightLbsPerGallon);
    simData.fuelLeftInnerPre->set(fuelConfiguration.getFuelLeftInnerGallons() * weightLbsPerGallon);
    simData.fuelFeedTwoPre->set(fuelConfiguration.getFuelFeedTwoGallons() * weightLbsPerGallon);
    simData.fuelFeedThreePre->set(fuelConfiguration.getFuelFeedThreeGallons() * weightLbsPerGallon);
    simData.fuelRightInnerPre->set(fuelConfiguration.getFuelRightInnerGallons() * weightLbsPerGallon);
    simData.fuelRightMidPre->set(fuelConfiguration.getFuelRightMidGallons() * weightLbsPerGallon);
    simData.fuelFeedFourPre->set(fuelConfiguration.getFuelFeedFourGallons() * weightLbsPerGallon);
    simData.fuelRightOuterPre->set(fuelConfiguration.getFuelRightOuterGallons() * weightLbsPerGallon);
    simData.fuelTrimPre->set(fuelConfiguration.getFuelTrimGallons() * weightLbsPerGallon);

    // set fuel levels from configuration to the sim
    simData.fuelTankDataPtr->data().fuelSystemFeedOne    = fuelConfiguration.getFuelFeedOneGallons();
    simData.fuelTankDataPtr->data().fuelSystemFeedTwo    = fuelConfiguration.getFuelFeedTwoGallons();
    simData.fuelTankDataPtr->data().fuelSystemFeedThree  = fuelConfiguration.getFuelFeedThreeGallons();
    simData.fuelTankDataPtr->data().fuelSystemFeedFour   = fuelConfiguration.getFuelFeedFourGallons();
    simData.fuelTankDataPtr->data().fuelSystemLeftOuter  = fuelConfiguration.getFuelLeftOuterGallons();
    simData.fuelTankDataPtr->data().fuelSystemLeftMid    = fuelConfiguration.getFuelLeftMidGallons();
    simData.fuelTankDataPtr->data().fuelSystemLeftInner  = fuelConfiguration.getFuelLeftInnerGallons();
    simData.fuelTankDataPtr->data().fuelSystemRightInner = fuelConfiguration.getFuelRightInnerGallons();
    simData.fuelTankDataPtr->data().fuelSystemRightMid   = fuelConfiguration.getFuelRightMidGallons();
    simData.fuelTankDataPtr->data().fuelSystemRightOuter = fuelConfiguration.getFuelRightOuterGallons();
    simData.fuelTankDataPtr->data().fuelSystemTrim       = fuelConfiguration.getFuelTrimGallons();
    simData.fuelTankDataPtr->writeDataToSim();
  }
  // on a non C/D spawn, set fuel levels from the sim
  else {
    simData.fuelLeftOuterPre->set(simData.fuelTankDataPtr->data().fuelSystemLeftOuter * weightLbsPerGallon);
    simData.fuelFeedOnePre->set(simData.fuelTankDataPtr->data().fuelSystemFeedOne * weightLbsPerGallon);
    simData.fuelLeftMidPre->set(simData.fuelTankDataPtr->data().fuelSystemLeftMid * weightLbsPerGallon);
    simData.fuelLeftInnerPre->set(simData.fuelTankDataPtr->data().fuelSystemLeftInner * weightLbsPerGallon);
    simData.fuelFeedTwoPre->set(simData.fuelTankDataPtr->data().fuelSystemFeedTwo * weightLbsPerGallon);
    simData.fuelFeedThreePre->set(simData.fuelTankDataPtr->data().fuelSystemFeedThree * weightLbsPerGallon);
    simData.fuelRightInnerPre->set(simData.fuelTankDataPtr->data().fuelSystemRightInner * weightLbsPerGallon);
    simData.fuelRightMidPre->set(simData.fuelTankDataPtr->data().fuelSystemRightMid * weightLbsPerGallon);
    simData.fuelFeedFourPre->set(simData.fuelTankDataPtr->data().fuelSystemFeedFour * weightLbsPerGallon);
    simData.fuelRightOuterPre->set(simData.fuelTankDataPtr->data().fuelSystemRightOuter * weightLbsPerGallon);
    simData.fuelTrimPre->set(simData.fuelTankDataPtr->data().fuelSystemTrim * weightLbsPerGallon);
  }
}

EngineControl_A380X::EngineState EngineControl_A380X::updateEngineState(int    engine,
                                                                        const FBW_A380_EngineOutputs& engineOutputs,
                                                                        bool   masterOn,
                                                                        bool   starterOn,
                                                                        bool   igniterArmed,
                                                                        bool   firePushed,
                                                                        double oatC,
                                                                        double deltaTime) {
  const int engineIdx = engine - 1;

  EngineState engineState = static_cast<EngineState>(simData.engineState[engineIdx]->get());

  const bool fuelCut  = !masterOn || firePushed;
  const bool startCmd = masterOn && (starterOn || igniterArmed);
  const bool running  = engineOutputs.N3_pct >= TRENT_IDLE_N3_PCT - 0.5;
  const bool settled  = engineOutputs.N3_pct < 0.5 && engineOutputs.egt_c <= oatC + 10.0;
  const bool lit      = engineOutputs.fuel_flow_kgs > 0.02;

  bool resetTimer = false;

  // Current State: OFF
  if (engineState == OFF) {
    if (startCmd) {
      engineState = STARTING;
      resetTimer  = true;
    }
    // Current State: ON
  } else if (engineState == ON) {
    if (fuelCut || !lit) {
      engineState = SHUTTING;
      resetTimer  = true;
    }
    // Current State: Starting / Re-Starting
  } else if (engineState == STARTING || engineState == RESTARTING) {
    if (running) {
      engineState = ON;
      resetTimer  = true;
    } else if (fuelCut) {
      engineState = settled ? OFF : SHUTTING;
      resetTimer  = true;
    }
    // Current State: Shutting
  } else if (engineState == SHUTTING) {
    if (startCmd && !running) {
      engineState = RESTARTING;
      resetTimer  = true;
    } else if (settled) {
      engineState = OFF;
      resetTimer  = true;
    }
  }

  if (resetTimer) {
    simData.engineTimer[engineIdx]->set(0);
  } else if (engineState == STARTING || engineState == SHUTTING) {
    simData.engineTimer[engineIdx]->set(simData.engineTimer[engineIdx]->get() + deltaTime);
  }

  return engineState;
}

void EngineControl_A380X::updateOilQuantity(int engine, double thrustN, double deltaTime) {
  const int engineIdx = engine - 1;

  double oilQtyActual   = simData.engineOil[engineIdx]->get();
  double oilTotalActual = simData.engineOilTotal[engineIdx]->get();
  double oilQtyObjective;
  double oilBurn;

  //--------------------------------------------
  // Oil Quantity
  //--------------------------------------------
  // Calculating Oil Qty as a function of thrust
  oilQtyObjective = oilTotalActual * (1 - Polynomial_A380X::oilGulpPct(thrustN));
  oilQtyActual    = oilQtyObjective;

  // Oil burnt taken into account for tank and total oil
  oilBurn        = (0.00011111 * deltaTime);
  oilQtyActual   = oilQtyActual - oilBurn;
  oilTotalActual = oilTotalActual - oilBurn;

  simData.engineOil[engineIdx]->set(oilQtyActual);
  simData.engineOilTotal[engineIdx]->set(oilTotalActual);
}

void EngineControl_A380X::updateFuel(double deltaTimeSeconds) {
#ifdef PROFILING
  profilerUpdateFuel.start();
#endif

  bool uiFuelTamper = false;

  const double engine1PreFF = simData.enginePreFF[E1]->get();
  const double engine2PreFF = simData.enginePreFF[E2]->get();
  const double engine3PreFF = simData.enginePreFF[E3]->get();
  const double engine4PreFF = simData.enginePreFF[E4]->get();

  const double engine1FF = simData.engineFF[E1]->get();  // kg/hour
  const double engine2FF = simData.engineFF[E2]->get();  // kg/hour
  const double engine3FF = simData.engineFF[E3]->get();  // kg/hour
  const double engine4FF = simData.engineFF[E4]->get();  // kg/hour

  /// weight of one gallon of fuel in pounds
  const double weightLbsPerGallon = simData.simVarsDataPtr->data().fuelWeightLbsPerGallon;

  double fuelLeftOuterPre  = simData.fuelLeftOuterPre->get();   // Pounds
  double fuelFeedOnePre    = simData.fuelFeedOnePre->get();     // Pounds
  double fuelLeftMidPre    = simData.fuelLeftMidPre->get();     // Pounds
  double fuelLeftInnerPre  = simData.fuelLeftInnerPre->get();   // Pounds
  double fuelFeedTwoPre    = simData.fuelFeedTwoPre->get();     // Pounds
  double fuelFeedThreePre  = simData.fuelFeedThreePre->get();   // Pounds
  double fuelRightInnerPre = simData.fuelRightInnerPre->get();  // Pounds
  double fuelRightMidPre   = simData.fuelRightMidPre->get();    // Pounds
  double fuelFeedFourPre   = simData.fuelFeedFourPre->get();    // Pounds
  double fuelRightOuterPre = simData.fuelRightOuterPre->get();  // Pounds
  double fuelTrimPre       = simData.fuelTrimPre->get();        // Pounds

  const double extraOneQty   = simData.fuelExtraTankDataPtr->data().fuelSystemExtraOne * weightLbsPerGallon;    // Pounds
  const double extraTwoQty   = simData.fuelExtraTankDataPtr->data().fuelSystemExtraTwo * weightLbsPerGallon;    // Pounds
  const double extraThreeQty = simData.fuelExtraTankDataPtr->data().fuelSystemExtraThree * weightLbsPerGallon;  // Pounds
  const double extraFourQty  = simData.fuelExtraTankDataPtr->data().fuelSystemExtraFour * weightLbsPerGallon;   // Pounds
  const double leftOuterQty  = simData.fuelTankDataPtr->data().fuelSystemLeftOuter * weightLbsPerGallon;        // Pounds
  const double feedOneQty    = simData.fuelTankDataPtr->data().fuelSystemFeedOne * weightLbsPerGallon;          // Pounds
  const double leftMidQty    = simData.fuelTankDataPtr->data().fuelSystemLeftMid * weightLbsPerGallon;          // Pounds
  const double leftInnerQty  = simData.fuelTankDataPtr->data().fuelSystemLeftInner * weightLbsPerGallon;        // Pounds
  const double feedTwoQty    = simData.fuelTankDataPtr->data().fuelSystemFeedTwo * weightLbsPerGallon;          // Pounds
  const double feedThreeQty  = simData.fuelTankDataPtr->data().fuelSystemFeedThree * weightLbsPerGallon;        // Pounds
  const double rightInnerQty = simData.fuelTankDataPtr->data().fuelSystemRightInner * weightLbsPerGallon;       // Pounds
  const double rightMidQty   = simData.fuelTankDataPtr->data().fuelSystemRightMid * weightLbsPerGallon;         // Pounds
  const double feedFourQty   = simData.fuelTankDataPtr->data().fuelSystemFeedFour * weightLbsPerGallon;         // Pounds
  const double rightOuterQty = simData.fuelTankDataPtr->data().fuelSystemRightOuter * weightLbsPerGallon;       // Pounds
  const double trimQty       = simData.fuelTankDataPtr->data().fuelSystemTrim * weightLbsPerGallon;             // Pounds

  const double fuelTotalActual = leftOuterQty + feedOneQty + leftMidQty + leftInnerQty + feedTwoQty + feedThreeQty + rightInnerQty +
                                 rightMidQty + feedFourQty + rightOuterQty + trimQty;  // Pounds
  const double fuelTotalPre = fuelLeftOuterPre + fuelFeedOnePre + fuelLeftMidPre + fuelLeftInnerPre + fuelFeedTwoPre + fuelFeedThreePre +
                              fuelRightInnerPre + fuelRightMidPre + fuelFeedFourPre + fuelRightOuterPre + fuelTrimPre;  // Pounds
  const double deltaFuelRate = std::abs(fuelTotalActual - fuelTotalPre) / (weightLbsPerGallon * deltaTimeSeconds);      // Pounds/ sec

  const EngineState engine1State = static_cast<EngineState>(simData.engineState[E1]->get());
  const EngineState engine2State = static_cast<EngineState>(simData.engineState[E2]->get());
  const EngineState engine3State = static_cast<EngineState>(simData.engineState[E3]->get());
  const EngineState engine4State = static_cast<EngineState>(simData.engineState[E4]->get());

  /// Delta time for this update in hours
  const double deltaTimeHours = deltaTimeSeconds / 3600;

  // Checking for in-game UI Fuel tampering
  const bool   isReadyVar          = msfsHandlerPtr->getAircraftIsReadyVar();
  const double refuelRate          = simData.refuelRate->get();
  const double refuelStartedByUser = simData.refuelStartedByUser->get();
  if ((isReadyVar && !refuelStartedByUser && deltaFuelRate > FUEL_RATE_THRESHOLD) ||
      (isReadyVar && refuelStartedByUser && deltaFuelRate > FUEL_RATE_THRESHOLD && refuelRate < 2)) {
    uiFuelTamper = true;
  }

  //--------------------------------------------
  // Main Fuel Burn Logic
  //--------------------------------------------
  const FLOAT64 aircraftDevelopmentStateVar = msfsHandlerPtr->getAircraftDevelopmentStateVar();

  if (uiFuelTamper && aircraftDevelopmentStateVar == 0) {
    simData.fuelLeftOuterPre->set(fuelLeftOuterPre);    // Pounds
    simData.fuelFeedOnePre->set(fuelFeedOnePre);        // Pounds
    simData.fuelLeftMidPre->set(fuelLeftMidPre);        // Pounds
    simData.fuelLeftInnerPre->set(fuelLeftInnerPre);    // Pounds
    simData.fuelFeedTwoPre->set(fuelFeedTwoPre);        // Pounds
    simData.fuelFeedThreePre->set(fuelFeedThreePre);    // Pounds
    simData.fuelRightInnerPre->set(fuelRightInnerPre);  // Pounds
    simData.fuelRightMidPre->set(fuelRightMidPre);      // Pounds
    simData.fuelFeedFourPre->set(fuelFeedFourPre);      // Pounds
    simData.fuelRightOuterPre->set(fuelRightOuterPre);  // Pounds
    simData.fuelTrimPre->set(fuelTrimPre);              // Pounds

    simData.fuelTankDataPtr->data().fuelSystemFeedOne   = fuelFeedOnePre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemFeedTwo   = fuelFeedTwoPre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemFeedThree = fuelFeedThreePre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemFeedFour  = fuelFeedFourPre / weightLbsPerGallon;

    simData.fuelTankDataPtr->data().fuelSystemLeftOuter  = fuelLeftOuterPre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemLeftMid    = fuelLeftMidPre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemLeftInner  = fuelLeftInnerPre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemRightInner = fuelRightInnerPre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemRightMid   = fuelRightMidPre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemRightOuter = fuelRightOuterPre / weightLbsPerGallon;
    simData.fuelTankDataPtr->data().fuelSystemTrim       = fuelTrimPre / weightLbsPerGallon;
    simData.fuelTankDataPtr->writeDataToSim();
  }
  // Detects refueling from the EFB
  else if (!uiFuelTamper && refuelStartedByUser == 1) {
    simData.fuelLeftOuterPre->set(leftOuterQty);
    simData.fuelFeedOnePre->set(feedOneQty);
    simData.fuelLeftMidPre->set(leftMidQty);
    simData.fuelLeftInnerPre->set(leftInnerQty);
    simData.fuelFeedTwoPre->set(feedTwoQty);
    simData.fuelFeedThreePre->set(feedThreeQty);
    simData.fuelRightInnerPre->set(rightInnerQty);
    simData.fuelRightMidPre->set(rightMidQty);
    simData.fuelFeedFourPre->set(feedFourQty);
    simData.fuelRightOuterPre->set(rightOuterQty);
    simData.fuelTrimPre->set(trimQty);
  } else {
    if (uiFuelTamper) {
      fuelLeftOuterPre  = leftOuterQty;   // in Pounds
      fuelFeedOnePre    = feedOneQty;     // in Pounds
      fuelLeftMidPre    = leftMidQty;     // in Pounds
      fuelLeftInnerPre  = leftInnerQty;   // in Pounds
      fuelFeedTwoPre    = feedTwoQty;     // in Pounds
      fuelFeedThreePre  = feedThreeQty;   // in Pounds
      fuelRightInnerPre = rightInnerQty;  // in Pounds
      fuelRightMidPre   = rightMidQty;    // in Pounds
      fuelFeedFourPre   = feedFourQty;    // in Pounds
      fuelRightOuterPre = rightOuterQty;  // in Pounds
      fuelTrimPre       = trimQty;        // in Pounds
    }

    double fuelFlowRateChange   = 0;  // was m in the original code
    double previousFuelFlowRate = 0;  // was b in the original code
    double fuelBurn1            = 0;  // in kg
    double fuelBurn2            = 0;  // in kg
    double fuelBurn3            = 0;  // in kg
    double fuelBurn4            = 0;  // in kg

    double fuelUsedEngine1 = simData.engineFuelUsed[E1]->get();
    double fuelUsedEngine2 = simData.engineFuelUsed[E2]->get();
    double fuelUsedEngine3 = simData.engineFuelUsed[E3]->get();
    double fuelUsedEngine4 = simData.engineFuelUsed[E4]->get();

    // Initialize arrays to avoid code duplication when looping over engines
    const double* engineFF[4]       = {&engine1FF, &engine2FF, &engine3FF, &engine4FF};
    const double* enginePreFF[4]    = {&engine1PreFF, &engine2PreFF, &engine3PreFF, &engine4PreFF};
    const double* fuelExtraQty[4]   = {&extraOneQty, &extraTwoQty, &extraThreeQty, &extraFourQty};
    double*       fuelBurn[4]       = {&fuelBurn1, &fuelBurn2, &fuelBurn3, &fuelBurn4};
    double*       fuelUsedEngine[4] = {&fuelUsedEngine1, &fuelUsedEngine2, &fuelUsedEngine3, &fuelUsedEngine4};

    // Loop over engines
    for (int i = 0; i < 4; i++) {
      // Engines fuel burn routine
      if (*fuelExtraQty[i] > 0) {
        // Cycle Fuel Burn
        if (aircraftDevelopmentStateVar != 2 && msfsHandlerPtr->getPauseState() == 0) {
          fuelFlowRateChange   = (*engineFF[i] - *enginePreFF[i]) / deltaTimeHours;
          previousFuelFlowRate = *enginePreFF[i];
          *fuelBurn[i]         = std::min((fuelFlowRateChange * std::pow(deltaTimeHours, 2) / 2) + (previousFuelFlowRate * deltaTimeHours),
                                          *fuelExtraQty[i]);  // KG, limits fuelburn to remaining tank qty
        }
        // Fuel Used Accumulators
        *fuelUsedEngine[i] += *fuelBurn[i];
      }
    }

    const double fuelExtraOne   = std::max(extraOneQty - (fuelBurn1 * Fadec::KGS_TO_LBS), 0.0);    // Pounds
    const double fuelExtraTwo   = std::max(extraTwoQty - (fuelBurn2 * Fadec::KGS_TO_LBS), 0.0);    // Pounds
    const double fuelExtraThree = std::max(extraThreeQty - (fuelBurn3 * Fadec::KGS_TO_LBS), 0.0);  // Pounds
    const double fuelExtraFour  = std::max(extraFourQty - (fuelBurn4 * Fadec::KGS_TO_LBS), 0.0);   // Pounds

    // Setting new pre-cycle conditions
    simData.enginePreFF[E1]->set(engine1FF);
    simData.enginePreFF[E2]->set(engine2FF);
    simData.enginePreFF[E3]->set(engine3FF);
    simData.enginePreFF[E4]->set(engine4FF);

    simData.engineFuelUsed[E1]->set(fuelUsedEngine1);
    simData.engineFuelUsed[E2]->set(fuelUsedEngine2);
    simData.engineFuelUsed[E3]->set(fuelUsedEngine3);
    simData.engineFuelUsed[E4]->set(fuelUsedEngine4);

    simData.fuelFeedOnePre->set(feedOneQty);
    simData.fuelFeedTwoPre->set(feedTwoQty);
    simData.fuelFeedThreePre->set(feedThreeQty);
    simData.fuelFeedFourPre->set(feedFourQty);

    simData.fuelLeftOuterPre->set(leftOuterQty);
    simData.fuelLeftMidPre->set(leftMidQty);
    simData.fuelLeftInnerPre->set(leftInnerQty);
    simData.fuelRightInnerPre->set(rightInnerQty);
    simData.fuelRightMidPre->set(rightMidQty);
    simData.fuelRightOuterPre->set(rightOuterQty);
    simData.fuelTrimPre->set(trimQty);

    simData.fuelExtraTankDataPtr->data().fuelSystemExtraOne   = (fuelExtraOne / weightLbsPerGallon);
    simData.fuelExtraTankDataPtr->data().fuelSystemExtraTwo   = (fuelExtraTwo / weightLbsPerGallon);
    simData.fuelExtraTankDataPtr->data().fuelSystemExtraThree = (fuelExtraThree / weightLbsPerGallon);
    simData.fuelExtraTankDataPtr->data().fuelSystemExtraFour  = (fuelExtraFour / weightLbsPerGallon);
    simData.fuelExtraTankDataPtr->writeDataToSim();
  }

  // Will save the current fuel quantities if the aircraft is on the ground AND engines being shutdown
  // AND 5 seconds have passed since the last save
  if (msfsHandlerPtr->getSimOnGround() && (msfsHandlerPtr->getSimulationTime() - lastFuelSaveTime) > 5.0 &&
      (engine1State == OFF || engine1State == SHUTTING ||  // 1
       engine2State == OFF || engine2State == SHUTTING ||  // 2
       engine3State == OFF || engine3State == SHUTTING ||  // 3
       engine4State == OFF || engine4State == SHUTTING)    // 4
  ) {
    fuelConfiguration.setFuelLeftOuterGallons(simData.fuelTankDataPtr->data().fuelSystemLeftOuter);
    fuelConfiguration.setFuelFeedOneGallons(simData.fuelTankDataPtr->data().fuelSystemFeedOne);
    fuelConfiguration.setFuelLeftMidGallons(simData.fuelTankDataPtr->data().fuelSystemLeftMid);
    fuelConfiguration.setFuelLeftInnerGallons(simData.fuelTankDataPtr->data().fuelSystemLeftInner);
    fuelConfiguration.setFuelFeedTwoGallons(simData.fuelTankDataPtr->data().fuelSystemFeedTwo);
    fuelConfiguration.setFuelFeedThreeGallons(simData.fuelTankDataPtr->data().fuelSystemFeedThree);
    fuelConfiguration.setFuelRightInnerGallons(simData.fuelTankDataPtr->data().fuelSystemRightInner);
    fuelConfiguration.setFuelRightMidGallons(simData.fuelTankDataPtr->data().fuelSystemRightMid);
    fuelConfiguration.setFuelFeedFourGallons(simData.fuelTankDataPtr->data().fuelSystemFeedFour);
    fuelConfiguration.setFuelRightOuterGallons(simData.fuelTankDataPtr->data().fuelSystemRightOuter);
    fuelConfiguration.setFuelTrimGallons(simData.fuelTankDataPtr->data().fuelSystemTrim);
    fuelConfiguration.saveConfigurationToIni();
    lastFuelSaveTime = msfsHandlerPtr->getSimulationTime();
  }

#ifdef PROFILING
  profilerUpdateFuel.stop();
#endif
}
