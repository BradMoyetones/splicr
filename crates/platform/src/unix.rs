//! macOS and Linux. Functions taking an `hwnd` belong to Windows' windows: the app only has one on
//! Windows, so here they do nothing.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Once, OnceLock};

use crate::TitleBarDoubleClick;

/// `~/Library/Application Support` on macOS; `$XDG_CONFIG_HOME`, or `~/.config`, elsewhere.
pub fn config_dir() -> Option<PathBuf> {
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home().map(|home| home.join("Library").join("Application Support"));
    }
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|dir| dir.is_absolute());
    xdg.or_else(|| home().map(|home| home.join(".config")))
}

/// Whether this copy is the one a package put in place (an `.app` bundle, the deb's `/usr/bin` or
/// an AppImage), which updates itself, rather than a build from source.
pub fn is_installed() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    if cfg!(target_os = "macos") {
        return exe.ancestors().any(|p| p.extension().is_some_and(|e| e == "app"));
    }
    std::env::var_os("APPIMAGE").is_some() || exe == Path::new("/usr/bin/splicr")
}

/// Opens `target` (a web address, a file or a folder) with the system's choice of app.
pub fn open(target: &str) {
    spawn(if cfg!(target_os = "macos") { "open" } else { "xdg-open" }, &[target]);
}

/// Shows `path` in the file manager: selected in the Finder on macOS, its folder elsewhere (no
/// standard way to select a file there).
pub fn reveal(path: &Path) {
    let path = path.to_string_lossy();
    if cfg!(target_os = "macos") {
        spawn("open", &["-R", &path]);
    } else {
        let folder = Path::new(&*path).parent().map_or(path.to_string(), |p| p.to_string_lossy().into_owned());
        spawn("xdg-open", &[&folder]);
    }
}

fn spawn(program: &str, args: &[&str]) {
    if let Ok(mut child) = Command::new(program).args(args).spawn() {
        // Reaped on a thread, so it doesn't linger as a zombie process.
        let _ = std::thread::Builder::new().name("splicr-open".into()).spawn(move || child.wait());
    }
}

/// The languages of the locale variables, most wanted first, as tags like `es-CO`. On macOS these
/// are set in a terminal but not for apps opened from the Finder.
pub(crate) fn locale_variables() -> Vec<String> {
    let vars = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"];
    language_tags(vars.iter().filter_map(|var| std::env::var(var).ok()))
}

/// Tags from locale values: `LANGUAGE` lists several (`es_CO:es`), the others name one
/// (`es_CO.UTF-8`). Encodings and modifiers are dropped; `C` and `POSIX` aren't languages.
fn language_tags(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for value in values {
        for locale in value.split(':') {
            let name = locale.split(['.', '@']).next().unwrap_or_default();
            if name.is_empty() || name == "C" || name == "POSIX" {
                continue;
            }
            let tag = name.replace('_', "-");
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    tags
}

/// Windows' title bar colours.
pub fn tint_title_bar(_hwnd: isize, _caption: [u8; 3], _text: [u8; 3], _border: [u8; 3]) {}

pub fn keep_title_bar_theme(_dark: Option<bool>) {}

pub fn repaint_title_bar_when_shown(_hwnd: isize) {}

pub fn use_exe_icon(_hwnd: isize, _id: u16) {}

/// macOS: the user's setting, read once on a background thread (the first call starts it and zooms
/// until it is known). Elsewhere a zoom.
pub fn title_bar_double_click() -> TitleBarDoubleClick {
    static ACTION: OnceLock<TitleBarDoubleClick> = OnceLock::new();
    static READ: Once = Once::new();
    if cfg!(target_os = "macos") {
        READ.call_once(|| {
            let _ = std::thread::Builder::new().name("splicr-title-bar-setting".into()).spawn(|| {
                let out = Command::new("/usr/bin/defaults").args(["read", "-g", "AppleActionOnDoubleClick"]).output();
                // Absent (never changed) or unreadable: macOS's default, zoom.
                let action = match out {
                    Ok(out) if out.status.success() => double_click_action(&String::from_utf8_lossy(&out.stdout)),
                    _ => TitleBarDoubleClick::Zoom,
                };
                let _ = ACTION.set(action);
            });
        });
    }
    ACTION.get().copied().unwrap_or(TitleBarDoubleClick::Zoom)
}

/// The global default `AppleActionOnDoubleClick` as `defaults read` prints it. Anything but
/// "Minimize" or "None" zooms: macOS's default "Maximize", and macOS 15's "Fill", which zooming
/// comes closest to. (After ArtCraft's craft-window, Apache-2.0, by way of Remapr.)
fn double_click_action(defaults_output: &str) -> TitleBarDoubleClick {
    match defaults_output.trim() {
        "Minimize" => TitleBarDoubleClick::Minimize,
        "None" => TitleBarDoubleClick::Nothing,
        _ => TitleBarDoubleClick::Zoom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_values_become_language_tags() {
        let values = ["es_CO:es", "", "en_US.UTF-8", "C.UTF-8", "de_DE@euro", "es"];
        let tags = language_tags(values.iter().map(|v| v.to_string()));
        assert_eq!(tags, ["es-CO", "es", "en-US", "de-DE"]);
    }

    #[test]
    fn reads_the_double_click_setting() {
        assert_eq!(double_click_action("Maximize\n"), TitleBarDoubleClick::Zoom);
        assert_eq!(double_click_action("Minimize\n"), TitleBarDoubleClick::Minimize);
        assert_eq!(double_click_action("None\n"), TitleBarDoubleClick::Nothing);
        for junk in ["", "Fill", "minimize", "1"] {
            assert_eq!(double_click_action(junk), TitleBarDoubleClick::Zoom, "{junk:?}");
        }
    }
}
