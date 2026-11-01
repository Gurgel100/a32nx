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

interface TrentSpoolGaugeProps {
  bus: EventBus;
  x: number;
  y: number;
  engine: number;
  active: Subscribable<boolean>;
}

export class TrentSpoolGauge extends DisplayComponent<TrentSpoolGaugeProps> {
  private readonly sub = this.props.bus.getSubscriber<EwdSimvars>();

  private readonly n2 = ConsumerSubject.create(
    this.sub.on(`n2_${this.props.engine}`).withPrecision(1).whenChanged(),
    0,
  );

  private readonly n3 = ConsumerSubject.create(
    this.sub.on(`n3_${this.props.engine}`).withPrecision(1).whenChanged(),
    0,
  );

  private readonly starterValveOpen = ConsumerSubject.create(
    this.sub.on(`starter_valve_${this.props.engine}`).whenChanged(),
    false,
  );

  private readonly n2Formatted = this.n2.map((val) => {
    const v = typeof val === 'number' && !isNaN(val) ? val : 0;
    return v.toFixed(1);
  });

  private readonly n3Formatted = this.n3.map((val) => {
    const v = typeof val === 'number' && !isNaN(val) ? val : 0;
    return v.toFixed(1);
  });

  // N3 start motoring box indication: active as soon as starter valve opens until cutout (BUG-P5-007)
  private readonly startingBoxVisible = MappedSubject.create(
    ([n3, sv]) => sv && n3 < 58.5,
    this.n3,
    this.starterValveOpen,
  ).map((s) => (s ? 'LightGreyBox Show' : 'LightGreyBox Hide'));

  render(): VNode {
    const x = this.props.x;
    const y = this.props.y + 148;

    return (
      <g id={`Trent-Spool-Gauge-${this.props.engine}`}>
        {/* Inactive display (XX) */}
        <g visibility={this.props.active.map((it) => (it ? 'hidden' : 'visible'))}>
          <text class="F19 End Amber Spread" x={x + 40} y={y}>
            XX
          </text>
        </g>

        {/* Active display (N2 & N3) */}
        <g visibility={this.props.active.map((it) => (it ? 'visible' : 'hidden'))}>
          {/* Light grey box on N3 during start motoring */}
          <rect x={x + 4} y={y - 16} width={44} height={20} class={this.startingBoxVisible} />

          {/* N2 label and value */}
          <text class="White F19" x={x - 46} y={y}>
            N2
          </text>
          <text class="Green F21 End" x={x - 2} y={y}>
            {this.n2Formatted}
          </text>

          {/* N3 label and value */}
          <text class="White F19" x={x + 6} y={y}>
            N3
          </text>
          <text class="Green F21 End" x={x + 46} y={y}>
            {this.n3Formatted}
          </text>
        </g>
      </g>
    );
  }
}
