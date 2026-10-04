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

interface EngineStartStatusProps {
  bus: EventBus;
  x: number;
  y: number;
  engine: number;
  active: Subscribable<boolean>;
}

export class EngineStartStatus extends DisplayComponent<EngineStartStatusProps> {
  private readonly sub = this.props.bus.getSubscriber<EwdSimvars>();

  private readonly starterValveOpen = ConsumerSubject.create(
    this.sub.on(`starter_valve_${this.props.engine}`).whenChanged(),
    false,
  );

  private readonly starterPress = ConsumerSubject.create(
    this.sub.on(`starter_press_${this.props.engine}`).withPrecision(0).whenChanged(),
    0,
  );

  private readonly ignChannel = ConsumerSubject.create(
    this.sub.on(`ign_channel_${this.props.engine}`).whenChanged(),
    0,
  );

  private readonly n3 = ConsumerSubject.create(
    this.sub.on(`n3_${this.props.engine}`).withPrecision(1).whenChanged(),
    0,
  );

  private readonly engineState = ConsumerSubject.create(
    this.sub.on(`engine_state_${this.props.engine}`).whenChanged(),
    0,
  );

  private readonly isVisible = MappedSubject.create(
    ([sv, n3, state]) => !!(sv || (n3 > 0.5 && n3 < 58.5 && state === 2)),
    this.starterValveOpen,
    this.n3,
    this.engineState,
  ).map((v) => (v ? 'visible' : 'hidden'));

  private readonly pressText = this.starterPress.map((p) => {
    const val = p !== undefined && !isNaN(p) && p > 0 ? Math.round(p) : 35;
    return val.toString();
  });

  private readonly pressClass = this.starterPress.map((p) => {
    const val = p !== undefined && !isNaN(p) ? p : 35;
    return val < 15 || val > 60 ? 'Amber F19 End' : 'Green F19 End';
  });

  private readonly ignText = this.ignChannel.map((ch) => {
    if (ch === 2) return 'B';
    if (ch === 3) return 'AB';
    return 'A';
  });

  private readonly valvePath = this.starterValveOpen.map((open) => {
    const cx = this.props.x - 30;
    const cy = this.props.y - 28;
    return open ? `M ${cx} ${cy - 7} l 0 14` : `M ${cx - 7} ${cy} l 14 0`;
  });

  public onAfterRender(node: VNode): void {
    super.onAfterRender(node);
    this.subscriptions.push(this.starterValveOpen, this.starterPress, this.ignChannel, this.n3, this.engineState);
  }

  render(): VNode {
    const cx = this.props.x - 30;
    const cy = this.props.y - 28;

    return (
      <g id={`Engine-Start-Status-${this.props.engine}`} visibility={this.isVisible}>
        {/* Starter valve circle */}
        <circle cx={cx} cy={cy} r={8} class="Green SW2 NoFill" />

        {/* Starter valve position line: vertical = open, horizontal = closed */}
        <path class="Green SW2" d={this.valvePath} />

        {/* Igniter channel (A / B / AB) */}
        <text class="Green F19 Center" x={cx} y={cy - 12}>
          {this.ignText}
        </text>

        {/* Duct pressure digits */}
        <text class={this.pressClass} x={this.props.x + 10} y={cy + 6}>
          {this.pressText}
        </text>

        {/* PSI unit */}
        <text class="Cyan F19" x={this.props.x + 14} y={cy + 6}>
          PSI
        </text>
      </g>
    );
  }
}
