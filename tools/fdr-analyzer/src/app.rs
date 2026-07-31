//! FDR Analyzer main application state and top-level layout.

use std::path::PathBuf;
use std::println;

use crate::export::export_csv;
use crate::fdr_reader::FdrStore;
use crate::parameter_tree::ParameterTree;
use crate::plot_panel::PlotPanel;
use crate::timeline::{TimeRange, format_time};
use crate::value_selector::ValueSelector;
use egui::*;
use log::warn;

pub struct FdrAnalyzerApp {
    fdr: FdrStore,
    panels: Vec<PlotPanel>,
    time_range: Option<TimeRange>,
    value_selector: ValueSelector,
    param_tree: Option<ParameterTree>,
    file_path: Option<PathBuf>,
    status: String,
    error: Option<String>,
}

impl Default for FdrAnalyzerApp {
    fn default() -> Self {
        Self {
            fdr: FdrStore::default(),
            panels: vec![PlotPanel::new("Flight Overview")],
            time_range: None,
            value_selector: ValueSelector::default(),
            param_tree: None,
            file_path: None,
            status: String::new(),
            error: None,
        }
    }
}

impl FdrAnalyzerApp {
    pub fn new(path: PathBuf) -> Self {
        let mut app = Self::default();
        app.file_path = Some(path);
        app
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
    }
}

impl eframe::App for FdrAnalyzerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(ref path) = self.file_path {
            if !self.fdr.is_open() {
                match self.fdr.open_file(path) {
                    Ok(()) => {
                        self.set_status(format!("Opened: {:?}", path.file_name()));
                        self.on_file_loaded();
                        self.file_path = None;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to open file: {}", e));
                        self.file_path = None;
                    }
                }
            } else {
                self.file_path = None;
            }
        }

        Self::menu_bar(&ui, &mut self.fdr);

        egui::Panel::bottom("timeline").show(ui, |ui| {
            if let Some(mut tr) = self.time_range.take() {
                println!("time_range exists: {}", tr.duration());
                let meta = self.fdr.metadata().unwrap_or_default();
                let ti = self.fdr.time_index().unwrap_or_default();
                let _changed = crate::timeline::timeline_bar(ui, &mut tr, &meta, &ti);
                self.time_range = Some(tr);
            }
        });

        egui::CentralPanel::default().show(ui, |ui| {
            self.main_content(ui);
        });
    }
}

impl FdrAnalyzerApp {
    fn menu_bar(_ctx: &egui::Context, _fdr: &mut FdrStore) {}

    fn main_content(&mut self, ui: &mut Ui) {
        if !self.fdr.is_open() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.heading("FlyByWire FDR Analyzer");
                ui.add_space(16.0);

                // Show error message if file opening failed.
                if let Some(ref err) = self.error {
                    ui.label(egui::RichText::new(err).color(egui::Color32::RED));
                    ui.add_space(12.0);
                }

                ui.label("Open an .fdr or .csv file to begin analyzing flight data.");
                ui.add_space(24.0);

                if ui
                    .add(egui::Button::new("Open File (test)").min_size(vec2(180.0, 36.0)))
                    .clicked()
                {
                    let _ = self.open_file_dialog();
                }
            });
            return;
        }

        // Show file info bar.
        if let Some(ref meta) = self.fdr.metadata() {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "File: {:?}",
                    meta.path
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_else(|| "<unknown>".into()),
                ));
                ui.separator();
                if let Some(ac) = meta.aircraft_type {
                    let name = match ac {
                        crate::fdr_reader::AircraftType::A320 => "A320",
                        crate::fdr_reader::AircraftType::A380 => "A380",
                    };
                    ui.label(name);
                } else {
                    ui.label("CSV data");
                }
                ui.separator();
                ui.label(format!(
                    "{} records  |  {} to {}",
                    meta.total_records,
                    format_time(meta.time_start_s),
                    format_time(meta.time_end_s)
                ));
                ui.separator();
                if ui.small_button("Close").clicked() {
                    self.fdr.close();
                    self.param_tree = None;
                    self.panels.clear();
                    self.panels.push(PlotPanel::new("Flight Overview"));
                    self.time_range = None;
                    self.value_selector = ValueSelector::default();
                    self.error = None;
                }
            });
        }

        ui.add_space(8.0);

        // Status line.
        if !self.status.is_empty() {
            ui.label(egui::RichText::new(&self.status).weak());
        }

        // Two-column layout: graphs on the left, parameter selector on the right.
        egui::Panel::right("param_selector")
            .resizable(true)
            .default_size(240.0)
            .show(ui, |ui| {
                ui.heading("Parameters");

                if let Some(ref tree) = self.param_tree {
                    if let Some(param) = self.value_selector.ui(ui, tree) {
                        if !self.panels.is_empty() {
                            self.panels[0].add_entry(param);
                            self.panels[0].invalidate_cache();
                        }
                    }
                }

                ui.separator();
                ui.add_space(8.0);

                // Export section.
                egui::CollapsingHeader::new("Export")
                    .default_open(false)
                    .show(ui, |export_ui| {
                        export_ui.label("Export the current view:");
                        export_ui.add_space(4.0);

                        if export_ui.button("Export CSV").clicked() {
                            let _ = self.export_visible_data();
                        }
                    });
            });

        // Graph area.
        egui::ScrollArea::vertical().show(ui, |graph_ui| {
            // Ensure time range is initialized.
            if self.time_range.is_none() {
                if let Some(ref meta) = self.fdr.metadata() {
                    self.time_range = Some(TimeRange::full(meta));
                }
            }

            for panel in &mut self.panels {
                if let Some(tr) = &self.time_range {
                    let keys: Vec<&str> = panel.visible_keys();
                    let max_pts = (tr.end - tr.start).max(1.0) as usize * 2;
                    let raw_data = self.fdr.query_range(&keys, tr.start, tr.end, max_pts);

                    if !panel.is_cached() || !raw_data.is_empty() {
                        panel.update_cache(&raw_data);
                    }

                    panel.ui(graph_ui, tr.start, tr.end);
                }
            }

            graph_ui.add_space(12.0);
            if graph_ui.button("Add Graph Panel").clicked() {
                let idx = self.panels.len();
                self.panels
                    .push(PlotPanel::new(&format!("Graph {}", idx + 1)));
            }
        });
    }

    fn on_file_loaded(&mut self) {
        let params = self.fdr.parameters();
        self.param_tree = Some(ParameterTree::build(&params));

        if let Some(meta) = self.fdr.metadata() {
            self.time_range = Some(TimeRange::full(&meta));
        }

        for panel in &mut self.panels {
            panel.invalidate_cache();
        }
    }

    fn open_file_dialog(&mut self) {
        let path = rfd::FileDialog::new()
            .add_filter("FDR / CSV", &["fdr", "csv"])
            .set_title("Open FDR or CSV File")
            .pick_file();

        match path {
            Some(p) => {
                self.file_path = Some(p);
            }
            None => {
                // User cancelled the dialog.
            }
        }
    }

    fn export_visible_data(&mut self) -> Result<(), String> {
        let meta = match self.fdr.metadata() {
            Some(m) => m,
            None => return Err("No file loaded".into()),
        };

        let tr = match &self.time_range {
            Some(tr) => tr,
            None => return Err("Time range not initialized".into()),
        };

        let mut keys: Vec<&str> = Vec::new();
        for panel in &self.panels {
            for key in panel.visible_keys() {
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }

        let raw_data = self.fdr.query_range(&keys, tr.start, tr.end, 5000);
        let export_path = meta.path.with_extension("export.csv");

        match export_csv(&export_path, &meta, &raw_data) {
            Ok(()) => {
                self.set_status(format!(
                    "Exported {} parameter series to {:?}",
                    raw_data.len(),
                    export_path.file_name()
                ));
                Ok(())
            }
            Err(e) => Err(format!("CSV export failed: {}", e)),
        }
    }
}
