// Copyright (c) 2024-2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
import { DisplayComponent, Subscribable, VNode, FSComponent, EventBus } from '@microsoft/msfs-sdk';
import { AttentionGetter } from './AttentionGetter';
import { EGT } from './EGT';
import { N1 } from './N1';
import { ThrustGauge } from './ThrustGauge';
import { TrentSpoolGauge } from './TrentSpoolGauge';
import { EngineStartStatus } from './EngineStartStatus';
import { EngineAlertMonitor } from './EngineAlertMonitor';

interface EngineGaugeProps {
  bus: EventBus;
  x: number;
  y: number;
  engine: number;
  active: Subscribable<boolean>;
  n1Degraded: Subscribable<boolean>;
  engineNormalAttentionGettingBox: Subscribable<boolean>;
  engineAbnormalAttentionGettingBox: Subscribable<boolean>;
}

export class EngineGauge extends DisplayComponent<EngineGaugeProps> {
  public onAfterRender(node: VNode): void {
    super.onAfterRender(node);
  }

  render() {
    return (
      <g id={`Engine-Gauge-${this.props.engine}`}>
        <AttentionGetter
          bus={this.props.bus}
          x={this.props.x}
          y={this.props.y}
          engine={this.props.engine}
          active={this.props.active}
          normal={this.props.engineNormalAttentionGettingBox}
          abnormal={this.props.engineAbnormalAttentionGettingBox}
        />
        <EngineStartStatus
          bus={this.props.bus}
          x={this.props.x}
          y={this.props.y}
          engine={this.props.engine}
          active={this.props.active}
        />
        <ThrustGauge
          bus={this.props.bus}
          x={this.props.x + 2}
          y={this.props.y - 3}
          engine={this.props.engine}
          active={this.props.active}
          n1Degraded={this.props.n1Degraded}
        />
        <N1
          bus={this.props.bus}
          x={this.props.x}
          y={this.props.y + 65}
          engine={this.props.engine}
          active={this.props.active}
          n1Degraded={this.props.n1Degraded}
        />
        <TrentSpoolGauge
          bus={this.props.bus}
          x={this.props.x}
          y={this.props.y}
          engine={this.props.engine}
          active={this.props.active}
        />
        <EngineAlertMonitor
          bus={this.props.bus}
          x={this.props.x}
          y={this.props.y}
          engine={this.props.engine}
          active={this.props.active}
        />
        <EGT
          bus={this.props.bus}
          x={this.props.x + 4}
          y={this.props.y + 212}
          engine={this.props.engine}
          active={this.props.active}
        />
      </g>
    );
  }
}
