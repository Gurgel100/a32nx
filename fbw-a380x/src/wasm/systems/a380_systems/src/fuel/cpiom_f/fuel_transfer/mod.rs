mod cg_transfer;
mod load_alleviation_transfer;
mod main_transfer;

use super::FuelQuantityProvider;
use crate::fuel::cpiom_f::{TransferGalleryConnections, TransferGalleryTankConnections};
use std::time::Duration;
use uom::si::f64::{Length, Mass, Ratio};

trait FuelTransfer {
    fn set_gallery_modes(
        &self,
        gallery_connections: &mut impl TransferGalleryConnections,
        tank_quantities: &impl FuelQuantityProvider,
    );
}

/// The CPIOM-F partition which calculates how the fuel should get transfered
#[derive(Default)]
pub(super) struct FuelTransferApplication {
    gallery_connections: TransferGalleryTankConnections,

    main_transfer: main_transfer::MainTransfer,
    load_alleviation_transfer: load_alleviation_transfer::LoadAlleviationTransfer,
    cg_transfer: cg_transfer::CGTransfer,
}
impl FuelTransferApplication {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(super) fn update(
        &mut self,
        tank_quantities: &impl FuelQuantityProvider,
        total_fuel_on_board: Option<Mass>,
        gross_weight: Option<Mass>,
        gross_cg: Option<Ratio>,
        remaining_flight_time: Option<Duration>,
        in_flight: bool,
        trim_tank_feed_isolated: bool,
        altitude: Option<Length>,
    ) {
        let Some(total_fuel_on_board) = total_fuel_on_board else {
            // Only manual transfers are available
            // TODO
            self.gallery_connections = TransferGalleryTankConnections::default();
            return;
        };

        self.load_alleviation_transfer.update(
            tank_quantities,
            total_fuel_on_board,
            remaining_flight_time,
            in_flight,
            trim_tank_feed_isolated,
            altitude,
        );

        self.cg_transfer.update(gross_weight, gross_cg);

        self.main_transfer
            .update(tank_quantities, remaining_flight_time);

        let mut gallery_connections = TransferGalleryTankConnections::default();

        self.load_alleviation_transfer
            .set_gallery_modes(&mut gallery_connections, tank_quantities);
        self.cg_transfer
            .set_gallery_modes(&mut gallery_connections, tank_quantities);
        self.main_transfer
            .set_gallery_modes(&mut gallery_connections, tank_quantities);

        self.gallery_connections = gallery_connections;
    }

    pub(super) fn gallery_connections(&self) -> &TransferGalleryTankConnections {
        &self.gallery_connections
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::fuel::cpiom_f::TransferSourceTank;
//     use uom::si::{f64::Mass, mass::pound};

//     #[test]
//     fn fuel_in_all_tanks() {
//         let tank_quantity = Mass::new::<pound>(10_000.);
//         let remaining_flight_time = Duration::from_secs(120 * 60);
//         let mut app = FuelTransferApplication::new();

//         let feed_tank_quantities = [
//             (
//                 "Feed tanks above upper threshold",
//                 Mass::new::<pound>(45_323.),
//                 Mass::new::<pound>(48_140.),
//                 Mass::new::<pound>(48_140.),
//                 Mass::new::<pound>(45_323.),
//                 [TransferSourceTank::None; 4],
//             ),
//             (
//                 "Feed tanks below upper threshold but above lower threshold",
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(44_000.),
//                 [TransferSourceTank::None; 4],
//             ),
//             (
//                 "Feed 1 below lower threshold",
//                 Mass::new::<pound>(43_090.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(44_000.),
//                 [
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                 ],
//             ),
//             (
//                 "Feed 1 above lower threshold",
//                 Mass::new::<pound>(43_500.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(44_000.),
//                 [
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                 ],
//             ),
//             (
//                 "Feed 1 and 4 are balanced and below upper threshold",
//                 Mass::new::<pound>(43_700.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(43_700.),
//                 [
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                 ],
//             ),
//             (
//                 "Feed 1 is above, 4 below upper threshold, and feed 3 is below lower threshold",
//                 Mass::new::<pound>(45_310.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(45_940.),
//                 Mass::new::<pound>(45_210.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                 ],
//             ),
//             (
//                 "Feed 1 and 4 are above upper threshold, and feed 2 and 3 are and above lower threshold",
//                 Mass::new::<pound>(45_310.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(45_310.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                 ],
//             ),
//         ];

//         for (
//             description,
//             feed_1_tank_quantity,
//             feed_2_tank_quantity,
//             feed_3_tank_quantity,
//             feed_4_tank_quantity,
//             expected,
//         ) in feed_tank_quantities
//         {
//             let tank_quantities = TankQuantities {
//                 feed_tank_quantities: [
//                     feed_1_tank_quantity,
//                     feed_2_tank_quantity,
//                     feed_3_tank_quantity,
//                     feed_4_tank_quantity,
//                 ],
//                 inner_left_tank_quantity: tank_quantity,
//                 inner_right_tank_quantity: tank_quantity,
//                 mid_left_tank_quantity: tank_quantity,
//                 mid_right_tank_quantity: tank_quantity,
//                 outer_left_tank_quantity: tank_quantity,
//                 outer_right_tank_quantity: tank_quantity,
//                 trim_tank_quantity: tank_quantity,
//             };
//             app.update(&tank_quantities, remaining_flight_time);
//             assert_eq!(
//                 app.get_tank_sources(),
//                 expected,
//                 "Scenario failed: {}",
//                 description
//             );
//         }
//     }

//     #[test]
//     fn fuel_in_all_tanks_less_than_90_min() {
//         let tank_quantity = Mass::new::<pound>(10_000.);
//         let remaining_flight_time = Duration::from_secs(60 * 60);
//         let mut app = FuelTransferApplication::new();

//         let feed_tank_quantities = [
//             (
//                 "Feed tanks above upper threshold",
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(43_000.),
//                 Mass::new::<pound>(43_000.),
//                 Mass::new::<pound>(40_000.),
//                 [TransferSourceTank::None; 4],
//             ),
//             (
//                 "Feed tanks below upper threshold but above lower threshold",
//                 Mass::new::<pound>(37_000.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(37_000.),
//                 [TransferSourceTank::None; 4],
//             ),
//             (
//                 "Feed 4 below lower threshold",
//                 Mass::new::<pound>(37_000.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(36_490.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                 ],
//             ),
//             (
//                 "Feed 4 above lower threshold",
//                 Mass::new::<pound>(37_000.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(36_700.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                 ],
//             ),
//             (
//                 "Feed 1 and 4 are balanced and below upper threshold",
//                 Mass::new::<pound>(37_000.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(37_000.),
//                 [
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                 ],
//             ),
//             (
//                 "Feed 4 is above, 1 below upper threshold, and feed 2 is below lower threshold",
//                 Mass::new::<pound>(38_690.),
//                 Mass::new::<pound>(39_350.),
//                 Mass::new::<pound>(40_000.),
//                 Mass::new::<pound>(38_710.),
//                 [
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                 ],
//             ),
//             (
//                 "Feed 1 and 4 are above upper threshold, and feed 2 and 3 are and above lower threshold",
//                 Mass::new::<pound>(38_710.),
//                 Mass::new::<pound>(41_000.),
//                 Mass::new::<pound>(41_000.),
//                 Mass::new::<pound>(38_710.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                 ],
//             ),
//         ];

//         for (
//             description,
//             feed_1_tank_quantity,
//             feed_2_tank_quantity,
//             feed_3_tank_quantity,
//             feed_4_tank_quantity,
//             expected,
//         ) in feed_tank_quantities
//         {
//             let tank_quantities = TankQuantities {
//                 feed_tank_quantities: [
//                     feed_1_tank_quantity,
//                     feed_2_tank_quantity,
//                     feed_3_tank_quantity,
//                     feed_4_tank_quantity,
//                 ],
//                 inner_left_tank_quantity: tank_quantity,
//                 inner_right_tank_quantity: tank_quantity,
//                 mid_left_tank_quantity: tank_quantity,
//                 mid_right_tank_quantity: tank_quantity,
//                 outer_left_tank_quantity: tank_quantity,
//                 outer_right_tank_quantity: tank_quantity,
//                 trim_tank_quantity: tank_quantity,
//             };
//             app.update(&tank_quantities, remaining_flight_time);
//             assert_eq!(
//                 app.get_tank_sources(),
//                 expected,
//                 "Scenario failed: {}",
//                 description
//             );
//         }
//     }

//     #[test]
//     fn fuel_in_all_tanks_less_than_17_650_lbs() {
//         let tank_quantity = Mass::new::<pound>(7_000.);
//         let remaining_flight_time = Duration::from_secs(120 * 60);
//         let mut app = FuelTransferApplication::new();

//         let feed_tank_quantities = [
//             (
//                 "Feed tanks above upper threshold",
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(46_000.),
//                 Mass::new::<pound>(46_000.),
//                 [TransferSourceTank::None; 4],
//             ),
//             (
//                 "Feed tanks below upper threshold but above lower threshold",
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 [TransferSourceTank::None; 4],
//             ),
//             (
//                 "Feed 2 below lower threshold",
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(43_090.),
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                 ],
//             ),
//             (
//                 "Feed 2 above lower threshold",
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(43_500.),
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                 ],
//             ),
//             (
//                 "Feed 2 and 3 are balanced and below upper threshold",
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(43_600.),
//                 Mass::new::<pound>(43_600.),
//                 Mass::new::<pound>(44_000.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                 ],
//             ),
//             (
//                 "Feed 1, 2, and 3 are balanced and below upper threshold",
//                 Mass::new::<pound>(43_800.),
//                 Mass::new::<pound>(43_800.),
//                 Mass::new::<pound>(43_800.),
//                 Mass::new::<pound>(44_000.),
//                 [
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::None,
//                 ],
//             ),
//             (
//                 "Feed tanks are balanced and below upper threshold",
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 [
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                 ],
//             ),
//             (
//                 "Feed 1 above upper threshold other feed tanks below upper threshold",
//                 Mass::new::<pound>(45_310.),
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 Mass::new::<pound>(44_000.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                     TransferSourceTank::Inner,
//                 ],
//             ),
//             (
//                 "Feed tanks above upper threshold",
//                 Mass::new::<pound>(45_310.),
//                 Mass::new::<pound>(45_310.),
//                 Mass::new::<pound>(45_310.),
//                 Mass::new::<pound>(45_310.),
//                 [
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                     TransferSourceTank::None,
//                 ],
//             ),
//         ];

//         for (
//             description,
//             feed_1_tank_quantity,
//             feed_2_tank_quantity,
//             feed_3_tank_quantity,
//             feed_4_tank_quantity,
//             expected,
//         ) in feed_tank_quantities
//         {
//             let tank_quantities = TankQuantities {
//                 feed_tank_quantities: [
//                     feed_1_tank_quantity,
//                     feed_2_tank_quantity,
//                     feed_3_tank_quantity,
//                     feed_4_tank_quantity,
//                 ],
//                 inner_left_tank_quantity: tank_quantity,
//                 inner_right_tank_quantity: tank_quantity,
//                 mid_left_tank_quantity: tank_quantity,
//                 mid_right_tank_quantity: tank_quantity,
//                 outer_left_tank_quantity: tank_quantity,
//                 outer_right_tank_quantity: tank_quantity,
//                 trim_tank_quantity: tank_quantity,
//             };
//             app.update(&tank_quantities, remaining_flight_time);
//             assert_eq!(
//                 app.get_tank_sources(),
//                 expected,
//                 "Scenario failed: {}",
//                 description
//             );
//         }
//     }
// }
