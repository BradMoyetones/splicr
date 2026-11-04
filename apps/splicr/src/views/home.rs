//! Home: where files come in. A drop zone (or a click, for the file dialog), the files opened
//! lately, and what Splicr does with them.

use eframe::egui::{self, RichText};
use splicr_ui::{Palette, card, drop_zone, icon, icon_label, semibold};

use crate::app::App;
use crate::dialogs::Dialog;
use crate::i18n::tr;
use crate::views::section_title;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let p = Palette::of(ui);
    let dragging = ui.input(|i| !i.raw.hovered_files.is_empty());
    let zone = drop_zone(
        ui,
        dragging,
        icon::FILE_ADD,
        tr!("Drop files here, or click to choose them"),
        tr!("CSV, Excel (xlsx, xlsm, xlsb, xls), OpenDocument (ods) and zip archives of them"),
        200.0,
    );
    if zone.clicked() {
        app.dialog = Some(Dialog::Open);
    }
    ui.add_space(20.0);

    ui.columns(2, |columns| {
        let ui = &mut columns[0];
        section_title(ui, tr!("Recent files"));
        ui.add_space(4.0);
        if app.config.recent.is_empty() {
            ui.label(RichText::new(tr!("The files you open will be listed here.")).color(p.muted));
        }
        let mut open = None;
        for path in &app.config.recent {
            let name = crate::app::file_name(path);
            let folder = path.parent().map(|f| f.display().to_string()).unwrap_or_default();
            let response = card(ui, false, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    icon_label(ui, icon::SPREADSHEET, 18.0, p.accent);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new(&name).color(p.text));
                        ui.add(egui::Label::new(RichText::new(&folder).small().color(p.muted)).truncate());
                    });
                });
            })
            .response
            .interact(egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(path.display().to_string());
            if response.clicked() {
                open = Some(path.clone());
            }
            ui.add_space(2.0);
        }
        if let Some(path) = open {
            app.open(vec![path]);
        }

        let ui = &mut columns[1];
        section_title(ui, tr!("What you can do"));
        ui.add_space(4.0);
        let features = [
            (
                icon::TABLE,
                tr!("Look through your data"),
                tr!("Every sheet in a grid that stays fast with a million rows."),
            ),
            (
                icon::LAYERS,
                tr!("One workbook, one sheet each"),
                tr!("Many CSV and sheets gathered in a single Excel file."),
            ),
            (
                icon::MERGE,
                tr!("Stack them into one table"),
                tr!("Rows of every file one after another, columns matched by name."),
            ),
            (icon::SPLIT, tr!("Or one file each"), tr!("Each table as its own xlsx or CSV, named as you like.")),
        ];
        for (glyph, title, detail) in features {
            card(ui, false, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    icon_label(ui, glyph, 18.0, p.accent);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new(title).font(semibold(14.0)).color(p.text));
                        ui.add(egui::Label::new(RichText::new(detail).color(p.muted)).wrap());
                    });
                });
            });
            ui.add_space(2.0);
        }
    });
}
