//! Data: the tables open, listed by file on the left, and the one picked in the grid.

use std::borrow::Cow;

use eframe::egui::{self, Align, Frame, Layout, Margin, RichText, vec2};
use splicr_core::{Format, Table, column_letters};
use splicr_ui::{
    GridData, Palette, badge, card, danger_icon_button, data_grid, icon, icon_label, list_item, primary_button,
    semibold, switch,
};

use crate::app::{App, group};
use crate::dialogs::Dialog;
use crate::i18n::{tr, trn};
use crate::router::Route;
use crate::views::{empty_state, section_title};
use crate::workspace::TableKey;

pub fn show(app: &mut App, ui: &mut egui::Ui, key: Option<TableKey>) {
    if app.workspace.table_count() == 0 {
        empty_state(
            ui,
            icon::TABLE,
            tr!("No tables open"),
            tr!("Open CSV or spreadsheet files and their tables show up here."),
            |ui| {
                if primary_button(ui, Some(icon::FILE_ADD), tr!("Open files")).clicked() {
                    app.dialog = Some(Dialog::Open);
                }
            },
        );
        return;
    }
    // Shows the first table when none is picked, or the one picked was closed.
    let key = key.filter(|k| app.workspace.table(*k).is_some()).or_else(|| app.workspace.keys().next());
    let p = *Palette::of(ui);
    egui::Panel::left("tables")
        .default_size(280.0)
        .size_range(220.0..=440.0)
        .resizable(true)
        .frame(Frame::new().inner_margin(Margin { left: 0, right: 16, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                section_title(ui, tr!("Open files"));
                ui.label(RichText::new(app.workspace.entries.len().to_string()).color(p.muted));
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| file_list(app, ui, key));
        });
    egui::CentralPanel::default()
        .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 16, right: 0, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            let Some(key) = key else { return };
            let Some(table) = app.workspace.table(key).cloned() else { return };
            ui.horizontal(|ui| {
                ui.label(RichText::new(&table.name).font(semibold(16.0)).color(p.text));
                badge(ui, &tr!("{rows} × {cols}", rows = group(table.rows()), cols = group(table.cols())));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(tr!("First row as header")).color(p.muted));
                    switch(ui, &mut app.first_row_header, tr!("First row as header"));
                });
            });
            ui.add_space(6.0);
            let view = TableView { table: &table, header: app.first_row_header };
            let picked = data_grid(ui, (key.source, key.index, view.header), &view);
            app.cell = picked.selected.map(|(row, col)| {
                let address = format!("{}{}", column_letters(col), row + 1 + usize::from(view.header));
                format!("{address}: {}", view.cell(row, col))
            });
        });
}

/// Each open file as a card: its name, where it came from and a button to close it, then its
/// tables as a list to pick from.
fn file_list(app: &mut App, ui: &mut egui::Ui, current: Option<TableKey>) {
    let p = *Palette::of(ui);
    let mut close = None;
    let mut go = None;
    for entry in &app.workspace.entries {
        card(ui, false, |ui| {
            ui.set_width(ui.available_width());
            let glyph = match (entry.source.entry.is_some(), entry.source.format) {
                (true, _) => icon::ARCHIVE,
                (false, Format::Csv) => icon::CSV,
                _ => icon::SPREADSHEET,
            };
            let tables = entry.source.tables.len();
            let detail = match entry.source.archive_name() {
                Some(archive) => tr!("{format} in {archive}", format = entry.source.format.label(), archive = archive),
                None => format!("{} · {}", entry.source.format.label(), trn!("{n} table|{n} tables", tables)),
            };
            // The close button first, from the right: the name takes what is left, and is cut there.
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 36.0), Layout::right_to_left(Align::Center), |ui| {
                if danger_icon_button(ui, icon::CLOSE, tr!("Close this file")).clicked() {
                    close = Some(entry.id);
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    icon_label(ui, glyph, 18.0, p.accent);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let name = RichText::new(entry.source.file_name()).font(semibold(13.5)).color(p.text);
                        ui.add(egui::Label::new(name).truncate());
                        ui.add(egui::Label::new(RichText::new(detail).small().color(p.muted)).truncate());
                    });
                });
            });
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for (index, table) in entry.source.tables.iter().enumerate() {
                let key = TableKey { source: entry.id, index };
                let rows = group(table.rows());
                if list_item(ui, icon::SHEET, &table.name, &rows, current == Some(key)).clicked() {
                    go = Some(key);
                }
            }
        });
        ui.add_space(8.0);
    }
    if let Some(id) = close {
        app.close_source(id);
    }
    if let Some(key) = go {
        app.router.go(Route::Data(Some(key)));
    }
}

/// A table as the grid shows it: with its first row as the header, or as rows like the others.
struct TableView<'a> {
    table: &'a Table,
    header: bool,
}

impl GridData for TableView<'_> {
    fn rows(&self) -> usize {
        self.table.rows().saturating_sub(usize::from(self.header))
    }

    fn cols(&self) -> usize {
        self.table.cols()
    }

    fn column_title(&self, col: usize) -> Cow<'_, str> {
        let letter = column_letters(col);
        match self.header {
            true => match self.table.get(0, col).display().trim() {
                "" => Cow::Owned(letter),
                name => Cow::Owned(name.to_owned()),
            },
            false => Cow::Owned(letter),
        }
    }

    fn cell(&self, row: usize, col: usize) -> Cow<'_, str> {
        self.table.get(row + usize::from(self.header), col).display()
    }

    fn numeric(&self, row: usize, col: usize) -> bool {
        self.table.get(row + usize::from(self.header), col).is_numeric()
    }
}
