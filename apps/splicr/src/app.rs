//! The window: the sidebar, the header, the status bar, and the screen of the current route in
//! between (`views`). It talks to the engine (files are read and written on its thread), the file
//! dialogs and the updater, and keeps the settings saved.
//!
//! What the user has open and what an export takes is the [`Workspace`]'s; where they are is the
//! [`Router`]'s. The screens change both; this module draws the frame around them.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui::{
    self, Align, Frame, Key, KeyboardShortcut, Layout, Margin, Modifiers, Rect, RichText, ThemePreference,
    ViewportCommand, pos2, vec2,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use splicr_engine::{Command, Engine, Error, Event, Warning};
use splicr_platform::TitleBarDoubleClick;
use splicr_ui::{Palette, Tone, button, icon, icon_button, nav_item, semibold, title_bar};

use crate::config::{Config, Theme};
use crate::dialogs::{self, Dialog, Picked};
use crate::i18n::{tr, trn};
use crate::router::{Route, Router, Section};
use crate::updates::{self, Updates};
use crate::views;
use crate::workspace::{TableKey, Workspace};

/// The executable's icon resource (`build.rs`).
pub const ICON: u16 = 1;
/// On macOS the window's content runs under its transparent title bar: this much room is left at
/// the top for the native window buttons.
const MACOS_TITLE_BAR: f32 = 28.0;
/// The header's height, and the sidebar's brand block's.
const HEADER_H: f32 = 72.0;
/// The sidebar's padding on each side. Folded, the sidebar is `2 * (SIDEBAR_MARGIN + RAIL)` = 72
/// points: room for macOS's window buttons, which sit over it.
const SIDEBAR_MARGIN: f32 = 18.0;
/// Where the sidebar's logo and icons are centred, from the inside of its padding: the same folded
/// and unfolded, so nothing moves as it folds. Half of a navigation item's 36 points.
const RAIL: f32 = 18.0;
const LOGO: &[u8] = include_bytes!("../../../assets/logo-light.svg");
const LOGO_URI: &str = "bytes://splicr/logo.svg";
/// The most messages shown at once; older ones give way.
const NOTICES: usize = 4;

/// A message above the screen, until closed.
pub struct Notice {
    pub tone: Tone,
    pub text: String,
    /// A file to show in its folder, with a button.
    pub reveal: Option<PathBuf>,
}

/// The files being read: the one now, of how many.
pub struct Opening {
    pub file: String,
    pub index: usize,
    pub count: usize,
}

/// Where the export stands.
pub enum Export {
    Idle,
    Running { file: PathBuf, done: usize, total: usize },
    Done { written: Vec<PathBuf> },
    Failed { written: Vec<PathBuf>, error: String },
}

pub struct App {
    pub config: Config,
    /// The config changed since it was last saved.
    pub unsaved: bool,
    pub workspace: Workspace,
    pub router: Router,
    pub engine: Engine,
    pub opening: Option<Opening>,
    pub export: Export,
    pub notices: Vec<Notice>,
    /// A file dialog asked for, shown at the end of the frame.
    pub dialog: Option<Dialog>,
    answers: Receiver<Picked>,
    answer_sender: Sender<Picked>,
    /// Files read lately go to Data when they arrive.
    show_opened: bool,
    /// The table Data showed last, to come back to it from the sidebar.
    pub last_table: Option<TableKey>,
    /// The cell picked in the grid, for the status bar.
    pub cell: Option<String>,
    /// Data reads each table's first row as its header.
    pub first_row_header: bool,
    /// Files an export would replace, waiting for the user to say yes.
    pub confirm: Option<(splicr_core::plan::Plan, Vec<PathBuf>)>,
    pub updates: Updates,
    hwnd: Option<isize>,
    /// Whether the title bar was last tinted for the dark theme; `None` before the first time.
    title_bar_dark: Option<bool>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        splicr_ui::install(&cc.egui_ctx);
        cc.egui_ctx.include_bytes(LOGO_URI, LOGO);
        let hwnd = match cc.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
            _ => None,
        };
        if let Some(hwnd) = hwnd {
            splicr_platform::use_exe_icon(hwnd, ICON);
            splicr_platform::repaint_title_bar_when_shown(hwnd);
        }
        let (mut config, load_warning) = Config::load();
        let mut notices: Vec<Notice> =
            load_warning.into_iter().map(|text| Notice { tone: Tone::Warning, text, reveal: None }).collect();
        if config.last_version != updates::VERSION {
            if !config.last_version.is_empty() {
                let text = tr!("Splicr was updated to version {version}.", version = updates::VERSION);
                notices.push(Notice { tone: Tone::Success, text, reveal: None });
            }
            config.last_version = updates::VERSION.into();
            let _ = config.save();
        }
        let ctx = cc.egui_ctx.clone();
        let mut engine = Engine::start(move || ctx.request_repaint());
        let show_opened = !files.is_empty();
        if show_opened {
            engine.send(Command::Open(files));
        }
        let (answer_sender, answers) = mpsc::channel();
        let mut workspace = Workspace::default();
        workspace.export.folder = config.export_folder.clone();
        // Starts reading the system's setting now, so it is known by the first double-click.
        let _ = splicr_platform::title_bar_double_click();
        let app = Self {
            updates: Updates::new(&cc.egui_ctx, update_source()),
            config,
            unsaved: false,
            workspace,
            router: Router::default(),
            engine,
            opening: None,
            export: Export::Idle,
            notices,
            dialog: None,
            answers,
            answer_sender,
            show_opened,
            last_table: None,
            cell: None,
            first_row_header: true,
            confirm: None,
            hwnd,
            title_bar_dark: None,
        };
        app.apply_theme(&cc.egui_ctx);
        app
    }

    pub fn notify(&mut self, tone: Tone, text: String) {
        self.notices.push(Notice { tone, text, reveal: None });
        if self.notices.len() > NOTICES {
            self.notices.remove(0);
        }
    }

    /// Reads `paths` on the engine's thread; the tables appear in Data as they arrive.
    pub fn open(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        self.show_opened = true;
        self.engine.send(Command::Open(paths));
    }

    /// Starts the export, unless it would replace files: then it asks first.
    pub fn start_export(&mut self, plan: splicr_core::plan::Plan, replace: bool) {
        let existing = plan.existing(Path::exists);
        if !replace && !existing.is_empty() {
            self.confirm = Some((plan, existing));
            return;
        }
        self.config.export_folder = Some(plan.folder.clone());
        self.unsaved = true;
        self.export = Export::Running { file: PathBuf::new(), done: 0, total: 0 };
        self.engine.send(Command::Export(plan));
    }

    pub fn close_source(&mut self, id: u64) {
        self.workspace.remove(id);
        self.router.forget(|key| key.source == id);
        if self.last_table.is_some_and(|key| key.source == id) {
            self.last_table = None;
        }
    }

    /// egui also brings the native title bar along (`Options::sync_window_theme`).
    pub fn apply_theme(&self, ctx: &egui::Context) {
        let (preference, dark) = match self.config.theme {
            Theme::System => (ThemePreference::System, None),
            Theme::Light => (ThemePreference::Light, Some(false)),
            Theme::Dark => (ThemePreference::Dark, Some(true)),
        };
        ctx.set_theme(preference);
        splicr_platform::keep_title_bar_theme(dark);
    }

    fn persist(&mut self) {
        if !std::mem::take(&mut self.unsaved) {
            return;
        }
        if let Err(e) = self.config.save() {
            log::error!("couldn't save the settings: {e}");
            self.notify(Tone::Danger, tr!("Couldn't save the settings: {error}", error = e));
        }
    }

    fn handle_events(&mut self) {
        let events: Vec<Event> = self.engine.events().collect();
        for event in events {
            match event {
                Event::Opening { path, index, count } => {
                    self.opening = Some(Opening { file: file_name(&path), index, count });
                }
                Event::Opened { path, result } => {
                    if self.opening.as_ref().is_some_and(|o| o.index + 1 == o.count) {
                        self.opening = None;
                    }
                    match result {
                        Ok(opened) => {
                            for warning in &opened.warnings {
                                self.notify(Tone::Warning, describe_warning(&file_name(&path), warning));
                            }
                            let first = self.workspace.add(opened.sources);
                            self.config.remember(path);
                            self.unsaved = true;
                            if std::mem::take(&mut self.show_opened) && first.is_some() {
                                self.router.go(Route::Data(first));
                            }
                        }
                        Err(e) => {
                            let text =
                                tr!("Couldn't open {file}: {error}", file = file_name(&path), error = describe(&e));
                            self.notify(Tone::Danger, text);
                        }
                    }
                }
                Event::Exporting { file, done, total } => self.export = Export::Running { file, done, total },
                Event::Exported { written, error: None } => self.export = Export::Done { written },
                Event::Exported { written, error: Some(Error::Cancelled) } => {
                    let text = tr!("Export cancelled. Files finished before stay: {count}.", count = written.len());
                    self.export = Export::Idle;
                    self.notify(Tone::Info, text);
                }
                Event::Exported { written, error: Some(e) } => {
                    self.export = Export::Failed { written, error: describe(&e) };
                }
            }
        }
        while let Ok(picked) = self.answers.try_recv() {
            match picked {
                Picked::Files(paths) => self.open(paths),
                Picked::Folder(folder) => {
                    self.workspace.export.folder = Some(folder.clone());
                    self.config.export_folder = Some(folder);
                    self.unsaved = true;
                }
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let pressed = |key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, key)));
        if pressed(Key::O) {
            self.dialog = Some(Dialog::Open);
        }
        let sections = [(Key::Num1, Section::Home), (Key::Num2, Section::Data), (Key::Num3, Section::Combine)];
        for (key, section) in sections {
            if pressed(key) {
                self.router.go_to_section(section, self.last_table);
            }
        }
        if pressed(Key::Comma) {
            self.router.go(Route::Settings);
        }
        if pressed(Key::OpenBracket) {
            self.router.back();
        }
    }

    /// Files dropped on the window open; ones still being dragged light the drop zones up.
    fn dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect()
        });
        self.open(dropped);
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        let compact = self.config.compact_sidebar;
        // The brand block is as tall as the header. On macOS its top strip holds the window buttons,
        // and the logo sits in what is left below them.
        let (block, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), egui::Sense::hover());
        let mac = cfg!(target_os = "macos");
        if mac {
            self.title_bar(ui, block.expand2(vec2(SIDEBAR_MARGIN, 0.0)));
        }
        let center_y = (block.top() + if mac { MACOS_TITLE_BAR } else { 0.0 } + block.bottom()) / 2.0;
        // On the rail, as the icons below: nothing moves when the sidebar folds.
        let logo = Rect::from_center_size(pos2(block.left() + RAIL, center_y), vec2(28.0, 28.0));
        egui::Image::from_uri(LOGO_URI).paint_at(ui, logo);
        if !compact {
            let at = pos2(logo.right() + 10.0, center_y);
            ui.painter().text(at, egui::Align2::LEFT_CENTER, "Splicr", semibold(18.0), p.text);
        }
        ui.add_space(8.0);
        let current = self.router.current().section();
        let tables = self.workspace.table_count();
        let entries = [
            (Section::Home, icon::HOME, tr!("Home")),
            (Section::Data, icon::TABLE, tr!("Data")),
            (Section::Combine, icon::COMBINE, tr!("Combine")),
        ];
        for (section, glyph, label) in entries {
            let response = nav_item(ui, glyph, label, current == section, compact);
            if section == Section::Data && tables > 0 && !compact {
                let at = pos2(response.rect.right() - 12.0, response.rect.center().y);
                ui.painter().text(
                    at,
                    egui::Align2::RIGHT_CENTER,
                    tables.to_string(),
                    egui::FontId::proportional(12.0),
                    p.muted,
                );
            }
            if response.clicked() {
                self.router.go_to_section(section, self.last_table);
            }
        }
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            let label = if compact { tr!("Expand the sidebar") } else { tr!("Collapse the sidebar") };
            // A 32-point button, centred on the rail.
            let toggled = ui.horizontal(|ui| {
                ui.add_space(RAIL - 16.0);
                icon_button(ui, icon::SIDEBAR, label).clicked()
            });
            if toggled.inner {
                self.config.compact_sidebar = !compact;
                self.unsaved = true;
            }
            ui.add_space(4.0);
            if nav_item(ui, icon::SETTINGS, tr!("Settings"), current == Section::Settings, compact).clicked() {
                self.router.go(Route::Settings);
            }
        });
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        let (title, subtitle) = match self.router.current() {
            Route::Home => (tr!("Open your data"), tr!("Spreadsheets and CSV files, one or many at once")),
            Route::Data(_) => (tr!("Data"), tr!("The tables you have open")),
            Route::Combine => (tr!("Combine"), tr!("Join tables into the files you need")),
            Route::Settings => (tr!("Settings"), tr!("How Splicr looks and updates")),
        };
        // The actions first, from the right; the title takes what is left and is cut there.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            match self.router.current() {
                Route::Data(_) if self.workspace.table_count() > 0 => {
                    if splicr_ui::primary_button(ui, Some(icon::COMBINE), tr!("Combine")).clicked() {
                        self.router.go(Route::Combine);
                    }
                    if button(ui, Some(icon::FILE_ADD), tr!("Open files")).clicked() {
                        self.dialog = Some(Dialog::Open);
                    }
                }
                Route::Home | Route::Combine if button(ui, Some(icon::FILE_ADD), tr!("Open files")).clicked() => {
                    self.dialog = Some(Dialog::Open);
                }
                _ => {}
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                if self.router.can_go_back() && icon_button(ui, icon::BACK, tr!("Back")).clicked() {
                    self.router.back();
                }
                let width = ui.available_width().max(0.0);
                let line = |text: &str, font, color| {
                    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
                    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
                    ui.painter().layout_job(job)
                };
                let title = line(title, semibold(20.0), p.text);
                let subtitle = line(subtitle, egui::FontId::proportional(14.0), p.muted);
                let height = title.size().y + subtitle.size().y;
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
                let title_h = title.size().y;
                ui.painter().galley(rect.min, title, p.text);
                ui.painter().galley(pos2(rect.left(), rect.top() + title_h), subtitle, p.muted);
            });
        });
    }

    fn notices(&mut self, ui: &mut egui::Ui) {
        let mut close = None;
        let mut reveal = None;
        for (i, notice) in self.notices.iter().enumerate() {
            let (strong, soft) = Palette::of(ui).tone(notice.tone);
            Frame::new()
                .fill(soft)
                .stroke((1.0, strong.gamma_multiply(0.5)))
                .corner_radius(10)
                .inner_margin(Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        splicr_ui::icon_label(ui, notice.tone.icon(), 16.0, strong);
                        ui.add(egui::Label::new(RichText::new(&notice.text).color(strong)).wrap());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon_button(ui, icon::CLOSE, tr!("Close notice")).clicked() {
                                close = Some(i);
                            }
                            if notice.reveal.is_some() && ui.button(tr!("Show in folder")).clicked() {
                                reveal = notice.reveal.clone();
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
        if let Some(path) = reveal {
            splicr_platform::reveal(&path);
        }
        if let Some(i) = close {
            self.notices.remove(i);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        ui.horizontal(|ui| {
            if let Some(opening) = &self.opening {
                ui.add(egui::Spinner::new().size(12.0));
                let text = tr!(
                    "Reading {file} ({index} of {count})…",
                    file = opening.file,
                    index = opening.index + 1,
                    count = opening.count
                );
                ui.label(RichText::new(text).small().color(p.muted));
            } else if let Export::Running { done, total, .. } = &self.export {
                ui.add(egui::Spinner::new().size(12.0));
                let percent = if *total == 0 { 0 } else { done * 100 / total };
                ui.label(RichText::new(tr!("Exporting… {percent}%", percent = percent)).small().color(p.muted));
            } else {
                let tables = self.workspace.table_count();
                let text = if tables == 0 {
                    tr!("No files open").to_owned()
                } else {
                    trn!("{n} table open|{n} tables open", tables)
                };
                ui.label(RichText::new(text).small().color(p.muted));
            }
            if let (Route::Data(Some(_)), Some(cell)) = (self.router.current(), &self.cell) {
                ui.separator();
                ui.label(RichText::new(cell).small().color(p.text));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).small().color(p.muted));
                // Quiet, beside the version, as in Remapr's footer; what it finds shows above the screen.
                self.updates.check_icon(ui);
            });
        });
    }

    fn confirm_replace(&mut self, ui: &egui::Ui) {
        let Some((_, existing)) = &self.confirm else { return };
        let (ctx, p) = (ui.ctx().clone(), Palette::of(ui));
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("confirm_replace"))
            .backdrop_color(p.scrim)
            .frame(Frame::new().fill(p.surface).corner_radius(12).inner_margin(20).stroke((1.0, p.border)))
            .show(&ctx, |ui| {
                ui.set_width(420.0);
                let title = trn!("Replace {n} file?|Replace {n} files?", existing.len());
                ui.label(RichText::new(title).font(semibold(17.0)).color(p.text));
                ui.label(RichText::new(tr!("These files are already in the folder:")).color(p.muted));
                for path in existing.iter().take(6) {
                    ui.label(RichText::new(file_name(path)).color(p.text));
                }
                if existing.len() > 6 {
                    ui.label(RichText::new(tr!("and {count} more", count = existing.len() - 6)).color(p.muted));
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if splicr_ui::primary_button(ui, None, tr!("Replace")).clicked() {
                        choice = Some(true);
                    }
                    if button(ui, None, tr!("Cancel")).clicked() {
                        choice = Some(false);
                    }
                });
            });
        match choice {
            Some(true) => {
                if let Some((plan, _)) = self.confirm.take() {
                    self.start_export(plan, true);
                }
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    /// Windows: the native title bar in the palette's colours, so it and the app read as one.
    fn tint_title_bar(&mut self, ui: &egui::Ui) {
        let dark = ui.visuals().dark_mode;
        let Some(hwnd) = self.hwnd.filter(|_| self.title_bar_dark != Some(dark)) else { return };
        let p = Palette::of(ui);
        let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
        splicr_platform::tint_title_bar(hwnd, rgb(p.surface), rgb(p.text), rgb(p.border));
        self.title_bar_dark = Some(dark);
    }

    /// macOS: dragging the strip's empty part moves the window; a double-click does what the
    /// system's setting says.
    fn title_bar(&self, ui: &mut egui::Ui, rect: Rect) {
        let response = title_bar::empty_area(ui, rect);
        if !response.double_clicked() || title_bar::is_fullscreen(ui) {
            return;
        }
        let command = match splicr_platform::title_bar_double_click() {
            TitleBarDoubleClick::Zoom => ViewportCommand::Maximized(!title_bar::is_maximized(ui)),
            TitleBarDoubleClick::Minimize => ViewportCommand::Minimized(true),
            TitleBarDoubleClick::Nothing => return,
        };
        ui.ctx().send_viewport_cmd(command);
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events();
        self.dropped_files(ctx);
        self.shortcuts(ctx);
        if self.updates.poll() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if let Route::Data(Some(key)) = self.router.current() {
            self.last_table = Some(key);
        }
        self.persist();
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let p = *Palette::of(ui);
        self.tint_title_bar(ui);
        let mac = cfg!(target_os = "macos");
        // Folded, the sidebar is exactly the rail's width on both sides: its items are centred
        // without moving.
        let width = if self.config.compact_sidebar { 2.0 * (SIDEBAR_MARGIN + RAIL) } else { 224.0 };
        let margin = SIDEBAR_MARGIN as i8;
        // No frame stroke: it would take a point off each side and put the rail off centre when
        // folded. The panel draws its own edge.
        egui::Panel::left("sidebar")
            .exact_size(width)
            .resizable(false)
            .frame(Frame::new().fill(p.surface).inner_margin(Margin {
                left: margin,
                right: margin,
                top: 0,
                bottom: margin,
            }))
            .show(ui, |ui| self.sidebar(ui));
        egui::Panel::bottom("status")
            .frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(20, 6)))
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::top("header")
            .exact_size(HEADER_H)
            .frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(24, 0)))
            .show(ui, |ui| {
                if mac {
                    self.title_bar(ui, ui.max_rect().expand2(vec2(24.0, 0.0)));
                }
                self.header(ui);
            });
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 24, right: 24, top: 14, bottom: 12 }))
            .show(ui, |ui| {
                self.notices(ui);
                // A new version is offered on every screen; Settings shows it in its Updates section.
                if self.router.current() != Route::Settings {
                    self.updates.banner(ui);
                }
                match self.router.current() {
                    Route::Home => views::home::show(self, ui),
                    Route::Data(key) => views::data::show(self, ui, key),
                    Route::Combine => views::combine::show(self, ui),
                    Route::Settings => views::settings::show(self, ui),
                }
            });
        self.confirm_replace(ui);
        if let Some(dialog) = self.dialog.take() {
            let start_in = match dialog {
                Dialog::Open => self.config.recent.first().and_then(|p| p.parent()).map(Path::to_path_buf),
                Dialog::ExportFolder => self.workspace.export.folder.clone(),
            };
            dialogs::show(dialog, frame, start_in, self.answer_sender.clone(), ui.ctx().clone());
        }
        // Keeps the hover state of drop zones current while files are dragged over the window.
        if ui.input(|i| !i.raw.hovered_files.is_empty()) {
            ui.ctx().request_repaint();
        }
        self.persist();
    }
}

/// Releases come from GitHub for the installed app. A debug build can be pointed at a test folder
/// with `SPLICR_UPDATES_DIR` (see `splicr_update::Source::local`); other builds from source never
/// update.
fn update_source() -> Option<splicr_update::Source> {
    if splicr_platform::is_installed() {
        return Some(splicr_update::Source::github());
    }
    let dir = std::env::var_os("SPLICR_UPDATES_DIR").filter(|_| cfg!(debug_assertions))?;
    Some(splicr_update::Source::local(Path::new(&dir)))
}

pub fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// A file error in the user's words.
pub fn describe(error: &Error) -> String {
    match error {
        Error::Io(detail) => detail.clone(),
        Error::TooBig => tr!("it is bigger than 512 MB, the most Splicr reads.").to_owned(),
        Error::Unreadable(detail) => {
            tr!("it isn't a spreadsheet Splicr can read, or it is damaged ({detail}).", detail = detail)
        }
        Error::NothingInside => tr!("the archive holds no CSV or spreadsheet files.").to_owned(),
        Error::TooManyRows { sheet } => {
            tr!("“{sheet}” has more rows than a spreadsheet holds (1 048 576). Export it as CSV.", sheet = sheet)
        }
        Error::TooManyColumns { sheet } => {
            tr!("“{sheet}” has more columns than a spreadsheet holds (16 384).", sheet = sheet)
        }
        Error::Write(detail) => detail.clone(),
        Error::Cancelled => tr!("cancelled.").to_owned(),
    }
}

/// A reading warning in the user's words.
pub fn describe_warning(file: &str, warning: &Warning) -> String {
    match warning {
        Warning::RowsCut { table, kept } => {
            tr!(
                "{file}: only the first {rows} rows of “{table}” were read.",
                file = file,
                rows = group(*kept),
                table = table
            )
        }
        Warning::ColumnsCut { table, kept } => tr!(
            "{file}: only the first {columns} columns of “{table}” were read.",
            file = file,
            columns = group(*kept),
            table = table
        ),
        Warning::SheetSkipped { sheet, error } => {
            tr!("{file}: the sheet “{sheet}” couldn't be read ({error}).", file = file, sheet = sheet, error = error)
        }
        Warning::EntriesSkipped { names } => tr!(
            "{file}: {count} files inside weren't read, they aren't tables: {names}",
            file = file,
            count = names.len(),
            names = names.iter().take(5).map(String::as_str).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// `1234567` as `1 234 567`, grouped by thin spaces: read the same way in every language.
pub fn group(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 * 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202F}');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_grouped_by_thousands() {
        assert_eq!(group(7), "7");
        assert_eq!(group(1234), "1\u{202F}234");
        assert_eq!(group(1_048_576), "1\u{202F}048\u{202F}576");
    }
}
