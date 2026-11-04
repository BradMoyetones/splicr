//! Screenshots of the design system, light and dark: every widget, tone and icon, as screens use
//! them. They catch a change of look in review.
//!
//! `cargo test -p splicr-ui --features snapshots` compares with `tests/snapshots/`;
//! `UPDATE_SNAPSHOTS=1` in front writes new ones.

#![cfg(feature = "snapshots")]

use std::borrow::Cow;

use eframe::egui::{self, Layout, RichText, Theme};
use egui_kittest::Harness;
use splicr_ui::{
    GridData, Palette, Tone, badge, banner, button, card, choice_card, danger_icon_button, data_grid, drop_zone, icon,
    icon_button, icon_label, nav_item, primary_button, semibold, switch,
};

struct Sample;

impl GridData for Sample {
    fn rows(&self) -> usize {
        1_000_000
    }

    fn cols(&self) -> usize {
        6
    }

    fn column_title(&self, col: usize) -> Cow<'_, str> {
        Cow::Borrowed(["id", "nombre", "ciudad", "total", "fecha", "notas"][col])
    }

    fn cell(&self, row: usize, col: usize) -> Cow<'_, str> {
        Cow::Owned(match col {
            0 => format!("{:05}", row + 1),
            1 => ["Ana", "Luis", "Señora Núñez", "Eva"][row % 4].to_owned(),
            2 => ["Bogotá", "Lima", "Medellín"][row % 3].to_owned(),
            3 => format!("{}", (row * 37) % 1000),
            4 => format!("2026-11-{:02}", row % 28 + 1),
            _ => "Una nota larga que no cabe en la celda y se corta con puntos suspensivos".to_owned(),
        })
    }

    fn numeric(&self, _row: usize, col: usize) -> bool {
        matches!(col, 3 | 4)
    }
}

fn gallery(ui: &mut egui::Ui) {
    let p = *Palette::of(ui);
    egui::Frame::new().fill(p.bg).inner_margin(20).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(200.0);
                nav_item(ui, icon::HOME, "Inicio", false, false);
                nav_item(ui, icon::TABLE, "Datos", true, false);
                nav_item(ui, icon::COMBINE, "Combinar", false, false);
                nav_item(ui, icon::SETTINGS, "Ajustes", false, true);
            });
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Combinar").font(semibold(20.0)).color(p.text));
                    badge(ui, "XLSX");
                    badge(ui, "12 345 × 8");
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        primary_button(ui, Some(icon::DOWNLOAD), "Exportar");
                        button(ui, Some(icon::FILE_ADD), "Abrir archivos");
                        icon_button(ui, icon::MORE, "Más");
                        danger_icon_button(ui, icon::DELETE, "Quitar");
                    });
                });
                ui.columns(3, |columns| {
                    choice_card(&mut columns[0], true, icon::LAYERS, "Un libro", "Una hoja por cada tabla.");
                    choice_card(&mut columns[1], false, icon::SPLIT, "Un archivo por tabla", "xlsx o CSV.");
                    choice_card(&mut columns[2], false, icon::MERGE, "Una sola tabla", "Columnas por encabezado.");
                });
                drop_zone(ui, false, icon::FILE_ADD, "Suelta archivos aquí", "CSV, xlsx, xls, ods y zip", 140.0);
            });
        });
        ui.add_space(8.0);
        for (tone, text) in [
            (Tone::Info, "Elige la carpeta donde irán los archivos."),
            (Tone::Success, "3 archivos escritos."),
            (Tone::Warning, "Solo se leyeron las primeras 1 048 576 filas."),
            (Tone::Danger, "No se pudo abrir datos.csv."),
        ] {
            banner(ui, tone, text);
        }
        card(ui, false, |ui| {
            ui.horizontal(|ui| {
                switch(ui, &mut true, "Origen");
                switch(ui, &mut false, "Apagado");
                ui.add_enabled_ui(false, |ui| primary_button(ui, Some(icon::DOWNLOAD), "Deshabilitado"));
                ui.label(RichText::new("Áé ñ ¿¡ · « » € → ×").color(p.text));
            });
        });
        ui.horizontal_wrapped(|ui| {
            for i in [
                icon::ADD,
                icon::ARCHIVE,
                icon::BACK,
                icon::CHECK,
                icon::CLOSE,
                icon::COLLAPSED,
                icon::COMBINE,
                icon::CSV,
                icon::DELETE,
                icon::DOWNLOAD,
                icon::EDIT,
                icon::ERROR,
                icon::EXPANDED,
                icon::FILE_ADD,
                icon::FOLDER,
                icon::FORWARD,
                icon::HOME,
                icon::INFO,
                icon::LAYERS,
                icon::MERGE,
                icon::MOON,
                icon::MORE,
                icon::REFRESH,
                icon::RELATE,
                icon::SEARCH,
                icon::SETTINGS,
                icon::SHEET,
                icon::SIDEBAR,
                icon::SPLIT,
                icon::SPREADSHEET,
                icon::SUCCESS,
                icon::SUN,
                icon::SYSTEM,
                icon::TABLE,
                icon::WARNING,
            ] {
                icon_label(ui, i, 20.0, p.text);
            }
        });
        ui.add_space(8.0);
        data_grid(ui, "sample", &Sample);
    });
}

fn snapshot(theme: Theme, name: &str) {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(980.0, 900.0))
        .with_pixels_per_point(2.0)
        .with_theme(theme)
        .with_context_setup(splicr_ui::install)
        .build_ui(gallery);
    harness.run();
    harness.snapshot(name);
}

#[test]
fn gallery_light() {
    snapshot(Theme::Light, "gallery_light");
}

#[test]
fn gallery_dark() {
    snapshot(Theme::Dark, "gallery_dark");
}
