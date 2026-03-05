use crate::fuel::{
    cpiom_f::{
        fuel_transfer::FuelTransfer, FuelQuantityProvider, TankMode, TransferGalleryConnections,
        FEED_TANKS, INNER_TANKS, MID_TANKS, OUTER_TANKS,
    },
    A380FuelTankType,
};
use std::{ops::Not, time::Duration};
use uom::si::{
    f64::{Length, Mass},
    length::foot,
    mass::pound,
};

#[derive(Default)]
pub(super) struct LoadAlleviationTransfer {
    transfer_to_outer_tank_active: [bool; 2],
    transfer_from_outer_tank_active: [bool; 2],
    trim_tank_forward_transfer_active: bool,
    max_altitude_reached: Length,
}
impl LoadAlleviationTransfer {
    const TIME_TO_DESTINATION_TRIM_TANK_FWD_TRANSFER_STOP: Duration = Duration::from_secs(60 * 80);
    const TIME_TO_DESTINATION_TRIM_TANK_FWD_TRANSFER_START: Duration = Duration::from_secs(60 * 78);
    const TIME_TO_DESTINATION_TRANSFER_FROM_OUTER_STOP: Duration = Duration::from_secs(60 * 30);
    const TIME_TO_DESTINATION_TRANSFER_FROM_OUTER_START: Duration = Duration::from_secs(60 * 28);

    pub(super) fn update(
        &mut self,
        tank_quantities: &impl FuelQuantityProvider,
        total_fuel_on_board: Mass,
        remaining_flight_time: Option<Duration>,
        in_flight: bool,
        trim_tank_feed_isolated: bool,
        altitude: Option<Length>,
    ) {
        // TODO: verify logic
        self.max_altitude_reached = if in_flight {
            self.max_altitude_reached.max(altitude.unwrap_or_default())
        } else {
            Length::default()
        };

        self.update_transfer_to_outer_tanks(tank_quantities, total_fuel_on_board, in_flight);

        // Load alleviation transfers from outer tanks
        self.update_transfer_from_outer_tanks(tank_quantities, remaining_flight_time, altitude);

        self.update_transfer_from_trim_tank(
            tank_quantities,
            remaining_flight_time,
            altitude,
            trim_tank_feed_isolated,
        );
    }

    pub(super) fn is_transfer_to_outer_tank_active(&self) -> bool {
        self.transfer_to_outer_tank_active
            .iter()
            .any(|&active| active)
    }

    pub(super) fn is_transfer_from_outer_tank_active(&self) -> bool {
        self.transfer_from_outer_tank_active
            .iter()
            .any(|&active| active)
    }

    pub(super) fn is_trim_tank_forward_transfer_active(&self) -> bool {
        self.trim_tank_forward_transfer_active
    }

    fn update_transfer_to_outer_tanks(
        &mut self,
        tank_quantities: &impl FuelQuantityProvider,
        total_fuel_on_board: Mass,
        in_flight: bool,
    ) {
        let transfer_enabled = total_fuel_on_board.get::<pound>() > 110_200. && in_flight;
        self.transfer_to_outer_tank_active =
            OUTER_TANKS.map(|tank| transfer_enabled && !tank_quantities.tank_full(tank));
    }

    fn update_transfer_from_outer_tanks(
        &mut self,
        tank_quantities: &impl FuelQuantityProvider,
        remaining_flight_time: Option<Duration>,
        altitude: Option<Length>,
    ) {
        self.transfer_from_outer_tank_active = [
            (
                A380FuelTankType::LeftOuter,
                self.transfer_from_outer_tank_active[0],
            ),
            (
                A380FuelTankType::RightOuter,
                self.transfer_from_outer_tank_active[1],
            ),
        ]
        .map(|(tank, active)| {
            let q = tank_quantities.get_tank_quantity(tank);
            q.get::<pound>() > 8_800.
                && if active {
                    remaining_flight_time
                        .is_some_and(|d| d <= Self::TIME_TO_DESTINATION_TRANSFER_FROM_OUTER_STOP)
                } else {
                    remaining_flight_time
                        .is_some_and(|d| d < Self::TIME_TO_DESTINATION_TRANSFER_FROM_OUTER_START)
                        || self.max_altitude_reached > Length::new::<foot>(25500.)
                            && altitude.is_some_and(|a| a < Length::new::<foot>(24500.))
                    // TODO: verify what happens when altitude and/or remaining flight time is missing
                }
        });
    }

    fn update_transfer_from_trim_tank(
        &mut self,
        tank_quantities: &impl FuelQuantityProvider,
        remaining_flight_time: Option<Duration>,
        altitude: Option<Length>,
        trim_tank_feed_isolated: bool,
    ) {
        let trim_tank_empty = tank_quantities.tank_empty(A380FuelTankType::Trim);
        self.trim_tank_forward_transfer_active = if self.trim_tank_forward_transfer_active {
            !trim_tank_empty
                && remaining_flight_time
                    .is_none_or(|d| d <= Self::TIME_TO_DESTINATION_TRIM_TANK_FWD_TRANSFER_STOP)
                && !trim_tank_feed_isolated
        } else {
            !trim_tank_empty
                && !trim_tank_feed_isolated
                && (remaining_flight_time.is_some_and(|d| d < Self::TIME_TO_DESTINATION_TRIM_TANK_FWD_TRANSFER_START) // The FCOM is not really clear in this regard
                    || self.max_altitude_reached > Length::new::<foot>(25500.)
                        && altitude.is_some_and(|a| a < Length::new::<foot>(24500.)))
            // TODO: verify what happens when altitude and/or remaining flight time is missing
        };
    }
}
impl FuelTransfer for LoadAlleviationTransfer {
    fn set_gallery_modes(
        &self,
        gallery_connections: &mut impl TransferGalleryConnections,
        tank_quantities: &impl FuelQuantityProvider,
    ) {
        if gallery_connections.is_forward_gallery_usable() {
            if self.is_transfer_from_outer_tank_active() {
                // TODO: balance tanks and only select non-full tanks
                // TODO: what if inner tanks are full?
                let target_tanks: &[A380FuelTankType] = if !tank_quantities.tanks_full(FEED_TANKS) {
                    &FEED_TANKS
                } else if !tank_quantities.tanks_full(MID_TANKS) {
                    &MID_TANKS
                } else {
                    &INNER_TANKS
                };
                gallery_connections.set_forward_gallery_modes(
                    OUTER_TANKS
                        .into_iter()
                        .zip(self.transfer_from_outer_tank_active)
                        .filter_map(|(tank, active)| {
                            if active {
                                Some((tank, TankMode::Source))
                            } else {
                                None
                            }
                        })
                        .chain(target_tanks.into_iter().filter_map(|&tank| {
                            tank_quantities
                                .tank_full(tank)
                                .not()
                                .then_some((tank, TankMode::Target))
                        })),
                );
            } else if self.is_transfer_to_outer_tank_active() {
                // TODO: what if mid tanks are empty?
                let tank_source = if !tank_quantities.tanks_empty(INNER_TANKS) {
                    INNER_TANKS
                } else {
                    MID_TANKS
                };

                gallery_connections.set_forward_gallery_modes(
                    OUTER_TANKS
                        .into_iter()
                        .zip(self.transfer_to_outer_tank_active)
                        .filter_map(|(tank, active)| {
                            if active {
                                Some((tank, TankMode::Target))
                            } else {
                                None
                            }
                        })
                        .chain(tank_source.into_iter().map(|tank| (tank, TankMode::Source))),
                );
            }
        }

        if self.trim_tank_forward_transfer_active && gallery_connections.is_aft_gallery_usable() {
            // Fuel transfers from the trim tank, via the aft gallery, to the:
            // ‐ Feed tanks, or
            // ‐ Mid tanks, if the feed tanks are full, or
            // ‐ Inner tanks, if the feed tanks and mid tanks are full.
            // TODO: what if inner tanks are full?

            // Determine where to transfer fuel from the trim tank
            // TODO: balance tanks
            let target_tanks: &[A380FuelTankType] = if !tank_quantities.tanks_full(FEED_TANKS) {
                // Transfer to feed tanks which are not full
                &FEED_TANKS
            } else if !tank_quantities.tanks_full(INNER_TANKS) {
                // Transfer to mid tanks
                &MID_TANKS
            } else {
                // Transfer to inner tanks
                &INNER_TANKS
            };

            gallery_connections.set_aft_gallery_modes(
                [(A380FuelTankType::Trim, TankMode::Source)]
                    .into_iter()
                    .chain(target_tanks.into_iter().filter_map(|&tank| {
                        tank_quantities
                            .tank_full(tank)
                            .not()
                            .then_some((tank, TankMode::Target))
                    })),
            );
        }
    }
}
