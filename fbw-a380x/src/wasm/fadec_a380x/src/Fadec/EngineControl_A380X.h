// Copyright (c) 2023-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

#ifndef FLYBYWIRE_AIRCRAFT_ENGINECONTROL_A380X_H
#define FLYBYWIRE_AIRCRAFT_ENGINECONTROL_A380X_H

#include "MsfsHandler.h"

#include "FadecSimData_A380X.hpp"
#include "FuelConfiguration_A380X.h"
#include "Trent900/fbw_a380_trent900_bridge.h"

#define FILENAME_FADEC_CONF_DIRECTORY "\\work\\AircraftStates\\"
#define FILENAME_FADEC_CONF_FILE_EXTENSION ".ini"

/**
 * @class EngineControl_A380X
 * @brief Manages the engine control for the A380X aircraft.
 *
 * The Rolls-Royce Trent 900 aerothermodynamic cycle, FADEC and start/shutdown
 * sequencing are computed by the WASM bridge in Trent900/fbw_a380_trent900_bridge.c
 * (FBW_A380_Trent900_Update). This class reads the cockpit and environmental
 * inputs, runs the bridge once per frame and writes the results back to the
 * simulator (corrected N1/N2) and to the ECAM LVars.
 * The fuel tank simulation (fuel burn, refueling, persistence) is still
 * handled by this class.
 */
class EngineControl_A380X {
 private:
  // Convenience pointer to the msfs handler
  MsfsHandler* msfsHandlerPtr = nullptr;

  // Convenience pointer to the data manager
  DataManager* dataManagerPtr = nullptr;

  // FADEC simulation data
  FadecSimData_A380X simData{};

  // ATC ID for the aircraft used to load and store the fuel levels
  std::string atcId = "A380X";

  // Whether we have already loaded the fuel configuration from the config file
  bool hasLoadedFuelConfig = false;

  bool fadecInitialized = false;

  // Fuel configuration for loading and storing fuel levels
  FuelConfiguration_A380X fuelConfiguration{};

  // Remember last fuel save time to allow saving fuel only every 5 seconds
  FLOAT64                 lastFuelSaveTime   = 0;
  static constexpr double FUEL_SAVE_INTERVAL = 5.0;  // seconds

  // Trent 900 bridge reference values (see the TLA map in the bridge)
  static constexpr double TRENT_IDLE_N1_PCT  = 17.52;  // % N1 ground idle
  static constexpr double TRENT_IDLE_N3_PCT  = 60.0;   // % N3 ground idle
  static constexpr double TRENT_IDLE_EGT_C   = 380.0;  // C
  static constexpr double TRENT_IDLE_FF_KGH  = 558.0;  // kg/h (0.155 kg/s)
  static constexpr double TRENT_CLIMB_N1_PCT = 76.0;   // % N1 at the CLB detent
  static constexpr double TRENT_FLEX_N1_PCT  = 84.0;   // % N1 at the FLX detent
  static constexpr double TRENT_MCT_N1_PCT   = 76.0;   // the bridge has no MCT detent; equals climb
  static constexpr double TRENT_TOGA_N1_PCT  = 88.4;   // % N1 at TOGA

  // additional constants
  static constexpr int    MAX_OIL             = 200;
  static constexpr int    MIN_OIL             = 170;
  static constexpr double FUEL_RATE_THRESHOLD = 661;  // lbs/sec for determining fuel ui tampering

  /**
   * @enum EngineState
   * @brief Enumerates the possible states for the engine state machine.
   *
   * @var OFF The engine is turned off. This is the initial state of the engine.
   * @var ON The engine is turned on and running.
   * @var STARTING The engine is in the process of starting up.
   * @var RESTARTING The engine is in the process of restarting.
   * @var SHUTTING The engine is in the process of shutting down.
   */
  enum EngineState {
    OFF        = 0,
    ON         = 1,
    STARTING   = 2,
    RESTARTING = 3,
    SHUTTING   = 4,
  };

#ifdef PROFILING
  // Profiling for the engine control - can eventually be removed
  SimpleProfiler profilerUpdate{"Fadec::EngineControl_A380X::update()", 100};
  SimpleProfiler profilerEnsureFadecIsInitialized{"Fadec::EngineControl_A380X::ensureFadecIsInitialized()", 100};
  SimpleProfiler profilerUpdateFuel{"Fadec::EngineControl_A380X::updateFuel()", 100};
#endif

  // ===========================================================================
  // Public methods
  // ===========================================================================

 public:
  /**
   * @brief Initializes the EngineControl_A380X class once during the gauge initialization.
   * @param msfsHandler
   */
  void initialize(MsfsHandler* msfsHandler);

  /**
   * @brief Updates the EngineControl_A380X class once per frame.
   */
  void update();

  /**
   * @brief Shuts down the EngineControl_A380X class once during the gauge shutdown.
   */
  void shutdown();

  // ===========================================================================
  // Private methods
  // ===========================================================================

 private:
  /**
   * @brief Initializes the required data for the engine simulation if it has not been initialized
   */
  void loadFuelConfigIfPossible();

  /**
   * @brief Initialize the FADEC and Fuel model
   * This is done after we have retrieved the ATC ID so we can load the fuel levels
   */
  void initializeEngineControlData();

  /**
   * @brief Initializes the fuel tanks based on a default config or the saved state of this livery
   * This method may be called multiple times during initialization
   */
  void initializeFuelTanks(FLOAT64 timeStamp, UINT64 tickCounter);

  /**
   * @brief Derives the engine state (OFF, ON, STARTING, RESTARTING, SHUTTING) from the
   *        Trent 900 bridge outputs and the cockpit inputs and maintains the engine timer.
   *
   * @param engine The engine number (1-4).
   * @param engineOutputs The bridge outputs for this engine.
   * @param masterOn Engine master switch position (OFF/ON/IGN -> ON or IGN).
   * @param starterOn Starter solenoid valve position.
   * @param igniterArmed Ignition switch in IGN position.
   * @param firePushed Fire pushbutton released (LP fuel valve shut).
   * @param oatC Outside air temperature in degrees Celsius.
   * @param deltaTime The time difference since the last update in seconds.
   * @return The current state of the engine.
   * @see EngineState
   */
  EngineState updateEngineState(int    engine,
                                const FBW_A380_EngineOutputs& engineOutputs,
                                bool   masterOn,
                                bool   starterOn,
                                bool   igniterArmed,
                                bool   firePushed,
                                double oatC,
                                double deltaTime);

  /**
   * @brief Updates the oil quantity (FQMS) as a function of thrust.
   * Oil pressure and temperature are provided by the Trent 900 bridge.
   *
   * @param engine The engine number (1-4).
   * @param thrustN The current net thrust of the engine in Newton.
   * @param deltaTime The time difference since the last update in seconds.
   */
  void updateOilQuantity(int engine, double thrustN, double deltaTime);

  /**
   * @brief FBW Fuel Consumption and Tanking. Updates Fuel Consumption with realistic values
   *
   * @param deltaTimeSeconds Frame delta time in seconds
   */
  void updateFuel(double deltaTimeSeconds);
};

#endif  // FLYBYWIRE_AIRCRAFT_ENGINECONTROL_A380X_H
