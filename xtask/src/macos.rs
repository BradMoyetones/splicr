//! `cargo xtask dist` on macOS: `target/dist/Splicr-<version>-macos-universal.dmg`, holding
//! `Splicr.app` (one binary for Apple silicon and Intel) beside a link to Applications.
//!
//! The steps are those of ArtCraft's `craft-launcher:packaging/macos/package.sh`, with Apple's own
//! tools (lipo, codesign, hdiutil). Its notarization is left out until there is a Developer ID.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::{run, write_sha256};

const TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];
/// Keep in step with `LSMinimumSystemVersion` in `packaging/macos/Info.plist.in`.
const MINIMUM_MACOS: &str = "11.0";
const INFO_PLIST: &str = include_str!("../../packaging/macos/Info.plist.in");

pub fn dist(root: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| e.to_string();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut build = Command::new(cargo);
    build.args(["build", "--release", "--locked", "-p", "splicr"]).env("MACOSX_DEPLOYMENT_TARGET", MINIMUM_MACOS);
    for target in TARGETS {
        build.args(["--target", target]);
    }
    run(build.current_dir(root))?;

    let work = root.join("target/macos");
    let _ = std::fs::remove_dir_all(&work);
    let app = work.join("Splicr.app");
    let contents = app.join("Contents");
    std::fs::create_dir_all(contents.join("MacOS")).map_err(io)?;
    std::fs::create_dir_all(contents.join("Resources")).map_err(io)?;
    let mut lipo = Command::new("lipo");
    lipo.args(["-create", "-output"]).arg(contents.join("MacOS/Splicr"));
    for target in TARGETS {
        lipo.arg(root.join("target").join(target).join("release/splicr"));
    }
    run(&mut lipo)?;
    std::fs::copy(root.join("assets/icon.icns"), contents.join("Resources/Splicr.icns")).map_err(io)?;
    std::fs::write(contents.join("Info.plist"), info_plist(env!("CARGO_PKG_VERSION"))).map_err(io)?;
    std::fs::write(contents.join("PkgInfo"), "APPL????").map_err(io)?;
    run(Command::new("plutil").arg("-lint").arg(contents.join("Info.plist")))?;
    // Attributes picked up while building (quarantine, FinderInfo) fail `codesign --verify --strict`.
    run(Command::new("xattr").arg("-cr").arg(&app))?;

    // Ad-hoc, inside out: macOS asks before opening it the first time, until there is a Developer ID
    // (ROADMAP, Later).
    let entitlements = root.join("packaging/macos/entitlements.plist");
    for code in [contents.join("MacOS/Splicr"), app.clone()] {
        let mut sign = Command::new("codesign");
        sign.args(["--force", "--sign", "-", "--timestamp=none", "--options", "runtime", "--entitlements"]);
        run(sign.arg(&entitlements).arg(code))?;
    }
    run(Command::new("codesign").args(["--verify", "--strict", "--deep", "--verbose=2"]).arg(&app))?;

    let stage = work.join("dmg");
    std::fs::create_dir_all(&stage).map_err(io)?;
    run(Command::new("ditto").arg(&app).arg(stage.join("Splicr.app")))?;
    run(Command::new("ln").args(["-s", "/Applications"]).arg(stage.join("Applications")))?;
    let dist = root.join("target/dist");
    std::fs::create_dir_all(&dist).map_err(io)?;
    let dmg = dist.join(format!("Splicr-{}-macos-universal.dmg", env!("CARGO_PKG_VERSION")));
    let _ = std::fs::remove_file(&dmg);
    // `create` attaches a device while it copies, which fails now and then on CI ("Resource busy").
    for attempt in 1..=5 {
        let mut create = Command::new("hdiutil");
        create.args(["create", "-srcfolder"]).arg(&stage);
        create.args(["-volname", "Splicr", "-fs", "HFS+", "-format", "UDZO", "-imagekey", "zlib-level=9", "-ov"]);
        match run(create.arg(&dmg)) {
            Ok(()) => break,
            Err(e) if attempt == 5 => return Err(e),
            Err(e) => {
                eprintln!("{e}; trying again");
                std::thread::sleep(Duration::from_secs(attempt * 10));
            }
        }
    }
    run(Command::new("codesign").args(["--force", "--sign", "-", "--timestamp=none"]).arg(&dmg))?;
    write_sha256(&dmg)?;
    println!("wrote {}", dmg.display());
    Ok(())
}

/// The bundle's `Info.plist`: the template, for `version`.
fn info_plist(version: &str) -> String {
    let short = version.split('-').next().unwrap_or(version);
    INFO_PLIST.replace("@VERSION@", version).replace("@SHORT_VERSION@", short)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_plist_is_filled_in_for_a_dock_app() {
        let p = info_plist("1.2.3-rc.1");
        assert!(p.contains("<string>1.2.3</string>") && p.contains("<string>1.2.3-rc.1</string>"), "{p}");
        assert!(!p.contains('@'), "every placeholder filled");
        assert!(p.contains(&format!("<string>{MINIMUM_MACOS}</string>")));
        // As ArtCraft's apps: a plain Dock app.
        assert!(!p.contains("<key>LSUIElement</key>"));
    }
}
