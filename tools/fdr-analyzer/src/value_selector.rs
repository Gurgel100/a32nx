//! Value selector — tree browser for available parameters.

use crate::fdr_reader::ParameterInfo;
use crate::parameter_tree::ParameterTree;
use egui::*;

/// State for the parameter browser sidebar.
pub struct ValueSelector {
    /// Search filter text.
    search: String,
    /// Expanded state per group name.
    expanded_groups: std::collections::HashSet<String>,
}

impl Default for ValueSelector {
    fn default() -> Self {
        Self {
            search: String::new(),
            expanded_groups: std::collections::HashSet::new(),
        }
    }
}

impl ValueSelector {
    /// Render the parameter browser UI. Returns `Some(ParameterInfo)` when the user
    /// clicks a parameter row (signaling "add to graph").
    pub fn ui(&mut self, ui: &mut Ui, tree: &ParameterTree) -> Option<ParameterInfo> {
        let mut selected: Option<ParameterInfo> = None;

        // Search bar.
        ui.horizontal(|ui| {
            ui.label("Search:");
            let resp = ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(f32::MAX));
            if resp.lost_focus() && ui.input(|r| r.key_pressed(egui::Key::Escape)) {
                self.search.clear();
            }
        });
        ui.add_space(4.0);

        let filtered = tree.filtered(&self.search);

        // Scrollable list of groups.
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (group_name, params) in &filtered {
                let is_expanded = self.expanded_groups.contains(group_name);
                let count = params.len();
                let header_text = format!("{} ({})", group_name, count);

                // Use a manual toggle button + Grid to avoid CollapsingHeader API issues.
                ui.horizontal(|ui| {
                    let arrow = if is_expanded { "▼" } else { "▶" };
                    if ui.button(format!("{} {}", arrow, header_text)).clicked() {
                        if self.expanded_groups.contains(group_name) {
                            self.expanded_groups.remove(group_name);
                        } else {
                            self.expanded_groups.insert(group_name.clone());
                        }
                    }
                });

                if is_expanded {
                    egui::Grid::new(format!("grid_{}", group_name)).show(ui, |grid_ui| {
                        for param in params {
                            let btn = grid_ui.button(&param.display_name);
                            if btn.clicked() {
                                selected = Some((*param).clone());
                            }
                            btn.on_hover_text(format!(
                                "Display: {}\nKey: {}\nUnit: {}",
                                param.display_name, param.key, param.unit
                            ));
                            grid_ui.end_row();
                        }
                    });
                }
            }

            if filtered.is_empty() {
                ui.add_space(16.0);
                ui.centered_and_justified(|ui| {
                    let msg = if self.search.is_empty() {
                        "No parameters available".to_string()
                    } else {
                        format!("No parameters match \"{}\"", self.search)
                    };
                    ui.label(&msg);
                });
            }
        });

        selected
    }
}
