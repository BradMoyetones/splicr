//! `cargo xtask <task>`: repository chores that are not part of the app.
//!
//! Remapr's, for Splicr's packages.
//!
//! - `icons`: renders `assets/logo-light.svg` into the app's icons. Run it after changing the logo
//!   and commit the results; the builds only embed or copy them.
//! - `dist`: builds the release package and its SHA-256 beside it, in `target/dist`. On Windows the
//!   installer, `Splicr-<version>-x64.msi`: fetches WiX on first use and needs the .NET runtime (6 or
//!   newer). On macOS `Splicr-<version>-macos-universal.dmg` ([`macos`]); on Linux
//!   `Splicr-<version>-linux-x86_64.deb` and `.AppImage` ([`linux`]).
//! - `layers`: fails when a crate depends on one it shouldn't (see [`layers`]). CI runs it.
//! - `keygen <folder>` and `sign <files…>`: the packages' signatures (see [`signing`]).

mod icns;
mod layers;
mod linux;
mod macos;
mod signing;

use std::path::{Path, PathBuf};
use std::process::Command;

use resvg::{tiny_skia, usvg};
use sha2::{Digest, Sha256};

/// Every size Windows asks for, from the 100% title-bar icon to the 256 px Explorer tile.
const ICO_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

/// A pinned NuGet package and the file in it that `dist` needs.
struct Package {
    id: &'static str,
    sha256: &'static str,
    file: &'static str,
}

/// WiX 5 (as photocraft uses) and its setup-wizard extension.
const WIX_VERSION: &str = "5.0.2";
const WIX: Package = Package {
    id: "wix",
    sha256: "f30ef0c74e2a986126539c5780be93ac24e8136eaf723b1937b26272703ae173",
    file: "tools/net6.0/any/wix.dll",
};
const WIX_UI: Package = Package {
    id: "wixtoolset.ui.wixext",
    sha256: "5ef2c707614b9f70b6bbadd2d4abcb4124efee215e9b16bfbc80113079a604c7",
    file: "wixext5/WixToolset.UI.wixext.dll",
};

/// The brand colour behind the logo on the installer's Welcome and Finish pages (the app's accent).
const ACCENT: [u8; 3] = [0x4F, 0x46, 0xE5];

fn main() -> Result<(), String> {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons(),
        Some("dist") => dist(),
        Some("layers") => layers::run(&root()),
        Some("keygen") => match std::env::args().nth(2) {
            Some(folder) => signing::keygen(Path::new(&folder)),
            None => Err("usage: cargo xtask keygen <folder outside the repository>".into()),
        },
        Some("sign") => signing::sign(&std::env::args().skip(2).collect::<Vec<_>>()),
        _ => Err("usage: cargo xtask <icons|dist|layers|keygen|sign>".into()),
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the workspace").to_path_buf()
}

fn dist() -> Result<(), String> {
    if cfg!(target_os = "macos") {
        return macos::dist(&root());
    }
    if cfg!(target_os = "linux") {
        return linux::dist(&root());
    }
    if !cfg!(windows) {
        return Err("dist builds the packages of Windows, macOS and Linux: run it on one of them".into());
    }
    let root = root();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    run(Command::new(cargo).args(["build", "--release", "--locked", "-p", "splicr"]).current_dir(&root))?;

    let wix_dir = root.join("target").join("wix");
    let wix = fetch(&wix_dir, &WIX)?;
    let wix_ui = fetch(&wix_dir, &WIX_UI)?;
    let art = wix_dir.join("art");
    installer_art(&root, &art)?;

    let version = env!("CARGO_PKG_VERSION");
    // MSI versions are numeric: a pre-release tag is dropped there.
    let msi_version = version.split('-').next().unwrap_or(version);
    let dist = root.join("target").join("dist");
    std::fs::create_dir_all(&dist).map_err(|e| e.to_string())?;
    let msi = dist.join(format!("Splicr-{version}-x64.msi"));
    let define = |name: &str, path: PathBuf| ["-d".to_owned(), format!("{name}={}", path.display())];
    run(Command::new("dotnet")
        .arg(wix)
        .arg("build")
        .arg(root.join("packaging/windows/splicr.wxs"))
        .args(["-arch", "x64", "-pdbtype", "none", "-culture", "es-ES", "-ext"])
        .arg(wix_ui)
        .args(["-d", &format!("Version={msi_version}")])
        .args(define("ExePath", root.join("target/release/splicr.exe")))
        .args(define("IconPath", root.join("assets/icon.ico")))
        .args(define("BannerBmp", art.join("banner.bmp")))
        .args(define("DialogBmp", art.join("dialog.bmp")))
        .args(define("LicenseRtf", art.join("license.rtf")))
        .arg("-o")
        .arg(&msi)
        // WiX targets .NET 6; any newer runtime will do.
        .env("DOTNET_ROLL_FORWARD", "Major"))?;

    write_sha256(&msi)?;
    println!("wrote {}", msi.display());
    Ok(())
}

/// `<file>.sha256` beside `file`, in the format `sha256sum -c` reads: what the updater checks a
/// download against.
fn write_sha256(file: &Path) -> Result<(), String> {
    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    std::fs::write(file.with_file_name(format!("{name}.sha256")), format!("{}  {name}\n", sha256(&bytes)))
        .map_err(|e| e.to_string())
}

/// Downloads and unpacks `package` into `dir` unless it is already there; returns its `file`.
fn fetch(dir: &Path, package: &Package) -> Result<PathBuf, String> {
    let Package { id, sha256: expected, file } = package;
    let unpacked = dir.join(format!("{id}.{WIX_VERSION}"));
    let wanted = unpacked.join(file);
    // Check the file, not the folder: CI's cache restores `target` without files cargo didn't make.
    if wanted.exists() {
        return Ok(wanted);
    }
    let system32 =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into())).join("System32");
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let nupkg = dir.join(format!("{id}.{WIX_VERSION}.nupkg"));
    let url = format!("https://api.nuget.org/v3-flatcontainer/{id}/{WIX_VERSION}/{id}.{WIX_VERSION}.nupkg");
    println!("fetching {url}");
    // curl.exe and tar.exe (which reads zips) ship with Windows 10 and later.
    run(Command::new(system32.join("curl.exe")).args(["-sSfL", "-o"]).arg(&nupkg).arg(&url))?;
    let actual = sha256(&std::fs::read(&nupkg).map_err(|e| e.to_string())?);
    if actual != *expected {
        let _ = std::fs::remove_file(&nupkg);
        return Err(format!("{id} {WIX_VERSION}: SHA-256 is {actual}, expected {expected}"));
    }
    // Unpack beside, then rename: an interrupted run never leaves a half-unpacked folder behind.
    let partial = dir.join(format!("{id}.{WIX_VERSION}.partial"));
    let _ = std::fs::remove_dir_all(&partial);
    let _ = std::fs::remove_dir_all(&unpacked);
    std::fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
    run(Command::new(system32.join("tar.exe")).arg("-xf").arg(&nupkg).arg("-C").arg(&partial))?;
    std::fs::rename(&partial, &unpacked).map_err(|e| e.to_string())?;
    Ok(wanted)
}

/// The setup wizard's art, drawn from the logo so the MSI shows Splicr rather than WiX's stock
/// bitmaps: a 493x58 banner across the top of the inner pages (WiX writes the page title over its
/// left side, so that stays white) and a 493x312 backdrop for the Welcome and Finish pages (their
/// text sits right of x=164). Both must be 24-bit BMPs. Plus the licence as the RTF its page shows.
fn installer_art(root: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let svg = std::fs::read(root.join("assets/logo-light.svg")).map_err(|e| e.to_string())?;
    let logo = usvg::Tree::from_data(&svg, &usvg::Options::default()).map_err(|e| e.to_string())?;

    let mut banner = tiny_skia::Pixmap::new(493, 58).expect("non-zero size");
    banner.fill(tiny_skia::Color::WHITE);
    draw(&logo, &mut banner, 493.0 - 50.0 - 8.0, 4.0, 50.0);
    write_bmp(&banner, &out.join("banner.bmp"))?;

    let mut dialog = tiny_skia::Pixmap::new(493, 312).expect("non-zero size");
    dialog.fill(tiny_skia::Color::WHITE);
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(ACCENT[0], ACCENT[1], ACCENT[2], 255);
    let panel = tiny_skia::Rect::from_xywh(0.0, 0.0, 164.0, 312.0).expect("valid rect");
    dialog.fill_rect(panel, &paint, tiny_skia::Transform::identity(), None);
    draw(&logo, &mut dialog, (164.0 - 112.0) / 2.0, 72.0, 112.0);
    write_bmp(&dialog, &out.join("dialog.bmp"))?;

    let license = std::fs::read_to_string(root.join("LICENSE")).map_err(|e| e.to_string())?;
    std::fs::write(out.join("license.rtf"), license_rtf(&license)).map_err(|e| e.to_string())
}

/// Draws the square `logo` at (`x`, `y`), `size` pixels wide.
fn draw(logo: &usvg::Tree, pixmap: &mut tiny_skia::Pixmap, x: f32, y: f32, size: f32) {
    let scale = size / logo.size().width();
    let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(x, y);
    resvg::render(logo, transform, &mut pixmap.as_mut());
}

/// A 24-bit, bottom-up BMP. `pixmap` must be opaque (its colours are then not premultiplied).
fn write_bmp(pixmap: &tiny_skia::Pixmap, path: &Path) -> Result<(), String> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let row = (width * 3).next_multiple_of(4);
    let mut bytes = Vec::with_capacity(54 + (row * height) as usize);
    bytes.extend(b"BM");
    bytes.extend((54 + row * height).to_le_bytes());
    bytes.extend([0; 4]);
    bytes.extend(54u32.to_le_bytes());
    // BITMAPINFOHEADER: size, width, height, 1 plane, 24 bits, then no compression and defaults.
    bytes.extend(40u32.to_le_bytes());
    bytes.extend(width.to_le_bytes());
    bytes.extend(height.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(24u16.to_le_bytes());
    bytes.extend([0; 24]);
    for y in (0..height).rev() {
        for x in 0..width {
            let p = pixmap.pixel(x, y).expect("in bounds");
            bytes.extend([p.blue(), p.green(), p.red()]);
        }
        bytes.resize(bytes.len() + (row - width * 3) as usize, 0);
    }
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// `text` as RTF: one paragraph per blank-line-separated block (the file wraps lines at 80 columns).
fn license_rtf(text: &str) -> String {
    let paragraphs: Vec<String> = text
        .replace("\r\n", "\n")
        .split("\n\n")
        .map(|block| block.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|block| !block.is_empty())
        .map(|block| {
            block
                .chars()
                .map(|c| match c {
                    '\\' | '{' | '}' => format!("\\{c}"),
                    c if c.is_ascii() => c.to_string(),
                    c => format!("\\u{}?", c as u32 as i16),
                })
                .collect()
        })
        .collect();
    format!("{{\\rtf1\\ansi\\deff0{{\\fonttbl{{\\f0 Segoe UI;}}}}\\f0\\fs18 {}\\par}}", paragraphs.join("\\par\\par "))
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|e| format!("{:?}: {e}", command.get_program()))?;
    if status.success() { Ok(()) } else { Err(format!("{:?} failed: {status}", command.get_program())) }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn icons() -> Result<(), String> {
    let dir = root().join("assets");
    let svg = std::fs::read(dir.join("logo-light.svg")).map_err(|e| e.to_string())?;
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).map_err(|e| e.to_string())?;

    write_ico(&tree, dir.join("icon.ico"))?;
    let png = render(&tree, 256).encode_png().map_err(|e| e.to_string())?;
    std::fs::write(dir.join("icon-256.png"), png).map_err(|e| e.to_string())?;
    let pngs = icns::SIZES.map(|size| render_on_apple_grid(&tree, size).encode_png().map(|png| (size, png)));
    let pngs = pngs.into_iter().collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    std::fs::write(dir.join("icon.icns"), icns::pack(&pngs)?).map_err(|e| e.to_string())?;
    println!("wrote icon.ico, icon-256.png and icon.icns to {}", dir.display());
    Ok(())
}

/// macOS app icons leave a margin: the shape fills 824 of 1024 px (Apple's icon grid), centred. As
/// ArtCraft's `packaging/icons.sh` does, the logo is drawn unchanged inside a wider view.
fn render_on_apple_grid(tree: &usvg::Tree, size: u32) -> tiny_skia::Pixmap {
    let shape = tree.root().abs_stroke_bounding_box();
    let view = shape.width().max(shape.height()) * 1024.0 / 824.0;
    let (x, y) = (shape.x() + shape.width() / 2.0 - view / 2.0, shape.y() + shape.height() / 2.0 - view / 2.0);
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("icon sizes are non-zero");
    let scale = size as f32 / view;
    let transform = tiny_skia::Transform::from_scale(scale, scale).pre_translate(-x, -y);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    pixmap
}

/// Each size is rendered from the vector, not scaled down from a bitmap, so 16 px stays sharp.
fn write_ico(tree: &usvg::Tree, path: PathBuf) -> Result<(), String> {
    let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
    for size in ICO_SIZES {
        let image = ico::IconImage::from_rgba_data(size, size, rgba(&render(tree, size)));
        // Windows' convention: 32-bit BMP below 256 px (every reader understands it), PNG at 256.
        let entry =
            if size < 256 { ico::IconDirEntry::encode_as_bmp(&image) } else { ico::IconDirEntry::encode(&image) };
        icon.add_entry(entry.map_err(|e| e.to_string())?);
    }
    let file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    icon.write(file).map_err(|e| e.to_string())
}

fn render(tree: &usvg::Tree, size: u32) -> tiny_skia::Pixmap {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("icon sizes are non-zero");
    let scale = size as f32 / tree.size().width();
    resvg::render(tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap
}

/// Straight (not premultiplied) RGBA, as ICO wants it.
fn rgba(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn licence_becomes_rtf_paragraphs() {
        let rtf = license_rtf("MIT {x}\nwrapped line\n\nC:\\path");
        assert!(rtf.starts_with("{\\rtf1"));
        assert!(rtf.contains("MIT \\{x\\} wrapped line\\par\\par C:\\\\path"));
    }

    #[test]
    fn bmp_rows_are_padded_and_bottom_up() {
        let dir = std::env::temp_dir().join(format!("splicr-xtask-bmp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut pixmap = tiny_skia::Pixmap::new(3, 2).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        let path = dir.join("t.bmp");
        write_bmp(&pixmap, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        // 3 px * 3 bytes = 9, padded to 12 per row.
        assert_eq!(bytes.len(), 54 + 12 * 2);
        assert_eq!(&bytes[54..57], &[30, 20, 10]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
