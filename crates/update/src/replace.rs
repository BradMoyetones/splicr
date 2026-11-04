//! Putting a downloaded, verified package in place of the running Splicr, by system.
//!
//! The old copy is only moved aside once the new one sits complete beside it, in the same folder,
//! and comes back if the swap fails: an update can fail, but never leaves Splicr half-installed.
//! The running process keeps its files open; the new copy starts once it has quit.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Error;

/// How this copy of Splicr updates itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Installer {
    /// Windows: the per-user MSI, which ends Splicr, installs over it and starts it again.
    Msi,
    /// macOS: the `.app` bundle Splicr runs from, replaced by the one in the DMG.
    App(PathBuf),
    /// Linux: the AppImage Splicr runs from, replaced by the new one.
    AppImage(PathBuf),
}

/// How this copy updates itself, or `None` when it can't: a deb (it needs root), an app run from
/// its DMG or from a folder Splicr can't write to, or one macOS runs from a read-only copy
/// (App Translocation, for apps opened where they were downloaded). The app then offers the
/// release page instead.
pub fn installer() -> Option<Installer> {
    if cfg!(windows) {
        return Some(Installer::Msi);
    }
    if cfg!(target_os = "macos") {
        let exe = std::env::current_exe().ok()?;
        let app = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app"))?.to_path_buf();
        let replaceable = !translocated(&app) && writable(app.parent()?);
        return replaceable.then_some(Installer::App(app));
    }
    if cfg!(target_os = "linux") {
        let image = PathBuf::from(std::env::var_os("APPIMAGE")?);
        return writable(image.parent()?).then_some(Installer::AppImage(image));
    }
    None
}

/// Puts `package` (already verified) in place and arranges for the new Splicr to start once this
/// one quits; the caller quits next.
pub fn install(installer: &Installer, package: &Path) -> Result<(), Error> {
    match installer {
        Installer::Msi => msi(package),
        Installer::App(app) => {
            replace_app(package, app)?;
            relaunch(app)
        }
        Installer::AppImage(image) => {
            replace_file(package, image)?;
            relaunch(image)
        }
    }
}

/// Starts Windows Installer on `msi` with a progress bar and no questions, logging beside the file.
/// It ends Splicr itself, and starts the new one when done.
fn msi(msi: &Path) -> Result<(), Error> {
    let windows = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()));
    Command::new(windows.join("System32").join("msiexec.exe"))
        .arg("/i")
        .arg(msi)
        .args(["/passive", "/norestart", "/l*v"])
        .arg(msi.with_extension("log"))
        .spawn()
        .map(drop)
        .map_err(|_| Error::Installer(None))
}

/// macOS runs apps opened where they were downloaded from a random, read-only path.
fn translocated(app: &Path) -> bool {
    app.to_string_lossy().contains("/AppTranslocation/")
}

/// Whether Splicr can create files in `dir`: the new copy is staged there, so the swap is a rename.
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".splicr-update-probe-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

fn replace_error(e: impl std::fmt::Display) -> Error {
    Error::Replace(e.to_string())
}

/// `.Splicr.app.new` and `.Splicr.app.old` beside `target`: hidden, on the same volume.
fn beside(target: &Path, suffix: &str) -> Result<PathBuf, Error> {
    let name = target.file_name().ok_or_else(|| replace_error("no file name"))?.to_string_lossy();
    Ok(target.with_file_name(format!(".{name}.{suffix}")))
}

fn remove(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
    let _ = std::fs::remove_file(path);
}

/// Moves `target` aside, `staged` into its place, then drops the old one; puts `target` back if the
/// second move fails.
fn swap(target: &Path, staged: &Path) -> Result<(), Error> {
    let old = beside(target, "old")?;
    remove(&old);
    std::fs::rename(target, &old).map_err(replace_error)?;
    if let Err(e) = std::fs::rename(staged, target) {
        let _ = std::fs::rename(&old, target);
        remove(staged);
        return Err(replace_error(e));
    }
    remove(&old);
    Ok(())
}

/// An AppImage (or any single file): the new one is copied beside the old, made executable, and
/// swapped in.
fn replace_file(new: &Path, target: &Path) -> Result<(), Error> {
    let staged = beside(target, "new")?;
    remove(&staged);
    std::fs::copy(new, &staged).map_err(replace_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).map_err(replace_error)?;
    }
    swap(target, &staged)
}

/// The `.app` in `dmg` copied beside `app` and swapped in. The DMG is mounted out of sight
/// (`-nobrowse`) and read-only, and the bundle copied with `ditto`, which keeps its signature and
/// attributes intact.
fn replace_app(dmg: &Path, app: &Path) -> Result<(), Error> {
    let mount = std::env::temp_dir().join(format!("splicr-update-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&mount);
    let hdiutil = "/usr/bin/hdiutil";
    let attach = ["attach", "-nobrowse", "-readonly", "-noautoopen", "-noverify", "-mountpoint"];
    run(Command::new(hdiutil).args(attach).arg(&mount).arg(dmg))?;
    let staged = beside(app, "new")?;
    let copied = (|| {
        let entries = std::fs::read_dir(&mount).map_err(replace_error)?;
        let source = entries
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .ok_or_else(|| replace_error("the DMG holds no app"))?;
        remove(&staged);
        run(Command::new("/usr/bin/ditto").arg(&source).arg(&staged))
    })();
    let _ = run(Command::new(hdiutil).arg("detach").arg(&mount).arg("-force"));
    let _ = std::fs::remove_dir(&mount);
    if let Err(e) = copied {
        remove(&staged);
        return Err(e);
    }
    // Nothing a browser downloaded, but a quarantine flag would make macOS ask again.
    let _ = run(Command::new("/usr/bin/xattr").arg("-cr").arg(&staged));
    swap(app, &staged)
}

/// Starts `target` once this process has quit: a shell waits for it, then opens the new copy.
fn relaunch(target: &Path) -> Result<(), Error> {
    let open = if cfg!(target_os = "macos") { r#"exec /usr/bin/open -n "$2""# } else { r#"exec "$2""# };
    let script = format!(r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; {open}"#);
    Command::new("/bin/sh")
        .args(["-c", &script, "splicr-relaunch", &std::process::id().to_string()])
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(replace_error)
}

fn run(command: &mut Command) -> Result<(), Error> {
    let out = command.stdin(Stdio::null()).output().map_err(replace_error)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::Replace(String::from_utf8_lossy(&out.stderr).trim().to_owned()))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("splicr-replace-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_appimage_is_swapped_and_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp("appimage");
        let (old, new) = (dir.join("Splicr.AppImage"), dir.join("download"));
        std::fs::write(&old, "old").unwrap();
        std::fs::write(&new, "new").unwrap();
        replace_file(&new, &old).unwrap();
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "new");
        assert_eq!(std::fs::metadata(&old).unwrap().permissions().mode() & 0o777, 0o755);
        // Nothing left beside it.
        let names: Vec<String> =
            std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_swap_keeps_the_old_copy() {
        let dir = temp("swap");
        let target = dir.join("Splicr.app");
        std::fs::create_dir_all(target.join("Contents")).unwrap();
        // Nothing staged: the second move fails, the old copy comes back.
        assert!(swap(&target, &dir.join(".Splicr.app.new")).is_err());
        assert!(target.join("Contents").is_dir());
        assert!(!dir.join(".Splicr.app.old").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn translocated_apps_are_spotted() {
        assert!(translocated(Path::new("/private/var/folders/x/T/AppTranslocation/ABC/d/Splicr.app")));
        assert!(!translocated(Path::new("/Applications/Splicr.app")));
        assert!(writable(&std::env::temp_dir()));
        assert!(!writable(Path::new("/nonexistent-splicr-dir")));
    }

    /// The whole macOS path with a real DMG: `cargo test -p splicr-update -- --ignored`. Ignored by
    /// default: hdiutil is slow and now and then busy on CI runners.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore]
    fn an_app_is_replaced_from_a_dmg() {
        let dir = temp("dmg");
        let source = dir.join("src");
        std::fs::create_dir_all(source.join("Splicr.app/Contents")).unwrap();
        std::fs::write(source.join("Splicr.app/Contents/version"), "new").unwrap();
        let dmg = dir.join("Splicr.dmg");
        run(Command::new("/usr/bin/hdiutil")
            .args(["create", "-quiet", "-srcfolder"])
            .arg(&source)
            .arg("-ov")
            .arg(&dmg))
        .unwrap();
        let installed = dir.join("Applications/Splicr.app");
        std::fs::create_dir_all(installed.join("Contents")).unwrap();
        std::fs::write(installed.join("Contents/version"), "old").unwrap();
        replace_app(&dmg, &installed).unwrap();
        assert_eq!(std::fs::read_to_string(installed.join("Contents/version")).unwrap(), "new");
        assert!(!dir.join("Applications/.Splicr.app.old").exists());
        assert!(!dir.join("Applications/.Splicr.app.new").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
