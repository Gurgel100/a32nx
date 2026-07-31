//! Shared timeline control — horizontal time ruler supporting click-drag zoom and scroll pan.

use crate::fdr_reader::{FileMetadata, TimeIndex};
use egui::*;

/// A selectable range on the shared X-axis (time).
#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

impl TimeRange {
    pub fn full(metadata: &FileMetadata) -> Self {
        Self {
            start: metadata.time_start_s,
            end: metadata.time_end_s,
        }
    }

    pub fn clamp(&mut self, min_time: f64, max_time: f64) {
        self.start = self.start.max(min_time).min(max_time);
        self.end = self.end.max(min_time).min(max_time);
        if self.start >= self.end {
            self.start = self.end - 0.1;
        }
    }

    pub fn zoom_to(&mut self, new_start: f64, new_end: f64, min_time: f64, max_time: f64) {
        let s = new_start.min(new_end);
        let e = new_start.max(new_end);
        self.start = s.clamp(min_time, max_time);
        self.end = e.clamp(min_time, max_time);
        if (self.end - self.start).abs() < 0.01 {
            let center = (self.start + self.end) / 2.0;
            self.start = (center - 0.005).max(min_time);
            self.end = (center + 0.005).min(max_time);
        }
    }

    pub fn zoom_out(&mut self, factor: f64, min_time: f64, max_time: f64) {
        let center = (self.start + self.end) / 2.0;
        let half_range = (self.end - self.start) / 2.0 * factor;
        self.start = (center - half_range).max(min_time);
        self.end = (center + half_range).min(max_time);
    }

    pub fn reset(&mut self, metadata: &FileMetadata) {
        self.start = metadata.time_start_s;
        self.end = metadata.time_end_s;
    }

    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

/// Format a time value as "mm:ss" or "hh:mm:ss".
pub fn format_time(s: f64) -> String {
    let s = s.max(0.0);
    let hours = s as u32 / 3600;
    let minutes = (s as u32 % 3600) / 60;
    let seconds = (s as u32) % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

/// Render a horizontal timeline bar. Returns true if the range was changed.
pub fn timeline_bar(
    ui: &mut Ui,
    time_range: &mut TimeRange,
    metadata: &FileMetadata,
    _time_index: &TimeIndex,
) -> bool {
    let min_time = metadata.time_start_s;
    let max_time = metadata.time_end_s;
    let mut changed = false;

    let total_duration = max_time - min_time;
    if total_duration <= 0.0 {
        return false;
    }

    let left_frac = (time_range.start - min_time) / total_duration;
    let right_frac = (time_range.end - min_time) / total_duration;

    // All UI positions use f32.
    let avail = ui.available_rect_before_wrap();
    let rect = Rect::from_min_max(
        pos2(avail.min.x, avail.center().y - 8.0),
        pos2(avail.max.x, avail.center().y + 8.0),
    );

    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let r = resp.rect;
    let left_x = r.min.x + (left_frac as f32 * r.width());
    let right_x = r.min.x + (right_frac as f32 * r.width());

    // Draw track background.
    ui.painter()
        .rect_filled(r, 2.0, ui.visuals().extreme_bg_color);
    ui.painter().rect_stroke(
        r,
        2.0,
        ui.style().noninteractive().fg_stroke,
        egui::StrokeKind::Outside,
    );

    if left_x < right_x {
        let inner_rect = Rect::from_min_max(pos2(left_x, r.min.y), pos2(right_x, r.max.y));
        ui.painter()
            .rect_filled(inner_rect, 2.0, ui.style().visuals.selection.bg_fill);

        // Draw edge handles.
        let handle_w = 4.0;
        for x in &[left_x, right_x] {
            let h_rect = Rect::from_min_max(
                pos2(*x - handle_w / 2.0, r.min.y),
                pos2(*x + handle_w / 2.0, r.max.y),
            );
            ui.painter()
                .rect_filled(h_rect, 1.0, ui.style().visuals.selection.stroke.color);
        }
    }

    // Drag interaction.
    if resp.dragged() {
        if let Some(cursor) = resp.hover_pos() {
            let frac = ((cursor.x - r.min.x) / r.width()).clamp(0.0, 1.0) as f64;
            let t = min_time + frac * total_duration;

            let left_dist = (t - time_range.start).abs();
            let right_dist = (t - time_range.end).abs();
            let center = (time_range.start + time_range.end) / 2.0;
            let mid_dist = (t - center).abs();

            if left_dist < right_dist && left_dist < mid_dist {
                time_range.start = t.clamp(min_time, time_range.end);
                changed = true;
            } else if right_dist < mid_dist {
                time_range.end = t.clamp(time_range.start, max_time);
                changed = true;
            } else {
                let dur = time_range.duration();
                let new_start = (t - dur / 2.0).max(min_time);
                let new_end = (t + dur / 2.0).min(max_time);
                if new_end - new_start < dur {
                    if t - dur / 2.0 < min_time {
                        time_range.start = min_time;
                        time_range.end = (min_time + dur).min(max_time);
                    } else if t + dur / 2.0 > max_time {
                        time_range.end = max_time;
                        time_range.start = (max_time - dur).max(min_time);
                    } else {
                        time_range.start = new_start;
                        time_range.end = new_end;
                    }
                } else {
                    time_range.start = new_start;
                    time_range.end = new_end;
                }
                changed = true;
            }
        }
    }

    // Mouse wheel zooms.
    let wheel_delta = ui.input(|r| r.smooth_scroll_delta.y);
    if resp.hovered() && wheel_delta.abs() > 0.0 {
        let zoom_factor = 1.15_f64.powf(-wheel_delta as f64 * 0.1);
        if let Some(cursor) = resp.hover_pos() {
            let frac = ((cursor.x - r.min.x) / r.width()).clamp(0.0, 1.0) as f64;
            let pivot = min_time + frac * total_duration;
            if pivot >= time_range.start && pivot <= time_range.end {
                let left_span = pivot - time_range.start;
                let right_span = time_range.end - pivot;
                time_range.start = (pivot - left_span * zoom_factor).max(min_time);
                time_range.end = (pivot + right_span * zoom_factor).min(max_time);
                changed = true;
            }
        }
    }

    // Double-click resets to full range.
    if resp.double_clicked() {
        time_range.reset(metadata);
        changed = true;
    }

    // Draw time labels.
    ui.painter().text(
        pos2(left_x, r.max.y + 4.0),
        Align2::LEFT_BOTTOM,
        format_time(time_range.start),
        FontId::monospace(11.0),
        ui.visuals().text_color(),
    );
    ui.painter().text(
        pos2(right_x - 50.0, r.max.y + 4.0),
        Align2::LEFT_BOTTOM,
        format_time(time_range.end),
        FontId::monospace(11.0),
        ui.visuals().text_color(),
    );

    // Keyboard shortcuts.
    let minus_pressed = ui.input(|r| r.key_pressed(Key::Minus));
    let equals_pressed = ui.input(|r| r.key_pressed(Key::Equals));
    if minus_pressed || equals_pressed {
        let factor = if minus_pressed { 1.5 } else { 0.67 };
        time_range.zoom_out(factor, min_time, max_time);
        changed = true;
    }

    ui.add_space(24.0);
    changed
}
