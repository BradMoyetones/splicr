//! The "new version" banner: checks for releases in the background, then downloads, verifies and
//! installs one when the user asks. From Remapr's: once the package is in place (the MSI running on
//! Windows, the new app swapped in on macOS, the new AppImage on Linux) Splicr quits and the new one
//! starts. Copies that can't replace themselves (a deb, an app run from its DMG) offer the release
//! page instead.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui::{self, Align, Layout, RichText};
use splicr_ui::{Palette, button, card, icon, icon_button, icon_label, primary_button, semibold, small_icon_button};
use splicr_update::{Error, Installer, Release, Source};

use crate::i18n::tr;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Between background checks. The first one comes a few seconds after start.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

enum Event {
    Found(Release),
    /// A check the user asked for found nothing newer.
    UpToDate,
    CheckFailed(String),
    Progress(f32),
    Installing,
    /// The new version is in place, or its installer running: quit so it can start.
    QuitForInstaller,
    Failed(String),
}

enum State {
    Idle,
    Available(Release),
    Downloading(Release, f32),
    Installing(Release),
    Failed(Release, String),
}

enum Action {
    Update,
    Later,
    Close,
    Notes(String),
}

pub struct Updates {
    state: State,
    source: Option<Arc<Source>>,
    /// A check the user asked for is under way.
    checking: bool,
    /// What that check found, when it wasn't a new version.
    message: Option<String>,
    /// "Más tarde" on this version: not offered again until Splicr restarts.
    dismissed: Option<String>,
    events: Receiver<Event>,
    sender: Sender<Event>,
    /// How this copy puts a new version in place; `None` when it can't, and the release page is
    /// offered instead.
    installer: Option<Installer>,
}

impl Updates {
    /// Checks in the background when there is a `source`: the installed app, or a debug build
    /// pointed at a test folder.
    pub fn new(ctx: &egui::Context, source: Option<Source>) -> Self {
        let (sender, events) = mpsc::channel();
        let source = source.map(Arc::new);
        if let Some(source) = source.clone() {
            let (sender, ctx) = (sender.clone(), ctx.clone());
            let _ = std::thread::Builder::new().name("splicr-updates".into()).spawn(move || {
                std::thread::sleep(Duration::from_secs(5));
                loop {
                    // A failed background check isn't worth bothering the user: the next one comes later.
                    match source.check(VERSION) {
                        Ok(Some(release)) => {
                            log::info!("version {} is out", release.version);
                            let _ = sender.send(Event::Found(release));
                            ctx.request_repaint();
                        }
                        Ok(None) => {}
                        Err(e) => log::warn!("update check failed: {e:?}"),
                    }
                    std::thread::sleep(CHECK_EVERY);
                }
            });
        }
        let installer = splicr_update::installer();
        Self { state: State::Idle, source, checking: false, message: None, dismissed: None, events, sender, installer }
    }

    /// Whether this copy updates at all: the installed app, or a debug build pointed at a folder.
    pub fn available(&self) -> bool {
        self.source.is_some()
    }

    /// The "Buscar actualizaciones" button, for the copies that update at all.
    pub fn check_button(&mut self, ui: &mut egui::Ui) {
        if self.source.is_none() {
            return;
        }
        if self.checking {
            ui.add(egui::Spinner::new().size(14.0));
        } else if button(ui, Some(icon::REFRESH), tr!("Check for updates")).clicked() {
            self.check_now(ui.ctx());
        }
    }

    /// The status bar's quiet version of [`Self::check_button`]: a small icon, as in Remapr's footer.
    pub fn check_icon(&mut self, ui: &mut egui::Ui) {
        if self.source.is_none() {
            return;
        }
        if self.checking {
            ui.add(egui::Spinner::new().size(12.0));
        } else if small_icon_button(ui, icon::REFRESH, tr!("Check for updates")).clicked() {
            self.check_now(ui.ctx());
        }
    }

    fn check_now(&mut self, ctx: &egui::Context) {
        let Some(source) = self.source.clone() else { return };
        if !matches!(self.state, State::Idle | State::Available(_) | State::Failed(..)) {
            return;
        }
        // Asked for: offer what it finds even if "Más tarde" set it aside.
        (self.checking, self.message, self.dismissed) = (true, None, None);
        let (sender, ctx) = (self.sender.clone(), ctx.clone());
        let _ = std::thread::Builder::new().name("splicr-update-check".into()).spawn(move || {
            let event = match source.check(VERSION) {
                Ok(Some(release)) => Event::Found(release),
                Ok(None) => Event::UpToDate,
                Err(e) => {
                    log::warn!("update check failed: {e:?}");
                    Event::CheckFailed(describe(&e))
                }
            };
            let _ = sender.send(event);
            ctx.request_repaint();
        });
    }

    /// Takes in what the background threads report. `true` when Splicr must quit for the installer.
    pub fn poll(&mut self) -> bool {
        let mut quit = false;
        while let Ok(event) = self.events.try_recv() {
            match &event {
                Event::Found(_) => self.checking = false,
                Event::UpToDate => {
                    self.checking = false;
                    self.message = Some(tr!("You have the latest version (v{version}).", version = VERSION));
                }
                Event::CheckFailed(e) => {
                    self.checking = false;
                    self.message = Some(tr!("Couldn't check for updates. {error}", error = e));
                }
                _ => {}
            }
            let state = std::mem::replace(&mut self.state, State::Idle);
            self.state = match (state, event) {
                (State::Idle | State::Available(_), Event::Found(r)) if self.dismissed.as_ref() != Some(&r.version) => {
                    State::Available(r)
                }
                (State::Downloading(r, _), Event::Progress(p)) => State::Downloading(r, p),
                (State::Downloading(r, _), Event::Installing) => State::Installing(r),
                (State::Downloading(r, _) | State::Installing(r), Event::Failed(e)) => State::Failed(r, e),
                (state, Event::QuitForInstaller) => {
                    quit = true;
                    state
                }
                (state, _) => state,
            };
        }
        quit
    }

    fn start(&mut self, ctx: &egui::Context, release: Release) {
        let Some(installer) = self.installer.clone() else { return };
        self.state = State::Downloading(release.clone(), 0.0);
        let (sender, ctx) = (self.sender.clone(), ctx.clone());
        let _ = std::thread::Builder::new().name("splicr-update".into()).spawn(move || {
            let send = |event| {
                let _ = sender.send(event);
                ctx.request_repaint();
            };
            let dir = std::env::temp_dir().join("Splicr").join("update");
            let result = splicr_update::download(&release, &dir, |p| send(Event::Progress(p))).and_then(|package| {
                send(Event::Installing);
                splicr_update::install(&installer, &package)?;
                let _ = std::fs::remove_file(&package);
                send(Event::QuitForInstaller);
                Ok(())
            });
            if let Err(e) = result {
                log::error!("update to {} failed: {e:?}", release.version);
                send(Event::Failed(describe(&e)));
            }
        });
    }

    /// The banner, when there is something to say.
    pub fn banner(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        let installs = self.installer.is_some();
        let mut action = None;
        match &self.state {
            State::Idle => {
                let Some(message) = &self.message else { return };
                card(ui, false, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(RichText::new(message).color(p.text)).wrap());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon_button(ui, icon::CLOSE, tr!("Close notice")).clicked() {
                                action = Some(Action::Close);
                            }
                        });
                    });
                });
            }
            State::Available(release) => {
                card(ui, true, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        icon_label(ui, icon::DOWNLOAD, 18.0, p.accent);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let title = tr!("New version available: v{version}", version = release.version);
                            ui.label(RichText::new(title).font(semibold(14.0)).color(p.text));
                            let detail = if installs {
                                tr!("You have v{version}. It installs itself and Splicr restarts.", version = VERSION)
                            } else if cfg!(target_os = "macos") {
                                tr!(
                                    "You have v{version}. Download it and drag Splicr to Applications.",
                                    version = VERSION
                                )
                            } else {
                                tr!("You have v{version}. Download it from its page.", version = VERSION)
                            };
                            ui.label(RichText::new(detail).small().color(p.muted));
                        });
                    });
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        if ui.link(RichText::new(tr!("What's new")).color(p.accent)).clicked() {
                            action = Some(Action::Notes(release.notes_url.clone()));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if !installs {
                                if primary_button(ui, Some(icon::DOWNLOAD), tr!("Download")).clicked() {
                                    action = Some(Action::Notes(release.notes_url.clone()));
                                }
                            } else if primary_button(ui, None, tr!("Update and restart")).clicked() {
                                action = Some(Action::Update);
                            }
                            if ui.button(tr!("Later")).clicked() {
                                action = Some(Action::Later);
                            }
                        });
                    });
                });
            }
            State::Downloading(release, progress) => {
                card(ui, true, |ui| {
                    ui.set_width(ui.available_width());
                    let text = tr!("Downloading v{version}…", version = release.version);
                    ui.label(RichText::new(text).font(semibold(14.0)).color(p.text));
                    ui.add(egui::ProgressBar::new(*progress).show_percentage().fill(p.accent));
                });
            }
            State::Installing(release) => {
                card(ui, true, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spinner();
                        let text =
                            tr!("Installing v{version}. Splicr will restart by itself.", version = release.version);
                        ui.label(RichText::new(text).color(p.text));
                    });
                });
            }
            State::Failed(_, error) => {
                card(ui, false, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        icon_label(ui, icon::ERROR, 16.0, p.danger);
                        ui.add(
                            egui::Label::new(
                                RichText::new(tr!("Couldn't update. {error}", error = error)).color(p.danger),
                            )
                            .wrap(),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if primary_button(ui, None, tr!("Retry")).clicked() {
                                action = Some(Action::Update);
                            }
                            if ui.button(tr!("Close")).clicked() {
                                action = Some(Action::Close);
                            }
                        });
                    });
                });
            }
        }
        ui.add_space(4.0);

        match action {
            Some(Action::Notes(url)) => splicr_platform::open(&url),
            Some(Action::Update) => {
                if let State::Available(release) | State::Failed(release, _) =
                    std::mem::replace(&mut self.state, State::Idle)
                {
                    self.start(ui.ctx(), release);
                }
            }
            Some(Action::Later) => {
                if let State::Available(release) = std::mem::replace(&mut self.state, State::Idle) {
                    self.dismissed = Some(release.version);
                }
            }
            Some(Action::Close) => (self.state, self.message) = (State::Idle, None),
            None => {}
        }
    }
}

/// An updater error in the user's words.
fn describe(error: &Error) -> String {
    match error {
        Error::Network(detail) if detail.is_empty() => tr!("No connection.").to_owned(),
        Error::Network(detail) => tr!("No connection ({detail}).", detail = detail),
        Error::GitHub(detail) => tr!("GitHub answered something unexpected ({detail}).", detail = detail),
        Error::UnreadableChecksum => tr!("The published SHA-256 can't be read.").to_owned(),
        Error::ChecksumMismatch => {
            tr!("The download doesn't match its published SHA-256; it was discarded.").to_owned()
        }
        Error::BadSignature => {
            tr!("The download isn't signed by Splicr's project; it was discarded, for your safety.").to_owned()
        }
        Error::File(detail) => tr!("A file couldn't be saved ({detail}).", detail = detail),
        Error::Replace(detail) => {
            tr!("The new version couldn't take the old one's place, which stays ({detail}).", detail = detail)
        }
        Error::Installer(Some(1602)) => tr!("The update was cancelled.").to_owned(),
        Error::Installer(Some(1618)) => {
            tr!("Another installation is running. Wait for it to finish and try again.").to_owned()
        }
        Error::Installer(Some(code)) => tr!("Windows Installer stopped with error {code}.", code = code),
        Error::Installer(None) => tr!("Windows Installer couldn't start or was interrupted.").to_owned(),
    }
}
