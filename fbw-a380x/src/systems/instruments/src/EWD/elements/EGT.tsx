import {
  DisplayComponent,
  Subscribable,
  VNode,
  FSComponent,
  EventBus,
  ConsumerSubject,
  Subject,
  MappedSubject,
} from '@microsoft/msfs-sdk';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';
import { GaugeComponent, GaugeMarkerComponent, GaugeMaxEGTComponent } from '../../MsfsAvionicsCommon/gauges';

interface EGTProps {
  bus: EventBus;
  x: number;
  y: number;
  engine: number;
  active: Subscribable<boolean>;
}

export class EGT extends DisplayComponent<EGTProps> {
  private readonly sub = this.props.bus.getSubscriber<EwdSimvars>();

  private readonly throttlePosition = ConsumerSubject.create(
    this.sub.on(`throttle_position_${this.props.engine}`).whenChanged(),
    0,
  );

  private readonly egt = ConsumerSubject.create(
    this.sub.on(`egt_${this.props.engine}`).withPrecision(1).whenChanged(),
    0,
  );

  private readonly starterValveOpen = ConsumerSubject.create(
    this.sub.on(`starter_valve_${this.props.engine}`).whenChanged(),
    false,
  );

  private radius = 68;
  private startAngle = 270;
  private endAngle = 90;
  private min = 0;
  private max = 1000;

  private warningEGTColor = (EGTemperature: number, throttleMode: number, starterValve: boolean) => {
    if (starterValve && EGTemperature >= 700) {
      return 'Red';
    }
    if (EGTemperature >= 900) {
      return 'Red';
    }
    if (EGTemperature > (starterValve ? 700 : 850) && (throttleMode < 33 || throttleMode < 3)) {
      return 'Amber';
    }
    return 'Green';
  };

  private readonly amberVisible = MappedSubject.create(
    ([tm, sv]) => sv || tm < 33,
    this.throttlePosition,
    this.starterValveOpen,
  );

  private readonly egtColour = MappedSubject.create(
    ([egt, tm, sv]) => this.warningEGTColor(egt, tm, sv),
    this.egt,
    this.throttlePosition,
    this.starterValveOpen,
  );

  private readonly maxLimitEgt = MappedSubject.create(
    ([starterValve]) => (starterValve ? 700 : 850),
    this.starterValveOpen,
  );

  // Gauge needle indicator clamped to valid gauge scale range (0 - 1000 °C)
  private readonly trimmedEGT = this.egt.map((egt) => Math.min(this.max, Math.max(this.min, egt)));

  public onAfterRender(node: VNode): void {
    super.onAfterRender(node);
  }

  render() {
    return (
      <>
        <g id={`EGT-indicator-${this.props.engine}`}>
          <g visibility={this.props.active.map((it) => (!it ? 'inherit' : 'hidden'))}>
            <GaugeComponent
              x={this.props.x}
              y={this.props.y}
              radius={this.radius}
              startAngle={this.startAngle}
              endAngle={this.endAngle}
              visible={Subject.create(true)}
              class="GaugeComponent WhiteLine SW2"
            />
            <text class="F26 End Amber" x={this.props.x + 17} y={this.props.y + 11.7}>
              XX
            </text>
          </g>
          <g visibility={this.props.active.map((it) => (it ? 'inherit' : 'hidden'))}>
            <text class={this.egtColour.map((col) => `Large End ${col}`)} x={this.props.x + 33} y={this.props.y + 11.7}>
              {this.egt.map((egt) => Math.round(egt))}
            </text>
            <GaugeComponent
              x={this.props.x}
              y={this.props.y}
              radius={this.radius}
              startAngle={this.startAngle}
              endAngle={this.endAngle}
              visible={Subject.create(true)}
              class="GaugeComponent Gauge"
            >
              <GaugeComponent
                x={this.props.x}
                y={this.props.y}
                radius={this.radius - 2}
                startAngle={this.endAngle - 20}
                endAngle={this.endAngle}
                visible={Subject.create(true)}
                class="GaugeComponent Gauge ThickRedLine"
              />
              <GaugeMarkerComponent
                value={Subject.create(this.min)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge Medium"
              />
              <GaugeMarkerComponent
                value={Subject.create(500)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge"
              />
              <GaugeMarkerComponent
                value={Subject.create(this.max)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge RedLine"
              />
              <g visibility={this.amberVisible.map((it) => (it ? 'inherit' : 'hidden'))}>
                <GaugeMaxEGTComponent
                  value={this.maxLimitEgt}
                  x={this.props.x}
                  y={this.props.y}
                  min={this.min}
                  max={this.max}
                  radius={this.radius}
                  startAngle={this.startAngle}
                  endAngle={this.endAngle}
                  class="GaugeThrustLimitIndicatorFill Gauge"
                />
              </g>
              <rect x={this.props.x - 36} y={this.props.y - 11} width={72} height={26} class="DarkGreyBox" />
              <GaugeMarkerComponent
                value={this.trimmedEGT}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class={this.egtColour.map((col) => `${col}GaugeIndicator Gauge`)}
                multiplierInner={0.75}
                indicator
                halfIndicator
              />
            </GaugeComponent>
          </g>
        </g>
      </>
    );
  }
}
