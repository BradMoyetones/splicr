//! Combine: how the tables become files (one workbook, a file each, or one stacked table), which
//! tables and under which names, and where the files go. The export runs on the engine's thread,
//! with its progress here.

use eframe::egui::{self, Align, Layout, RichText};
use splicr_core::names::{file_stem, unique_file_names, unique_sheet_names};
use splicr_core::plan::{Mode, OutputFormat};
use splicr_ui::{Palette, Tone, banner, button, card, choice_card, icon, primary_button, semibold, switch};

use crate::app::{App, Export, group};
use crate::dialogs::Dialog;
use crate::i18n::{tr, trn};
use crate::router::Route;
use crate::views::{empty_state, section_title};
use crate::workspace::TableKey;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    if app.workspace.table_count() == 0 {
        empty_state(
            ui,
            icon::COMBINE,
            tr!("Nothing to combine yet"),
            tr!("Open two or more CSV or spreadsheet files, then come back here."),
            |ui| {
                if primary_button(ui, Some(icon::FILE_ADD), tr!("Open files")).clicked() {
                    app.dialog = Some(Dialog::Open);
                }
            },
        );
        return;
    }
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.set_max_width(980.0);
        mode(app, ui);
        ui.add_space(18.0);
        tables(app, ui);
        ui.add_space(18.0);
        output(app, ui);
        ui.add_space(18.0);
        run(app, ui);
    });
}

fn mode(app: &mut App, ui: &mut egui::Ui) {
    section_title(ui, tr!("1. How to combine them"));
    ui.add_space(4.0);
    let current = app.workspace.export.mode;
    let options = [
        (Mode::Workbook, icon::LAYERS, tr!("One workbook"), tr!("A single xlsx file, with one sheet for each table.")),
        (Mode::Files, icon::SPLIT, tr!("A file each"), tr!("Each table in a file of its own, xlsx or CSV.")),
        (
            Mode::Stack,
            icon::MERGE,
            tr!("One stacked table"),
            tr!("Every table's rows one after another, columns matched by their header."),
        ),
    ];
    ui.columns(options.len(), |columns| {
        for (ui, (mode, glyph, title, detail)) in columns.iter_mut().zip(options) {
            if choice_card(ui, current == mode, glyph, title, detail).clicked() {
                app.workspace.export.mode = mode;
            }
        }
    });
}

fn tables(app: &mut App, ui: &mut egui::Ui) {
    let p = *Palette::of(ui);
    let (taken, total) = (app.workspace.included(), app.workspace.table_count());
    ui.horizontal(|ui| {
        section_title(ui, tr!("2. Tables"));
        ui.label(RichText::new(tr!("{taken} of {total}", taken = taken, total = total)).color(p.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = if taken == total { tr!("Take none") } else { tr!("Take all") };
            if ui.button(label).clicked() {
                app.workspace.toggle_all();
            }
        });
    });
    let hint = match app.workspace.export.mode {
        Mode::Workbook => tr!("The name is the sheet's name in the workbook."),
        Mode::Files => tr!("The name is the file's name."),
        Mode::Stack => tr!("With the column of origin on, the name fills it for that table's rows."),
    };
    ui.label(RichText::new(hint).small().color(p.muted));
    ui.add_space(4.0);
    let keys: Vec<TableKey> = app.workspace.keys().collect();
    let mut show = None;
    card(ui, false, |ui| {
        ui.set_width(ui.available_width());
        for key in keys {
            let Some(entry) = app.workspace.entry(key.source) else { continue };
            let file = entry.source.file_name();
            let rows = entry.source.tables.get(key.index).map_or(0, |t| t.rows());
            let Some(pick) = app.workspace.pick_mut(key) else { continue };
            ui.horizontal(|ui| {
                ui.checkbox(&mut pick.include, "");
                ui.add_enabled_ui(pick.include, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut pick.name).desired_width(240.0));
                });
                let detail = tr!("from {file} · {rows} rows", file = file, rows = group(rows));
                ui.add(egui::Label::new(RichText::new(detail).small().color(p.muted)).truncate());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if splicr_ui::icon_button(ui, icon::TABLE, tr!("Show this table")).clicked() {
                        show = Some(key);
                    }
                });
            });
        }
    });
    if let Some(key) = show {
        app.router.go(Route::Data(Some(key)));
    }
}

fn output(app: &mut App, ui: &mut egui::Ui) {
    let p = *Palette::of(ui);
    section_title(ui, tr!("3. Where to save"));
    ui.add_space(4.0);
    let export = &mut app.workspace.export;
    card(ui, false, |ui| {
        ui.set_width(ui.available_width());
        egui::Grid::new("output").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
            ui.label(RichText::new(tr!("Folder")).color(p.muted));
            ui.horizontal(|ui| {
                let folder =
                    export.folder.as_ref().map_or(tr!("None chosen yet").to_owned(), |f| f.display().to_string());
                ui.add(egui::Label::new(RichText::new(folder).color(p.text)).truncate());
                if button(ui, Some(icon::FOLDER), tr!("Choose…")).clicked() {
                    app.dialog = Some(Dialog::ExportFolder);
                }
            });
            ui.end_row();

            if export.mode != Mode::Files {
                ui.label(RichText::new(tr!("File name")).color(p.muted));
                ui.add(egui::TextEdit::singleline(&mut export.file_name).desired_width(280.0));
                ui.end_row();
            }
            if export.mode != Mode::Workbook {
                ui.label(RichText::new(tr!("Format")).color(p.muted));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut export.format, OutputFormat::Xlsx, tr!("Excel (xlsx)"));
                    ui.selectable_value(&mut export.format, OutputFormat::Csv, tr!("CSV (UTF-8)"));
                });
                ui.end_row();
            }
            if export.mode == Mode::Stack {
                ui.label(RichText::new(tr!("Origin")).color(p.muted));
                ui.horizontal(|ui| {
                    switch(ui, &mut export.source_column, tr!("Add a column with each row's table"));
                    ui.label(RichText::new(tr!("Add a column with each row's table")).color(p.text));
                });
                ui.end_row();
            }
        });
    });
}

fn run(app: &mut App, ui: &mut egui::Ui) {
    let p = *Palette::of(ui);
    let plan = app.workspace.plan(tr!("Origin"));
    // What will be written, from the names alone: stacking builds its table only when it runs.
    if let Some(plan) = &plan {
        let ext = plan.format.extension();
        let files: Vec<String> = match plan.mode {
            Mode::Files => unique_file_names(plan.items.iter().map(|i| i.name.as_str()), ext),
            Mode::Workbook | Mode::Stack => vec![format!("{}.{ext}", file_stem(&plan.file_name))],
        };
        let summary = match plan.mode {
            Mode::Workbook => {
                let sheets = unique_sheet_names(plan.items.iter().map(|i| i.name.as_str()));
                tr!(
                    "{file}, with {sheets}: {names}",
                    file = files[0],
                    sheets = trn!("{n} sheet|{n} sheets", sheets.len()),
                    names = sheets.join(", ")
                )
            }
            Mode::Stack => tr!(
                "{file}, with the rows of {tables}",
                file = files[0],
                tables = trn!("{n} table|{n} tables", plan.items.len())
            ),
            Mode::Files => {
                tr!("{files}: {names}", files = trn!("{n} file|{n} files", files.len()), names = files.join(", "))
            }
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr!("It will write")).color(p.muted));
            ui.add(egui::Label::new(RichText::new(summary).color(p.text)).wrap());
        });
        ui.add_space(8.0);
    } else if app.workspace.export.folder.is_none() {
        banner(ui, Tone::Info, tr!("Choose the folder where the files go."));
        ui.add_space(8.0);
    } else {
        banner(ui, Tone::Info, tr!("Take at least one table."));
        ui.add_space(8.0);
    }

    let running = matches!(app.export, Export::Running { .. });
    ui.horizontal(|ui| {
        let can = plan.is_some() && !running;
        if ui.add_enabled_ui(can, |ui| primary_button(ui, Some(icon::DOWNLOAD), tr!("Export"))).inner.clicked()
            && let Some(plan) = plan
        {
            app.start_export(plan, false);
        }
    });
    ui.add_space(8.0);

    let mut clear = false;
    match &app.export {
        Export::Idle => {}
        Export::Running { file, done, total } => {
            card(ui, true, |ui| {
                ui.set_width(ui.available_width());
                let name = crate::app::file_name(file);
                ui.label(RichText::new(tr!("Writing {file}…", file = name)).font(semibold(14.0)).color(p.text));
                let progress = if *total == 0 { 0.0 } else { *done as f32 / *total as f32 };
                ui.add(egui::ProgressBar::new(progress).show_percentage().fill(p.accent));
                ui.horizontal(|ui| {
                    let rows = tr!("{done} of {total} rows", done = group(*done), total = group(*total));
                    ui.label(RichText::new(rows).small().color(p.muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if button(ui, None, tr!("Cancel")).clicked() {
                            app.engine.cancel();
                        }
                    });
                });
            });
        }
        Export::Done { written } => {
            let mut reveal = None;
            let mut open = None;
            card(ui, false, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    splicr_ui::icon_label(ui, icon::SUCCESS, 18.0, p.success);
                    let text = trn!("{n} file written|{n} files written", written.len());
                    ui.label(RichText::new(text).font(semibold(14.0)).color(p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if splicr_ui::icon_button(ui, icon::CLOSE, tr!("Close notice")).clicked() {
                            clear = true;
                        }
                        if written.len() == 1 && button(ui, None, tr!("Open")).clicked() {
                            open = written.first().cloned();
                        }
                        if button(ui, Some(icon::FOLDER), tr!("Show in folder")).clicked() {
                            reveal = written.first().cloned();
                        }
                    });
                });
            });
            if let Some(path) = reveal {
                splicr_platform::reveal(&path);
            }
            if let Some(path) = open {
                splicr_platform::open(&path.to_string_lossy());
            }
        }
        Export::Failed { written, error } => {
            let text = if written.is_empty() {
                tr!("Couldn't export: {error}", error = error)
            } else {
                tr!("The export stopped after {count} files: {error}", count = written.len(), error = error)
            };
            banner(ui, Tone::Danger, &text);
        }
    }
    if clear {
        app.export = Export::Idle;
    }
}
