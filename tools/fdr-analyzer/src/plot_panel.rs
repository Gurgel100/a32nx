//! Graph panel — individual chart rendered with egui_plot.

use std::collections::BTreeMap;

use crate::downsample::lttb;
use crate::fdr_reader::ParameterInfo;
use crate::timeline::format_time;
use egui::*;
use egui_plot::*;

/// A color assignment for one parameter in a graph panel.
#[derive(Debug, Clone)]
pub struct PlotEntry {
    pub param: ParameterInfo,
    pub color: Color32,
    pub visible: bool,
}

/// State for a single graph chart panel.
#[derive(Debug)]
pub struct PlotPanel {
    title: String,
    entries: Vec<PlotEntry>,
    auto_y: bool,
    y_range: std::ops::RangeInclusive<f64>,
    cached_series: BTreeMap<String, Vec<PlotPoint>>,
    cache_valid: bool,
}

impl Default for PlotPanel {
    fn default() -> Self {
        Self {
            title: "New Graph".to_string(),
            entries: Vec::new(),
            auto_y: true,
            y_range: -100.0..=100.0,
            cached_series: BTreeMap::new(),
            cache_valid: false,
        }
    }
}

impl PlotPanel {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            ..Default::default()
        }
    }

    pub fn add_entry(&mut self, param: ParameterInfo) {
        let color = Self::palette_color(self.entries.len());
        self.entries.push(PlotEntry {
            param: param.clone(),
            color,
            visible: true,
        });
        self.cache_valid = false;
    }

    #[allow(dead_code)]
    pub fn remove_entry(&mut self, idx: usize) {
        if idx < self.entries.len() {
            let key = self.entries[idx].param.key.clone();
            self.cached_series.remove(&key);
            self.entries.remove(idx);
            self.cache_valid = false;
        }
    }

    pub fn update_cache(&mut self, raw: &BTreeMap<String, crate::fdr_reader::ParameterSeries>) {
        let max_screen_points = 2000;

        self.cached_series.clear();
        println!("updating cache: {}", raw.len());
        for entry in &self.entries {
            if let Some(series) = raw.get(&entry.param.key) {
                let downsampled = lttb(series, max_screen_points);
                let plot_pts: Vec<PlotPoint> = downsampled
                    .into_iter()
                    .map(|pt| PlotPoint::new(pt.time_s as f64, pt.value))
                    .collect();
                self.cached_series.insert(entry.param.key.clone(), plot_pts);
            }
        }
        self.cache_valid = true;
    }

    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }

    pub fn is_cached(&self) -> bool {
        self.cache_valid
    }

    #[allow(dead_code)]
    pub fn entries_mut(&mut self) -> &mut Vec<PlotEntry> {
        &mut self.entries
    }

    /// Collect the keys of all visible entries.
    pub fn visible_keys(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| e.visible)
            .map(|e| e.param.key.as_str())
            .collect()
    }

    /// Render the graph panel using egui_plot.
    pub fn ui(&mut self, ui: &mut Ui, time_start: f64, time_end: f64) {
        if self.entries.is_empty() {
            ui.label("No parameters added to this graph. Use the Parameters panel on the right.");
            return;
        }

        let width = (ui.available_size().x) as usize;
        let height = 300.0;

        // Build plot series from cache.
        let mut plots: Vec<Line> = Vec::new();
        for entry in &self.entries {
            if !entry.visible {
                continue;
            }
            if let Some(points) = self.cached_series.get(&entry.param.key) {
                let filtered: Vec<_> = points
                    .iter()
                    .filter(|p| p.x >= time_start && p.x <= time_end)
                    .cloned()
                    .collect();

                if !filtered.is_empty() {
                    let owned_pts: Vec<[f64; 2]> = filtered.iter().map(|p| [p.x, p.y]).collect();
                    plots.push(
                        Line::new(&entry.param.display_name, owned_pts)
                            .color(entry.color)
                            .width(1.5),
                    );
                }
            }
        }

        ui.label(RichText::new(&self.title).strong());

        let plot = Plot::new(ui.id())
            .x_axis_formatter(|v, _r| {
                if v.value.abs() > 3600.0 {
                    format_time(v.value)
                } else {
                    format!("{:.1}s", v.value)
                }
            })
            .y_axis_formatter(|v, _r| format!("{:.2}", v.value))
            .allow_zoom(true)
            .allow_drag(true)
            .data_aspect(0.3)
            .width(width as f32)
            .height(height);

        let mut plots = plots;
        let plot_response = plot.show(ui, |plot_ui: &mut PlotUi<'_>| {
            while let Some(line) = plots.pop() {
                plot_ui.line(line);
            }
        });

        // Show values on hover.
        if plot_response.response.hovered() {
            if let Some(cursor) = plot_response.response.hover_pos() {
                let hover_plot = plot_response.transform.value_from_position(cursor);
                ui.label(format!(
                    "Time: {}, Value: {:.3}",
                    format_time(hover_plot.x),
                    hover_plot.y
                ));
            }
        }

        // Panel settings collapsible.
        ui.collapsing("Panel Settings", |ui| {
            ui.checkbox(&mut self.auto_y, "Auto-scale Y axis");

            if !self.auto_y {
                let mut ys = *self.y_range.start();
                let mut ye = *self.y_range.end();
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut ys).speed(1.0));
                    ui.label("to");
                    ui.add(DragValue::new(&mut ye).speed(1.0));
                });
                self.y_range = ys..=ye;
            }

            // Per-entry visibility/color.
            for entry in &mut self.entries {
                ui.horizontal(|ui| {
                    let name = &entry.param.display_name;
                    ui.checkbox(&mut entry.visible, name);
                    ui.color_edit_button_srgba(&mut entry.color);
                    if ui.small_button("Remove").clicked() {
                        // We can't remove while iterating, so just toggle visibility off.
                        entry.visible = false;
                    }
                });
            }
        });
    }

    fn palette_color(n: usize) -> Color32 {
        const PALETTE: [Color32; 12] = [
            Color32::from_rgb(255, 87, 87),  // red
            Color32::from_rgb(59, 180, 75),  // green
            Color32::from_rgb(50, 120, 255), // blue
            Color32::from_rgb(255, 193, 7),  // yellow
            Color32::from_rgb(155, 89, 182), // purple
            Color32::from_rgb(26, 188, 156), // teal
            Color32::from_rgb(230, 126, 34), // orange
            Color32::from_rgb(233, 30, 99),  // pink
            Color32::from_rgb(0, 188, 212),  // cyan
            Color32::from_rgb(103, 58, 183), // deep purple
            Color32::from_rgb(255, 152, 0),  // amber
            Color32::from_rgb(72, 61, 139),  // dark slate blue
        ];
        PALETTE[n % PALETTE.len()]
    }
}
