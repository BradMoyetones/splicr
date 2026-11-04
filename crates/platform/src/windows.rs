//! Windows: the settings folder, opening things through Explorer, and the native window's icon and
//! title bar (`window`, the only `unsafe` in Splicr).

use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(unsafe_code)]
mod window;

pub use window::{keep_title_bar_theme, repaint_title_bar_when_shown, tint_title_bar, use_exe_icon};

/// `%APPDATA%`: settings there roam with the user's account.
pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// Where the installer puts Splicr: per user, so installing and updating need no administrator.
fn install_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Programs").join("Splicr"))
}

/// Whether this copy is the one the installer put in place, which updates itself, rather than a
/// build from source.
pub fn is_installed() -> bool {
    let (Ok(exe), Some(dir)) = (std::env::current_exe(), install_dir()) else { return false };
    let folder = |p: &Path| p.to_string_lossy().trim_end_matches('\\').to_lowercase();
    exe.parent().is_some_and(|parent| folder(parent) == folder(&dir))
}

fn explorer() -> Command {
    let windows = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
    Command::new(windows.join("explorer.exe"))
}

/// Opens `target` (a web address, a file or a folder) with the system's choice of app.
pub fn open(target: &str) {
    let _ = explorer().arg(target).spawn();
}

/// Shows `path` selected in Explorer.
pub fn reveal(path: &Path) {
    use std::os::windows::process::CommandExt;
    // Explorer reads its own command line: `/select,"C:\a b\c.xlsx"` must reach it as one piece.
    let _ = explorer().raw_arg(format!("/select,\"{}\"", path.display())).spawn();
}

/// Windows has no locale variables: its languages come from `sys-locale` alone.
pub(crate) fn locale_variables() -> Vec<String> {
    Vec::new()
}

/// Windows' title bars zoom on a double-click, and draw their own.
pub fn title_bar_double_click() -> crate::TitleBarDoubleClick {
    crate::TitleBarDoubleClick::Zoom
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
