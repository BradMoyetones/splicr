//! One module per screen, each drawn by its `show` inside the app's frame (`app.rs`). Screens read
//! and change the app's state; they never touch files themselves: that goes through the engine.

pub mod combine;
pub mod data;
pub mod home;
pub mod settings;

use eframe::egui::{self, RichText};
use splicr_ui::{Icon, Palette, icon_label, semibold};

/// A screen's empty state: an icon, a title, a line of help, and room for a button below.
pub fn empty_state(ui: &mut egui::Ui, icon: Icon, title: &str, detail: &str, add: impl FnOnce(&mut egui::Ui)) {
    let p = Palette::of(ui);
    ui.add_space(64.0);
    ui.vertical_centered(|ui| {
        icon_label(ui, icon, 44.0, p.muted);
        ui.add_space(8.0);
        ui.label(RichText::new(title).font(semibold(17.0)).color(p.text));
        ui.label(RichText::new(detail).color(p.muted));
        ui.add_space(12.0);
        add(ui);
    });
}

/// A section's title inside a screen.
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    let p = Palette::of(ui);
    ui.label(RichText::new(text).font(semibold(15.0)).color(p.text));
}

/// Keeps a screen's content to `max` points wide, or to all there is when the window is narrower,
/// so lines stay readable on a wide window and nothing is cut on a narrow one.
pub fn readable_width(ui: &mut egui::Ui, max: f32) {
    let width = ui.available_width().min(max);
    ui.set_max_width(width);
}

/// Whether `count` items of at least `min_width` points fit side by side; when they don't, screens
/// put them one under another.
pub fn fit_side_by_side(ui: &egui::Ui, count: usize, min_width: f32) -> bool {
    let gaps = count.saturating_sub(1) as f32 * ui.spacing().item_spacing.x;
    ui.available_width() >= count as f32 * min_width + gaps
}
