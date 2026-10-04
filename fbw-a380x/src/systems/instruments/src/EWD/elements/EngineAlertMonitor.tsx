// Copyright (c) 2024-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
import {
  DisplayComponent,
  Subscribable,
  VNode,
  FSComponent,
  EventBus,
  ConsumerSubject,
  MappedSubject,
} from '@microsoft/msfs-sdk';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';

interface EngineAlertMonitorProps {
  bus: EventBus;
  x: number;
  y: number;
  engine: number;
  active: Subscribable<boolean>;
}

export class EngineAlertMonitor extends DisplayComponent<EngineAlertMonitorProps> {
  private readonly sub = this.props.bus.getSubscriber<EwdSimvars>();

  private readonly oilPress = ConsumerSubject.create(
    this.sub.on(`oil_press_${this.props.engine}`).withPrecision(1).whenChanged(),
    48.0,
  );

  private readonly oilTemp = ConsumerSubject.create(
    this.sub.on(`oil_temp_${this.props.engine}`).withPrecision(1).whenChanged(),
    75.0,
  );

  private readonly vibN1 = ConsumerSubject.create(
    this.sub.on(`vib_n1_${this.props.engine}`).withPrecision(1).whenChanged(),
    0.5,
  );

  private readonly vibN2 = ConsumerSubject.create(
    this.sub.on(`vib_n2_${this.props.engine}`).withPrecision(1).whenChanged(),
    0.5,
  );

  private readonly vibN3 = ConsumerSubject.create(
    this.sub.on(`vib_n3_${this.props.engine}`).withPrecision(1).whenChanged(),
    0.5,
  );

  private readonly n3 = ConsumerSubject.create(
    this.sub.on(`n3_${this.props.engine}`).withPrecision(1).whenChanged(),
    0.0,
  );

  private readonly alertText = MappedSubject.create(
    ([oilP, oilT, n3, v1, v2, v3]) => {
      if (n3 > 45.0 && oilP < 25.0) return 'OIL LO PR';
      if (oilT > 165.0) return 'OIL HI T';
      if (v1 > 2.5) return 'VIB N1';
      if (v2 > 2.5) return 'VIB N2';
      if (v3 > 2.5) return 'VIB N3';
      return '';
    },
    this.oilPress,
    this.oilTemp,
    this.n3,
    this.vibN1,
    this.vibN2,
    this.vibN3,
  );

  private readonly alertVisible = this.alertText.map((txt) => (txt.length > 0 ? 'visible' : 'hidden'));

  public onAfterRender(node: VNode): void {
    super.onAfterRender(node);
    this.subscriptions.push(this.oilPress, this.oilTemp, this.vibN1, this.vibN2, this.vibN3, this.n3);
  }

  render(): VNode {
    const x = this.props.x;
    const y = this.props.y + 168;

    return (
      <g id={`Engine-Alert-Monitor-${this.props.engine}`} visibility={this.alertVisible}>
        <text class="Amber F19 Center PulseAmberInfinite" x={x} y={y}>
          {this.alertText}
        </text>
      </g>
    );
  }
}
