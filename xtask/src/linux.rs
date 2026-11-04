//! `cargo xtask dist` on Linux: `target/dist/Splicr-<version>-linux-x86_64.deb` and `.AppImage`,
//! each with its `.sha256`.
//!
//! The steps of ArtCraft's craft-launcher `packaging/linux/package.sh`: one file tree for both, the
//! deb made by nfpm, the AppImage by appimagetool with the static type 2 runtime (so users need no
//! libfuse). The tools are fetched into `target/linux-tools` on first use, pinned by SHA-256.

use std::path::{Path, PathBuf};
use std::process::Command;

use resvg::usvg;

use crate::{render, run, sha256, write_sha256};

const APP_ID: &str = "io.github.bradmoyetones.splicr";
/// The glibc of Ubuntu 22.04, where releases are built: the oldest the deb asks for.
const GLIBC_FLOOR: &str = "2.35";
/// The sizes of the hicolor icon theme the package fills.
const ICON_SIZES: [u32; 8] = [16, 24, 32, 48, 64, 128, 256, 512];

/// A pinned download: name, URL and SHA-256.
type Tool = (&'static str, &'static str, &'static str);
const NFPM: Tool = (
    "nfpm-2.48.0.tar.gz",
    "https://github.com/goreleaser/nfpm/releases/download/v2.48.0/nfpm_2.48.0_Linux_x86_64.tar.gz",
    "6d7f9a28b2f397934403950fcbba83f598b08d02edbe79a0815b3b4897341750",
);
/// The versions and hashes ArtCraft pins.
const APPIMAGETOOL: Tool = (
    "appimagetool-1.9.1.AppImage",
    "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage",
    "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0",
);
const RUNTIME: Tool = (
    "appimage-runtime-20251108",
    "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64",
    "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d",
);

pub fn dist(root: &Path) -> Result<(), String> {
    // shortcut: x86_64 only, the arch every tool here is pinned for; add aarch64's hashes (ArtCraft
    // has them) and an ubuntu-22.04-arm job when someone asks.
    if std::env::consts::ARCH != "x86_64" {
        return Err("dist builds the Linux packages on x86_64 only, for now".into());
    }
    let io = |e: std::io::Error| e.to_string();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    run(Command::new(cargo).args(["build", "--release", "--locked", "-p", "splicr"]).current_dir(root))?;

    let here = root.join("packaging/linux");
    let work = root.join("target/linux");
    let _ = std::fs::remove_dir_all(&work);
    let stage = work.join("stage");
    let share = stage.join("usr/share");
    for dir in ["usr/bin", "usr/share/applications", "usr/share/metainfo", "usr/share/doc/splicr"] {
        std::fs::create_dir_all(stage.join(dir)).map_err(io)?;
    }
    std::fs::copy(root.join("target/release/splicr"), stage.join("usr/bin/splicr")).map_err(io)?;
    let _ = Command::new("strip").arg(stage.join("usr/bin/splicr")).status();
    let desktop = format!("{APP_ID}.desktop");
    std::fs::copy(here.join(&desktop), share.join("applications").join(&desktop)).map_err(io)?;
    let metainfo = std::fs::read_to_string(here.join(format!("{APP_ID}.metainfo.xml.in"))).map_err(io)?;
    let metainfo = metainfo.replace("@VERSION@", env!("CARGO_PKG_VERSION")).replace("@DATE@", &today()?);
    std::fs::write(share.join("metainfo").join(format!("{APP_ID}.metainfo.xml")), metainfo).map_err(io)?;
    icons(root, &share.join("icons/hicolor"))?;
    for doc in ["README.md", "LICENSE", "ATTRIBUTION.md"] {
        std::fs::copy(root.join(doc), share.join("doc/splicr").join(doc)).map_err(io)?;
    }

    let dist = root.join("target/dist");
    std::fs::create_dir_all(&dist).map_err(io)?;
    let base = format!("Splicr-{}-linux-x86_64", env!("CARGO_PKG_VERSION"));
    let tools = root.join("target/linux-tools");

    // nfpm reads `${STAGE}` in `contents` only once it is written out.
    let nfpm_tar = download(&tools, NFPM)?;
    run(Command::new("tar").arg("-xzf").arg(&nfpm_tar).arg("-C").arg(&tools).arg("nfpm"))?;
    let stage_text = stage.to_str().ok_or("the work folder's path isn't UTF-8")?;
    let config = std::fs::read_to_string(here.join("nfpm.yaml")).map_err(io)?.replace("${STAGE}", stage_text);
    std::fs::write(work.join("nfpm.yaml"), config).map_err(io)?;
    let deb = dist.join(format!("{base}.deb"));
    let mut nfpm = Command::new(tools.join("nfpm"));
    nfpm.args(["package", "-p", "deb", "-f"]).arg(work.join("nfpm.yaml")).arg("-t").arg(&deb);
    run(nfpm.env("VERSION", env!("CARGO_PKG_VERSION")).env("GLIBC_FLOOR", GLIBC_FLOOR).current_dir(root))?;
    write_sha256(&deb)?;
    println!("wrote {}", deb.display());

    let app_dir = work.join("Splicr.AppDir");
    run(Command::new("cp").arg("-a").arg(&stage).arg(&app_dir))?;
    run(Command::new("install").arg("-m755").arg(here.join("AppRun")).arg(app_dir.join("AppRun")))?;
    std::fs::copy(here.join(&desktop), app_dir.join(&desktop)).map_err(io)?;
    let icon = format!("{APP_ID}.png");
    std::fs::copy(share.join("icons/hicolor/256x256/apps").join(&icon), app_dir.join(&icon)).map_err(io)?;
    run(Command::new("ln").arg("-s").arg(&icon).arg(app_dir.join(".DirIcon")))?;
    let (tool, runtime) = (download(&tools, APPIMAGETOOL)?, download(&tools, RUNTIME)?);
    run(Command::new("chmod").arg("755").arg(&tool))?;
    let appimage = dist.join(format!("{base}.AppImage"));
    let mut make = Command::new(&tool);
    make.args(["--no-appstream", "--runtime-file"]).arg(&runtime).arg(&app_dir).arg(&appimage);
    // Extract-and-run: appimagetool itself needs no FUSE either (containers, CI).
    run(make.env("ARCH", "x86_64").env("APPIMAGE_EXTRACT_AND_RUN", "1"))?;
    write_sha256(&appimage)?;
    println!("wrote {}", appimage.display());
    Ok(())
}

/// The logo in every size of the hicolor theme, drawn from the vector, and the vector itself.
fn icons(root: &Path, hicolor: &Path) -> Result<(), String> {
    let svg = std::fs::read(root.join("assets/logo-light.svg")).map_err(|e| e.to_string())?;
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).map_err(|e| e.to_string())?;
    for size in ICON_SIZES {
        let dir = hicolor.join(format!("{size}x{size}/apps"));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        render(&tree, size).save_png(dir.join(format!("{APP_ID}.png"))).map_err(|e| e.to_string())?;
    }
    let scalable = hicolor.join("scalable/apps");
    std::fs::create_dir_all(&scalable).map_err(|e| e.to_string())?;
    std::fs::write(scalable.join(format!("{APP_ID}.svg")), svg).map_err(|e| e.to_string())
}

/// The UTC date, `YYYY-MM-DD`, for the metainfo's release.
fn today() -> Result<String, String> {
    let out = Command::new("date").args(["-u", "+%Y-%m-%d"]).output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// `tool` in `dir`, downloaded unless it is already there, checked against its SHA-256.
fn download(dir: &Path, (name, url, expected): Tool) -> Result<PathBuf, String> {
    let path = dir.join(name);
    let matches = |path: &Path| std::fs::read(path).is_ok_and(|bytes| sha256(&bytes) == expected);
    if matches(&path) {
        return Ok(path);
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let partial = path.with_extension("download");
    println!("fetching {url}");
    run(Command::new("curl").args(["--proto", "=https", "-sSfL", "-o"]).arg(&partial).arg(url))?;
    if !matches(&partial) {
        let _ = std::fs::remove_file(&partial);
        return Err(format!("{name}: its SHA-256 isn't the pinned {expected}"));
    }
    std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;
    Ok(path)
}
