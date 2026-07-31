//! FDR Analyzer — Main entry point for the egui application.

use fdr_analyzer::app::FdrAnalyzerApp;
use std::path::PathBuf;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::Vec2::new(1400.0, 900.0))
            .with_min_inner_size(egui::Vec2::new(900.0, 600.0)),
        ..Default::default()
    };

    let app_name = "FDR Analyzer";

    if let Some(path_str) = std::env::args().nth(1) {
        // Allow opening a file directly from CLI.
        let path = PathBuf::from(path_str);
        eframe::run_native(
            app_name,
            options,
            Box::new(move |_cc| Ok(Box::new(FdrAnalyzerApp::new(path)))),
        )
    } else {
        eframe::run_native(
            app_name,
            options,
            Box::new(|_cc| Ok(Box::new(FdrAnalyzerApp::default()))),
        )
    }
}
