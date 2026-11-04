# Splicr: notes for contributors and agents

Open-source app to view, combine, relate, edit and export spreadsheets and CSV files, in pure Rust
(eframe/egui 0.37). It follows Remapr's architecture and design system. See README.md for the
layout, ROADMAP.md for the phases and docs/VISION.md for the architecture, the screens and the
decisions behind them.

## Rules

- Files are hostile. Nothing a file holds may crash Splicr or make it allocate without bound: cap
  sizes before reading (`splicr_io::MAX_FILE_BYTES`, `MAX_ENTRIES`, a sheet's `MAX_ROWS` and
  `MAX_COLS`), cap reads whatever a header claims, and report what was left out as a
  `splicr_io::Warning` beside what was read. A crash fix comes with a test that feeds the file.
- Never lose the user's data. Text stays text unless it is a plain number (`csv::field`); exports
  make names valid and unique (`splicr_core::names`), ask before replacing files, and write beside
  the target and rename (`splicr_io::write`): a failed or cancelled export leaves no half file.
- `splicr-core` is pure (`#![forbid(unsafe_code)]`, no I/O, no system or UI crates). What an export
  makes is decided there (`plan::Plan::outputs`), with tests, before anything is written.
- File work runs on the engine's thread (`splicr-engine`): the app sends `Command`s and drains
  `Event`s once per frame. Views never read or write files themselves.
- What the user has open and what an export takes is `apps/splicr/src/workspace.rs`, tested
  without a window; where the user is, `router.rs`. A new screen is a `Route`, a module in
  `views/`, and a row in VISION.md's table of screens.
- `unsafe` lives only in `crates/platform/src/windows/window.rs`. The workspace denies
  `unsafe_code`; that module opts out with `#[allow(unsafe_code)]`.
- Code for one system sits in its own `#[cfg]` module of `crates/platform` (`windows.rs` and
  `windows/`, `unix.rs` for macOS and Linux), not scattered through the app. `lib.rs` lists every
  public item once, and each system's module provides all of them, a no-op where it can't.
- UI code takes colours from `splicr_ui::Palette` and fonts from the theme. No hard-coded colours
  in apps. The `palette_contrast` test checks every pairing screens use; a new token gets its rows
  there. The palette is Remapr's: keep the two apps one family.
- Text is Inter on every system. Icons are Lucide SVGs in `crates/ui/assets/icons`, listed in
  `splicr_ui::icon`; add new ones unchanged, from the same Lucide version (1.64.0).
- New widgets go in `crates/ui`, report themselves with `widget_info` for screen readers, and go in
  the design snapshots (`crates/ui/tests/gallery.rs`): `cargo test -p splicr-ui --features
  snapshots` draws them, light and dark, with wgpu.
- `splicr-ui` is Splicr's own copy of Remapr's design system: it lives in this repository and is
  changed here, never shared with Remapr by a git dependency.
- User-facing text is written in English in the code, through `tr!` and `trn!` (counts), and
  translated in `apps/splicr/locales/<lang>.tsv` (craft-i18n's format). The test
  `catalogs_cover_the_code` fails when a catalog lacks a text the code uses. Code, comments and
  docs are English.
- The settings format has a version (`config::VERSION`). Changing it means bumping it, a reader for
  the old version, a file as the old version wrote it in `apps/splicr/tests/fixtures` (never edited
  afterwards), and a test that converts it.
- Code or assets taken from another project keep their licence notice and get a row in
  ATTRIBUTION.md. GridCraft and ArtCraft's repositories are MIT OR Apache-2.0: copy with care,
  adapt, and say what changed. Their brand files are not open source: never copy them.
- The title bar follows ArtCraft's craft-window rule, as Remapr does. Windows keeps its native
  frame, tinted with the palette (`tint_title_bar`). macOS runs the sidebar and header under a
  transparent title bar, keeps `MACOS_TITLE_BAR` points clear for the window buttons, and lets the
  empty parts drag the window (`splicr_ui::title_bar`). Linux keeps the desktop's frame.
- Icons are generated: edit `assets/logo-light.svg`, run `cargo xtask icons`, commit the outputs.
- Never change the MSI's `UpgradeCode`, nor the app id `io.github.bradmoyetones.splicr` (macOS's
  `CFBundleIdentifier`, Linux's `.desktop` file and window class).
- The updater reads only the version from GitHub and builds download URLs itself; keep it that
  way. Release asset names (`Splicr-<version>-x64.msi`, `Splicr-<version>-macos-universal.dmg`,
  `Splicr-<version>-linux-x86_64.deb` and `.AppImage`, and their `.sha256`) are a contract between
  `xtask dist`, the release workflow and `crates/update` (`splicr_update::package`).
- Releases are made by pushing a `v<version>` tag matching `Cargo.toml` (see README, Releasing).
- A new crate goes in `crates/<name>`, is named `splicr-<name>`, inherits workspace lints and gets
  its layer in `xtask/src/layers.rs`; `cargo xtask layers` fails until it has one.
- Libraries log through the `log` facade; only the app installs a logger (`logging.rs`).

## Before committing

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask layers
```

CI runs these on Windows, macOS and Linux. To check another system's code from yours, add its
target (`rustup target add x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`…) and run clippy
with `--target`.
