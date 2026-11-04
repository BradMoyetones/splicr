//! Which crate may depend on which. `cargo xtask layers` fails when a dependency breaks the
//! layering, so the architecture holds by more than good intentions:
//!
//! | Layer | Crates | May use |
//! |---|---|---|
//! | 0 | `splicr-core` | nothing of the workspace, no system or UI crates: it is pure |
//! | 0 | `splicr-platform`, `splicr-update` | system crates, nothing of the workspace |
//! | 1 | `splicr-io` | layer 0, no system or UI crates |
//! | 2 | `splicr-engine` | lower layers, no system or UI crates |
//! | 3 | `splicr-ui` | UI crates only: it knows nothing about tables or files |
//! | – | `splicr`, `xtask` | anything |
//!
//! After ArtCraft's vectorcraft (`xtask/src/layers.rs`): the rules work on a small model, apart
//! from `cargo metadata`, so they are tested on their own.

use std::process::Command;

use serde_json::Value;

/// Where a workspace crate sits and what it may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub layer: u8,
    /// Workspace crates of lower layers.
    pub workspace: bool,
    /// Crates that talk to an operating system.
    pub system: bool,
    /// UI toolkits and windowing.
    pub ui: bool,
}

/// Every workspace crate; `None` for the app and the tooling, which may use anything.
pub const TABLE: &[(&str, Option<Place>)] = &[
    ("splicr-core", Some(Place { layer: 0, workspace: false, system: false, ui: false })),
    ("splicr-platform", Some(Place { layer: 0, workspace: false, system: true, ui: false })),
    ("splicr-update", Some(Place { layer: 0, workspace: false, system: true, ui: false })),
    ("splicr-io", Some(Place { layer: 1, workspace: true, system: false, ui: false })),
    ("splicr-engine", Some(Place { layer: 2, workspace: true, system: false, ui: false })),
    ("splicr-ui", Some(Place { layer: 3, workspace: false, system: false, ui: true })),
    ("splicr", None),
    ("xtask", None),
];

/// Crates that talk to an operating system. A trailing `*` is a prefix.
const SYSTEM_CRATES: &[&str] = &[
    "windows-sys",
    "windows",
    "winapi",
    "objc2*",
    "core-foundation*",
    "core-graphics*",
    "cocoa*",
    "libc",
    "nix",
    "evdev*",
    "x11*",
    "wayland*",
];

/// UI toolkits and windowing.
const UI_CRATES: &[&str] = &["eframe", "egui*", "winit", "glow", "wgpu*", "tray-icon", "rfd"];

fn listed(list: &[&str], name: &str) -> bool {
    list.iter().any(|entry| match entry.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == *entry,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    pub workspace: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crate {
    pub name: String,
    pub deps: Vec<Dep>,
}

/// Every broken rule, sorted, in words.
pub fn check(crates: &[Crate]) -> Vec<String> {
    let place = |name: &str| TABLE.iter().find(|(n, _)| *n == name).map(|&(_, p)| p);
    let mut broken = Vec::new();
    for c in crates {
        let Some(this) = place(&c.name) else {
            broken.push(format!("{}: not in xtask/src/layers.rs TABLE; give it a layer", c.name));
            continue;
        };
        let Some(this) = this else { continue };
        for d in c.deps.iter().filter(|d| d.name != c.name) {
            if d.workspace {
                let lower = place(&d.name).flatten().is_some_and(|p| p.layer < this.layer);
                if !this.workspace {
                    broken.push(format!("{} uses {}: it may use no workspace crate", c.name, d.name));
                } else if !lower {
                    broken.push(format!("{} (layer {}) uses {}: only lower layers", c.name, this.layer, d.name));
                }
            } else if !this.system && listed(SYSTEM_CRATES, &d.name) {
                broken.push(format!("{} uses {}: it may use no system crate", c.name, d.name));
            } else if !this.ui && listed(UI_CRATES, &d.name) {
                broken.push(format!("{} uses {}: it may use no UI crate", c.name, d.name));
            }
        }
    }
    broken.sort();
    broken.dedup();
    broken
}

/// The workspace's crates and their direct dependencies, from `cargo metadata --no-deps`.
pub fn from_metadata(meta: &Value) -> Result<Vec<Crate>, String> {
    let packages = meta["packages"].as_array().ok_or("cargo metadata: no packages")?;
    let members: Vec<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
    packages
        .iter()
        .map(|p| {
            let name = p["name"].as_str().ok_or("cargo metadata: a package without a name")?.to_owned();
            let deps = p["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| d["name"].as_str())
                .map(|n| Dep { name: n.to_owned(), workspace: members.contains(&n) })
                .collect();
            Ok(Crate { name, deps })
        })
        .collect()
}

/// `cargo xtask layers`.
pub fn run(root: &std::path::Path) -> Result<(), String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    let broken = check(&from_metadata(&meta)?);
    if broken.is_empty() {
        println!("layers: every dependency is where it belongs");
        return Ok(());
    }
    for line in &broken {
        eprintln!("layers: {line}");
    }
    Err(format!("{} dependencies break the layering", broken.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, deps: &[(&str, bool)]) -> Crate {
        let deps = deps.iter().map(|&(n, workspace)| Dep { name: n.into(), workspace }).collect();
        Crate { name: name.into(), deps }
    }

    #[test]
    fn todays_layering_holds() {
        let crates = [
            krate("splicr-core", &[]),
            krate("splicr-platform", &[("windows-sys", false), ("sys-locale", false)]),
            krate("splicr-update", &[("serde_json", false), ("sha2", false)]),
            krate("splicr-io", &[("splicr-core", true), ("calamine", false), ("zip", false)]),
            krate("splicr-engine", &[("splicr-core", true), ("splicr-io", true), ("log", false)]),
            krate("splicr-ui", &[("eframe", false), ("egui_extras", false), ("egui_kittest", false)]),
            krate("splicr", &[("splicr-ui", true), ("rfd", false)]),
        ];
        assert_eq!(check(&crates), Vec::<String>::new());
    }

    #[test]
    fn broken_rules_are_named() {
        let crates = [
            krate("splicr-core", &[("windows-sys", false), ("egui", false)]),
            krate("splicr-io", &[("splicr-engine", true), ("rfd", false)]),
            krate("splicr-engine", &[("splicr-ui", true), ("libc", false)]),
            krate("splicr-ui", &[("splicr-core", true), ("objc2-app-kit", false)]),
            krate("splicr-sheets", &[]),
        ];
        let broken = check(&crates);
        assert_eq!(broken.len(), 9, "{broken:#?}");
        assert!(broken.iter().any(|b| b.starts_with("splicr-sheets: not in")));
        assert!(broken.contains(&"splicr-core uses windows-sys: it may use no system crate".into()));
        assert!(broken.contains(&"splicr-io (layer 1) uses splicr-engine: only lower layers".into()));
        assert!(broken.contains(&"splicr-engine (layer 2) uses splicr-ui: only lower layers".into()));
        assert!(broken.contains(&"splicr-ui uses splicr-core: it may use no workspace crate".into()));
    }
}
