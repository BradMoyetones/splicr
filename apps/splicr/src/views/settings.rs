//! Settings: the theme, the sidebar, updates, and where Splicr keeps its files.

use eframe::egui::{self, RichText};
use splicr_ui::{Palette, button, card, choice_card, icon, semibold, switch};

use crate::app::App;
use crate::config::{self, Theme};
use crate::i18n::tr;
use crate::updates;
use crate::views::section_title;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let p = *Palette::of(ui);
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.set_max_width(760.0);
        section_title(ui, tr!("Appearance"));
        ui.add_space(4.0);
        let themes = [
            (Theme::System, icon::SYSTEM, tr!("Like the system"), tr!("Light or dark, as the system is.")),
            (Theme::Light, icon::SUN, tr!("Light"), tr!("Always light.")),
            (Theme::Dark, icon::MOON, tr!("Dark"), tr!("Always dark.")),
        ];
        ui.columns(themes.len(), |columns| {
            for (ui, (theme, glyph, title, detail)) in columns.iter_mut().zip(themes) {
                if choice_card(ui, app.config.theme == theme, glyph, title, detail).clicked() {
                    app.config.theme = theme;
                    app.apply_theme(ui.ctx());
                    app.unsaved = true;
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let mut compact = app.config.compact_sidebar;
            if switch(ui, &mut compact, tr!("Compact sidebar")).changed() {
                app.config.compact_sidebar = compact;
                app.unsaved = true;
            }
            ui.label(RichText::new(tr!("Compact sidebar: icons only")).color(p.text));
        });
        ui.add_space(18.0);

        section_title(ui, tr!("Updates"));
        ui.add_space(4.0);
        card(ui, false, |ui| {
            ui.set_width(ui.available_width());
            let version = tr!("Splicr {version}", version = updates::VERSION);
            ui.label(RichText::new(version).font(semibold(14.0)).color(p.text));
            if app.updates.available() {
                ui.label(
                    RichText::new(tr!("Splicr looks for new versions by itself, every few hours.")).color(p.muted),
                );
                app.updates.check_button(ui);
            } else {
                ui.label(
                    RichText::new(tr!("This copy was built from source: it doesn't update itself.")).color(p.muted),
                );
            }
        });
        app.updates.banner(ui);
        ui.add_space(18.0);

        section_title(ui, tr!("About"));
        ui.add_space(4.0);
        card(ui, false, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(tr!("Open source, MIT licence. Written in Rust, from scratch, in the style of Remapr."))
                    .color(p.text),
            );
            ui.horizontal(|ui| {
                if button(ui, None, tr!("Source code")).clicked() {
                    splicr_platform::open("https://github.com/BradMoyetones/splicr");
                }
                if button(ui, Some(icon::FOLDER), tr!("Settings and logs folder")).clicked() {
                    splicr_platform::open(&config::dir().to_string_lossy());
                }
            });
        });
    });
}
