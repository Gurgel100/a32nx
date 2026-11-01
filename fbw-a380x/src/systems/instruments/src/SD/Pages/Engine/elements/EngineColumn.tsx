import { useSimVar } from '@instruments/common/simVars';
import { EngineNumber, IgnitionActive, Position } from '@instruments/common/types';
import React, { FC } from 'react';
import DecimalValues from './DecimalValues';
import IgnitionBorder from './IgnitionBorder';
import NacelleTemperatureGauge from './NacelleTemperatureGauge';
import OilPressureGauge from './OilPressureGauge';
import OilQuantityGauge from './OilQuantityGauge';
import StartValve from './StartValve';
import { NXUnits } from '@flybywiresim/fbw-sdk-react';

interface EngineColumnProps {
  anyEngineRunning: boolean;
  useMetric: boolean;
}

const EngineColumn: FC<Position & EngineNumber & IgnitionActive & EngineColumnProps> = ({
  x,
  y,
  engine,
  ignition,
  anyEngineRunning,
}) => {
  const [N2] = useSimVar(`L:A32NX_ENGINE_N2:${engine}`, 'number', 100);
  const [N3] = useSimVar(`L:A32NX_ENGINE_N3:${engine}`, 'number', 100);
  const [starterValveOpen] = useSimVar(`L:A32NX_PNEU_ENG_${engine}_STARTER_VALVE_OPEN`, 'number', 500);
  const isStarterValveOpen = !!(starterValveOpen && starterValveOpen > 0.5);
  const starting = !!(N3 < 58.5 && (ignition || isStarterValveOpen));
  const [fadecManuallyPowered] = useSimVar(`L:A32NX_OVHD_FADEC_${engine}`, 'bool', 500);
  const [bridgeFadecPowered] = useSimVar(`L:A32NX_FADEC_POWERED_ENG:${engine}`, 'number', 100);
  const [engineFirePbReleased] = useSimVar(`L:A32NX_FIRE_BUTTON_ENG${engine}`, 'bool', 500);

  // Falls back to the manual FADEC override whenever no bridge telemetry is written (0/undefined)
  const fadecPowered =
    (ignition ||
      anyEngineRunning ||
      (bridgeFadecPowered !== undefined && bridgeFadecPowered > 0 ? bridgeFadecPowered > 0.5 : fadecManuallyPowered)) &&
    !engineFirePbReleased;

  const [fuelFlow] = useSimVar(`L:A32NX_ENGINE_FF:${engine}`, 'number', 100);

  const [n1VibLVar] = useSimVar(`L:A380X_ENG_VIB_N1:${engine}`, 'number', 100);
  const [n2VibLVar] = useSimVar(`L:A380X_ENG_VIB_N2:${engine}`, 'number', 100);
  const [n3VibLVar] = useSimVar(`L:A380X_ENG_VIB_N3:${engine}`, 'number', 100);
  const [turbEngVib] = useSimVar(`TURB ENG VIBRATION:${engine}`, 'number', 100);

  const n1Vibration = n1VibLVar !== undefined && n1VibLVar > 0 ? n1VibLVar : turbEngVib || 0.0;
  const n2Vibration = n2VibLVar !== undefined && n2VibLVar > 0 ? n2VibLVar : turbEngVib || 0.0;
  const n3Vibration = n3VibLVar !== undefined && n3VibLVar > 0 ? n3VibLVar : turbEngVib || 0.0;

  const [rawOilQty] = useSimVar(`L:A38X_TRENT_OIL_QTY:${engine}`, 'number', 500);
  const [altOilQty] = useSimVar(`L:A32NX_ENGINE_OIL_QTY:${engine}`, 'number', 500);
  const oilQuantity =
    typeof rawOilQty === 'number' && !isNaN(rawOilQty) && rawOilQty > 0
      ? rawOilQty
      : typeof altOilQty === 'number' && !isNaN(altOilQty)
        ? altOilQty
        : 0;

  const [rawOilTemp] = useSimVar(`L:A380X_ENG_OIL_TEMP_C:${engine}`, 'number', 100);
  const [genOilTemp] = useSimVar(`GENERAL ENG OIL TEMPERATURE:${engine}`, 'celsius', 100);
  const engineOilTemperature =
    typeof rawOilTemp === 'number' && !isNaN(rawOilTemp) && rawOilTemp > 0 ? rawOilTemp : genOilTemp || 0;

  const [rawNacelleTemp] = useSimVar(`L:A380X_ENG_${engine}_NACELLE_TEMP_C`, 'number', 500);
  const [altNacelleTemp] = useSimVar(`L:A380X_ENG_NACELLE_TEMP_C:${engine}`, 'number', 500);
  const nacelleTemp =
    typeof rawNacelleTemp === 'number' && !isNaN(rawNacelleTemp) && rawNacelleTemp > 0
      ? rawNacelleTemp
      : typeof altNacelleTemp === 'number' && !isNaN(altNacelleTemp) && altNacelleTemp > 0
        ? altNacelleTemp
        : 20.0;

  const showStartValve = isStarterValveOpen || (ignition && N3 < 58.5);

  return (
    <>
      <IgnitionBorder x={x} y={y} engine={engine} ignition={ignition} />
      {/* N2 */}
      <DecimalValues x={x} y={y} value={N2} active={fadecPowered} />
      <path className="White SW2" d={`M${engine > 2 ? x - 96 : x + 64},${y - 10} l 26, 0`} />
      {/* N3 */}
      <rect x={x - 55} y={y + 10} width={98} height={34} className={`LightGreyBox ${starting ? 'Show' : 'Hide'}`} />
      <DecimalValues x={x} y={y + 38} value={N3} active={fadecPowered} />
      <path className="White SW2" d={`M${engine > 2 ? x - 96 : x + 64},${y + 28} l 26, 0`} />
      {/* Fuel Flow */}
      {!fadecPowered && (
        <text x={x} y={y + 86} className="Amber F29 MiddleAlign">
          XX
        </text>
      )}
      {fadecPowered && (
        <text x={x + 30} y={y + 92} className="Green EndAlign F29">
          {Math.ceil(NXUnits.kgToUser(fuelFlow) / 10) * 10}
        </text>
      )}
      {/* OIL */}
      <OilQuantityGauge x={x} y={y + 206} engine={engine} active={fadecPowered} value={oilQuantity} />
      <DecimalValues x={x} y={y + 206} value={oilQuantity} active={fadecPowered} />
      {!fadecPowered && (
        <text x={x} y={y + 242} className="Amber F29 MiddleAlign">
          XX
        </text>
      )}
      {fadecPowered && (
        <text x={x + 20} y={y + 248} className={`${engineOilTemperature > 177 ? 'Amber' : 'Green'} EndAlign F29`}>
          {engineOilTemperature < 0 ? 0 : Math.round(engineOilTemperature)}
        </text>
      )}
      <path className="White SW2" d={`M${engine > 2 ? x - 96 : x + 64},${y + 82} l 26, 0`} />
      {/* Oil Pressure */}
      <OilPressureGauge x={x} y={y + 320} engine={engine} active={fadecPowered} />
      {/* VIB N1 */}
      <DecimalValues x={x} y={y + 380} value={n1Vibration} active={fadecPowered} shift={-14} />
      <path className="White SW2" d={`M${engine > 2 ? x - 96 : x + 64},${y + 370} l 26, 0`} />
      {/* VIB N2 */}
      <DecimalValues x={x} y={y + 416} value={n2Vibration} active={fadecPowered} shift={-14} />
      <path className="White SW2" d={`M${engine > 2 ? x - 96 : x + 64},${y + 406} l 26, 0`} />
      {/* VIB N3 */}
      <DecimalValues x={x} y={y + 450} value={n3Vibration} active={fadecPowered} shift={-14} />
      <path className="White SW2" d={`M${engine > 2 ? x - 96 : x + 64},${y + 440} l 26, 0`} />

      {/* NAC / Ignition */}
      {showStartValve && <StartValve x={x} y={y + 536} engine={engine} />}
      {!showStartValve && (
        <NacelleTemperatureGauge x={x} y={y + 536} engine={engine} active={fadecPowered} value={nacelleTemp} />
      )}
    </>
  );
};

export default EngineColumn;
