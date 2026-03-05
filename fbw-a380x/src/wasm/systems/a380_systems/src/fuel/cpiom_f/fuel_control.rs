use std::{cell::LazyCell, sync::LazyLock};

use crate::fuel::{
    cpiom_f::{TransferGalleryConnections, TransferGalleryTankConnections},
    A380FuelPump, A380FuelTankType, A380FuelValve,
};
use enum_map::{enum_map, Enum, EnumMap};
use fxhash::FxHashSet;

static TANK_PUMP_MAP: LazyLock<EnumMap<A380FuelTankType, (A380FuelPump, A380FuelPump)>> =
    LazyLock::new(|| {
        enum_map! {
            A380FuelTankType::FeedOne => (A380FuelPump::Feed1Main, A380FuelPump::Feed1Stby),
            A380FuelTankType::FeedTwo => (A380FuelPump::Feed2Main, A380FuelPump::Feed2Stby),
            A380FuelTankType::FeedThree => (A380FuelPump::Feed3Main, A380FuelPump::Feed3Stby),
            A380FuelTankType::FeedFour => (A380FuelPump::Feed4Main, A380FuelPump::Feed4Stby),
            A380FuelTankType::LeftInner => (A380FuelPump::LeftInnerFwd, A380FuelPump::LeftInnerAft),
            A380FuelTankType::LeftMid => (A380FuelPump::LeftMidFwd, A380FuelPump::LeftMidAft),
            A380FuelTankType::LeftOuter => (A380FuelPump::LeftOuter, A380FuelPump::LeftOuter), // only one pump for outer tank
            A380FuelTankType::RightInner => (A380FuelPump::RightInnerFwd, A380FuelPump::RightInnerAft),
            A380FuelTankType::RightMid => (A380FuelPump::RightMidFwd, A380FuelPump::RightMidAft),
            A380FuelTankType::RightOuter => (A380FuelPump::RightOuter, A380FuelPump::RightOuter), // only one pump for outer tank
            A380FuelTankType::Trim => (A380FuelPump::TrimLeft, A380FuelPump::TrimRight),
        }
    });

static PUMP_TANK_MAP: LazyLock<EnumMap<A380FuelPump, A380FuelTankType>> = LazyLock::new(|| {
    enum_map! {
        A380FuelPump::Feed1Main => A380FuelTankType::FeedOne,
        A380FuelPump::Feed1Stby => A380FuelTankType::FeedOne,
        A380FuelPump::Feed2Main => A380FuelTankType::FeedTwo,
        A380FuelPump::Feed2Stby => A380FuelTankType::FeedTwo,
        A380FuelPump::Feed3Main => A380FuelTankType::FeedThree,
        A380FuelPump::Feed3Stby => A380FuelTankType::FeedThree,
        A380FuelPump::Feed4Main => A380FuelTankType::FeedFour,
        A380FuelPump::Feed4Stby => A380FuelTankType::FeedFour,
        A380FuelPump::LeftInnerFwd => A380FuelTankType::LeftInner,
        A380FuelPump::LeftInnerAft => A380FuelTankType::LeftInner,
        A380FuelPump::LeftMidFwd => A380FuelTankType::LeftMid,
        A380FuelPump::LeftMidAft => A380FuelTankType::LeftMid,
        A380FuelPump::LeftOuter => A380FuelTankType::LeftOuter,
        A380FuelPump::RightInnerFwd => A380FuelTankType::RightInner,
        A380FuelPump::RightInnerAft => A380FuelTankType::RightInner,
        A380FuelPump::RightMidFwd => A380FuelTankType::RightMid,
        A380FuelPump::RightMidAft => A380FuelTankType::RightMid,
        A380FuelPump::RightOuter => A380FuelTankType::RightOuter,
        A380FuelPump::TrimLeft => A380FuelTankType::Trim,
        A380FuelPump::TrimRight => A380FuelTankType::Trim,
    }
});

static TANK_INLET_MAP: LazyLock<EnumMap<A380FuelTankType, (A380FuelValve, A380FuelValve)>> =
    LazyLock::new(|| {
        enum_map! {
            A380FuelTankType::FeedOne => (A380FuelValve::FeedTank1ForwardTransferValve, A380FuelValve::FeedTank1AftTransferValve),
            A380FuelTankType::FeedTwo => (A380FuelValve::FeedTank2ForwardTransferValve, A380FuelValve::FeedTank2AftTransferValve),
            A380FuelTankType::FeedThree => (A380FuelValve::FeedTank3ForwardTransferValve, A380FuelValve::FeedTank3AftTransferValve),
            A380FuelTankType::FeedFour => (A380FuelValve::FeedTank4ForwardTransferValve, A380FuelValve::FeedTank4AftTransferValve),
            A380FuelTankType::LeftInner => (A380FuelValve::LeftInnerForwardTransferValve, A380FuelValve::LeftInnerAftTransferValve),
            A380FuelTankType::LeftMid => (A380FuelValve::LeftMidForwardTransferValve, A380FuelValve::LeftMidAftTransferValve),
            A380FuelTankType::LeftOuter => (A380FuelValve::LeftOuterForwardTransferValve, A380FuelValve::LeftOuterAftTransferValve),
            A380FuelTankType::RightInner => (A380FuelValve::RightInnerForwardTransferValve, A380FuelValve::RightInnerAftTransferValve),
            A380FuelTankType::RightMid => (A380FuelValve::RightMidForwardTransferValve, A380FuelValve::RightMidAftTransferValve),
            A380FuelTankType::RightOuter => (A380FuelValve::RightOuterForwardTransferValve, A380FuelValve::RightOuterAftTransferValve),
            // The trim tank inlet valves are not used for fuel transfer, but for filling the tank during refueling
            A380FuelTankType::Trim => (A380FuelValve::TrimTankInletValve1, A380FuelValve::TrimTankInletValve2),
        }
    });

pub(super) struct FuelControlApplication {
    fuel_pump_requested_running: [bool; A380FuelPump::LENGTH],
    fuel_valve_requested_open: [bool; A380FuelValve::LENGTH],
}
impl FuelControlApplication {
    const FEED_TANK_FWD_INLET_VALVES: [A380FuelValve; 4] = [
        A380FuelValve::FeedTank1ForwardTransferValve,
        A380FuelValve::FeedTank2ForwardTransferValve,
        A380FuelValve::FeedTank3ForwardTransferValve,
        A380FuelValve::FeedTank4ForwardTransferValve,
    ];
    const FEED_TANK_AFT_INLET_VALVES: [A380FuelValve; 4] = [
        A380FuelValve::FeedTank1AftTransferValve,
        A380FuelValve::FeedTank2AftTransferValve,
        A380FuelValve::FeedTank3AftTransferValve,
        A380FuelValve::FeedTank4AftTransferValve,
    ];

    const PUMPS_IN_CONTROL: [A380FuelPump; 12] = [
        A380FuelPump::LeftInnerFwd,
        A380FuelPump::LeftInnerAft,
        A380FuelPump::LeftMidFwd,
        A380FuelPump::LeftMidAft,
        A380FuelPump::LeftOuter,
        A380FuelPump::RightInnerFwd,
        A380FuelPump::RightInnerAft,
        A380FuelPump::RightMidFwd,
        A380FuelPump::RightMidAft,
        A380FuelPump::RightOuter,
        A380FuelPump::TrimLeft,
        A380FuelPump::TrimRight,
    ];

    pub(super) fn new() -> Self {
        Self {
            fuel_pump_requested_running: [false; A380FuelPump::LENGTH],
            fuel_valve_requested_open: [false; A380FuelValve::LENGTH],
        }
    }

    pub(super) fn update(&mut self, gallery_connections: &TransferGalleryTankConnections) {
        self.fuel_pump_requested_running = Default::default();
        self.fuel_valve_requested_open = [false; A380FuelValve::LENGTH];

        // Determine feed tank inlet valve state
        for (tank, mode) in A380FuelTankType::iterator().zip(gallery_connections.forward_gallery) {}

        for (i, (source_tank, gallery)) in feed_tank_sources.iter().enumerate() {
            if source_tank.is_some() {
                // let fuel_valves = match gallery {
                //     TransferGallery::Fwd => Self::FEED_TANK_FWD_INLET_VALVES,
                //     TransferGallery::Aft => Self::FEED_TANK_AFT_INLET_VALVES,
                // };
                // self.fuel_valve_requested_open[fuel_valves[i].into_usize()] = true;
            }
        }

        // Determine which fuel pumps should run
        for source in &source_tanks {
            let (left_pump, right_pump) = self.source_tank_to_fuel_pump_map[source];
            self.fuel_pump_requested_running[left_pump.into_usize()] = true;
            self.fuel_pump_requested_running[right_pump.into_usize()] = true;
        }
    }
}
