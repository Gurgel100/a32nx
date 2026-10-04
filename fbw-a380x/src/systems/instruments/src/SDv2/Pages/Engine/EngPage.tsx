// Copyright (c) 2024-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  ConsumerSubject,
  EventBus,
  FSComponent,
  MappedSubject,
  Subject,
  VNode,
} from '@microsoft/msfs-sdk';
import { PageTitle } from '../Generic/PageTitle';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { SdPageProps } from '../../SD';
import { SDSimvars } from '../../SDSimvarPublisher';
import { NXDataStore } from '@flybywiresim/fbw-sdk';
import { fuelForDisplay } from '../../../Common/FuelFunctions';

export class EngPage extends DestroyableComponent<SdPageProps> {
  private readonly sub = this.props.bus.getSubscriber<SDSimvars>();
  private readonly topSvgDisplay = this.props.visible.map((v) => (v ? 'inline' : 'none'));

  private readonly usingMetric = NXDataStore.getSetting('CONFIG_USING_METRIC_UNIT');
  private readonly ffUnitText = this.usingMetric.map((v) => (v ? 'KG/H' : 'LBS/H'));

  // Live engine parameters per engine (1..4)
  private readonly enginesN2 = [Subject.create(0), Subject.create(0), Subject.create(0), Subject.create(0)];
  private readonly enginesN3 = [Subject.create(0), Subject.create(0), Subject.create(0), Subject.create(0)];
  private readonly enginesFF = [Subject.create(0), Subject.create(0), Subject.create(0), Subject.create(0)];
  private readonly enginesOilQty = [Subject.create(24.5), Subject.create(24.5), Subject.create(24.5), Subject.create(24.5)];
  private readonly enginesOilTemp = [Subject.create(85), Subject.create(85), Subject.create(85), Subject.create(85)];
  private readonly enginesOilPsi = [Subject.create(55), Subject.create(55), Subject.create(55), Subject.create(55)];
  private readonly enginesVibN1 = [Subject.create(0.4), Subject.create(0.4), Subject.create(0.4), Subject.create(0.4)];
  private readonly enginesVibN2 = [Subject.create(0.5), Subject.create(0.5), Subject.create(0.5), Subject.create(0.5)];
  private readonly enginesVibN3 = [Subject.create(0.6), Subject.create(0.6), Subject.create(0.6), Subject.create(0.6)];
  private readonly enginesIgn = [Subject.create('A'), Subject.create('A'), Subject.create('A'), Subject.create('A')];
  private readonly enginesStartVlv = [Subject.create(false), Subject.create(false), Subject.create(false), Subject.create(false)];
  private readonly enginesStartPsi = [Subject.create(0), Subject.create(0), Subject.create(0), Subject.create(0)];
  private readonly enginesFadecPowered = [Subject.create(true), Subject.create(true), Subject.create(true), Subject.create(true)];

  private intervalHandle: any = null;

  onAfterRender(node: VNode): void {
    super.onAfterRender(node);

    this.subscriptions.push(this.topSvgDisplay, this.ffUnitText);

    // Continuous 10 Hz telemetry loop from Trent 900 simulation
    this.intervalHandle = setInterval(() => {
      if (!this.props.visible.get()) return;

      const metric = this.usingMetric.get();
      const engModeSel = SimVar.GetSimVarValue('L:XMLVAR_ENG_MODE_SEL', 'number');

      for (let i = 0; i < 4; i++) {
        const engNum = i + 1;
        const n2 = SimVar.GetSimVarValue(`L:A32NX_ENGINE_N2:${engNum}`, 'number');
        const n3 = SimVar.GetSimVarValue(`L:A32NX_ENGINE_N3:${engNum}`, 'number');
        const ff = SimVar.GetSimVarValue(`L:A32NX_ENGINE_FF:${engNum}`, 'number');
        const oilQty = SimVar.GetSimVarValue(`L:A32NX_ENGINE_OIL_QTY:${engNum}`, 'number');
        const oilTemp = SimVar.GetSimVarValue(`GENERAL ENG OIL TEMPERATURE:${engNum}`, 'celsius');
        const oilPsi = SimVar.GetSimVarValue(`GENERAL ENG OIL PRESSURE:${engNum}`, 'psi');
        const vibN1 = SimVar.GetSimVarValue(`L:A380X_ENG_VIB_N1:${engNum}`, 'number');
        const vibN2 = SimVar.GetSimVarValue(`L:A380X_ENG_VIB_N2:${engNum}`, 'number');
        const vibN3 = SimVar.GetSimVarValue(`L:A380X_ENG_VIB_N3:${engNum}`, 'number');
        const ignCh = SimVar.GetSimVarValue(`L:A380X_ENG_IGNITION_CHANNEL:${engNum}`, 'number');
        const startVlv = SimVar.GetSimVarValue(`L:A32NX_PNEU_ENG_${engNum}_STARTER_VALVE_OPEN`, 'bool') !== 0;
        const startPsi = SimVar.GetSimVarValue(`L:A32NX_PNEU_ENG_${engNum}_REGULATED_TRANSDUCER_PRESSURE`, 'number');
        const fadecManual = SimVar.GetSimVarValue(`L:A32NX_OVHD_FADEC_${engNum}`, 'bool');
        const fireReleased = SimVar.GetSimVarValue(`L:A32NX_FIRE_BUTTON_ENG${engNum}`, 'bool');

        const isPowered = ((n3 > 15) || (engModeSel === 2) || (fadecManual !== 0)) && (fireReleased === 0);
        this.enginesFadecPowered[i].set(isPowered);

        this.enginesN2[i].set(n2 > 0 ? n2 : 0);
        this.enginesN3[i].set(n3 > 0 ? n3 : 0);
        this.enginesFF[i].set(ff > 0 ? (metric ? ff : ff * 2.20462) : 0);
        this.enginesOilQty[i].set(oilQty > 0 ? oilQty : 24.5);
        this.enginesOilTemp[i].set(oilTemp > -100 ? oilTemp : 20);
        this.enginesOilPsi[i].set(oilPsi >= 0 ? oilPsi : 0);
        this.enginesVibN1[i].set(vibN1 > 0 ? vibN1 : 0.2);
        this.enginesVibN2[i].set(vibN2 > 0 ? vibN2 : 0.3);
        this.enginesVibN3[i].set(vibN3 > 0 ? vibN3 : 0.4);
        this.enginesIgn[i].set(ignCh === 2 ? 'B' : ignCh === 3 ? 'AB' : 'A');
        this.enginesStartVlv[i].set(startVlv);
        this.enginesStartPsi[i].set(startPsi >= 0 ? startPsi : (startVlv ? 35 : 0));
      }
    }, 100);

    this.cleanups.push(() => {
      if (this.intervalHandle) {
        clearInterval(this.intervalHandle);
        this.intervalHandle = null;
      }
    });
  }

  destroy(): void {
    if (this.intervalHandle) {
      clearInterval(this.intervalHandle);
      this.intervalHandle = null;
    }
    super.destroy();
  }

  private renderEngineColumn(engineIdx: number, x: number) {
    const isPowered = this.enginesFadecPowered[engineIdx];
    const n2Text = this.enginesN2[engineIdx].map((v) => v.toFixed(1));
    const n3Text = this.enginesN3[engineIdx].map((v) => v.toFixed(1));
    const ffText = this.enginesFF[engineIdx].map((v) => (v > 10 ? (Math.round(v / 10) * 10).toFixed(0) : '0'));
    const oilQtyText = this.enginesOilQty[engineIdx].map((v) => v.toFixed(1));
    const oilTempText = this.enginesOilTemp[engineIdx].map((v) => `${Math.round(v)}`);
    const oilPsiText = this.enginesOilPsi[engineIdx].map((v) => `${Math.round(v)}`);
    const vibN1Text = this.enginesVibN1[engineIdx].map((v) => v.toFixed(1));
    const vibN2Text = this.enginesVibN2[engineIdx].map((v) => v.toFixed(1));
    const vibN3Text = this.enginesVibN3[engineIdx].map((v) => v.toFixed(1));
    const ignText = this.enginesIgn[engineIdx];
    const startVlv = this.enginesStartVlv[engineIdx];
    const startPsiText = this.enginesStartPsi[engineIdx].map((v) => `${Math.round(v)}`);

    return (
      <g id={`sdv2-eng-col-${engineIdx + 1}`}>
        {/* N2 */}
        <text x={x} y={68} class="F25 EndAlign Green">{n2Text}</text>
        {/* N3 */}
        <text x={x} y={108} class="F25 EndAlign Green">{n3Text}</text>
        {/* FF */}
        <text x={x} y={158} class="F29 EndAlign Green">{ffText}</text>
        {/* OIL QTY */}
        <text x={x} y={250} class="F25 EndAlign Green">{oilQtyText}</text>
        {/* OIL TEMP */}
        <text x={x} y={290} class="F25 EndAlign Green">{oilTempText}</text>
        {/* OIL PSI */}
        <text x={x} y={350} class="F29 EndAlign Green">{oilPsiText}</text>
        {/* VIBRATION N1 */}
        <text x={x} y={452} class="F25 EndAlign Green">{vibN1Text}</text>
        {/* VIBRATION N2 */}
        <text x={x} y={486} class="F25 EndAlign Green">{vibN2Text}</text>
        {/* VIBRATION N3 */}
        <text x={x} y={520} class="F25 EndAlign Green">{vibN3Text}</text>
        {/* IGN CHANNEL */}
        <text x={x} y={570} class="F25 EndAlign Green">{ignText}</text>
        {/* START VALVE */}
        <circle cx={x - 12} cy={605} r={9} fill="none" stroke="white" strokeWidth="2" />
        <line
          x1={x - 20}
          y1={605}
          x2={x - 4}
          y2={605}
          stroke="rgb(0,255,0)"
          strokeWidth="3"
          transform={startVlv.map((o) => (o ? `rotate(90 ${x - 12} 605)` : ''))}
        />
        {/* START DUCT PSI */}
        <text x={x} y={650} class="F25 EndAlign Green">{startPsiText}</text>
      </g>
    );
  }

  render() {
    return (
      <svg
        version="1.1"
        xmlns="http://www.w3.org/2000/svg"
        xmlnsXlink="http://www.w3.org/1999/xlink"
        viewBox="0 0 768 1024"
        style={{ display: this.topSvgDisplay }}
      >
        <PageTitle x={6} y={29}>
          ENGINE
        </PageTitle>

        {/* Engine 1 */}
        {this.renderEngineColumn(0, 115)}
        {/* Engine 2 */}
        {this.renderEngineColumn(1, 260)}

        {/* Central Parameter Labels */}
        <text x={380} y={68} class="F25 EndAlign White">N2</text>
        <text x={408} y={68} class="F25 EndAlign Cyan">%</text>

        <text x={380} y={108} class="F25 EndAlign White">N3</text>
        <text x={408} y={108} class="F25 EndAlign Cyan">%</text>

        <text x={380} y={158} class="F25 EndAlign White">FF</text>
        <text x={408} y={182} class="F25 EndAlign Cyan">{this.ffUnitText}</text>

        <text x={400} y={225} class="F25 EndAlign White">OIL</text>
        <text x={395} y={250} class="F25 EndAlign Cyan">QT</text>
        <text x={395} y={290} class="F25 EndAlign Cyan">°C</text>
        <text x={400} y={350} class="F25 EndAlign Cyan">PSI</text>

        <text x={375} y={452} class="F25 EndAlign White">VIB</text>
        <text x={415} y={452} class="F25 EndAlign White">N1</text>
        <text x={415} y={486} class="F25 EndAlign White">N2</text>
        <text x={415} y={520} class="F25 EndAlign White">N3</text>

        <text x={415} y={570} class="F25 EndAlign White">IGN</text>
        <text x={415} y={610} class="F25 EndAlign White">START</text>
        <text x={415} y={650} class="F25 EndAlign Cyan">PSI</text>

        {/* Engine 3 */}
        {this.renderEngineColumn(2, 545)}
        {/* Engine 4 */}
        {this.renderEngineColumn(3, 690)}
      </svg>
    );
  }
}
