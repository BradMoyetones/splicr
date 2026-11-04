//! Splicr's integration with the operating system.
//!
//! One facade, one module per system behind it: every function below exists on every system, so
//! the app builds anywhere without `#[cfg]`s of its own. `windows` wraps a few Win32 calls (the only
//! `unsafe` in Splicr); `unix` serves macOS and Linux.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as os;
#[cfg(windows)]
use windows as os;

pub use os::{
    config_dir, is_installed, keep_title_bar_theme, open, repaint_title_bar_when_shown, reveal, tint_title_bar,
    title_bar_double_click, use_exe_icon,
};

/// The app id: the macOS bundle identifier, Linux's `.desktop` file and window class. Never change
/// it: macOS ties the app's permissions and settings to it.
pub const APP_ID: &str = "io.github.bradmoyetones.splicr";

/// What double-clicking a window's title bar does: System Settings › Desktop & Dock on macOS; a
/// zoom (maximize) everywhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleBarDoubleClick {
    Zoom,
    Minimize,
    Nothing,
}

/// The user's languages, most wanted first, as tags like `es-CO`: the system's setting, then (on
/// macOS and Linux) the locale variables. Read as ArtCraft's launcher reads them (`sys-locale`).
pub fn preferred_languages() -> Vec<String> {
    let mut tags: Vec<String> = sys_locale::get_locales().collect();
    for tag in os::locale_variables() {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}
