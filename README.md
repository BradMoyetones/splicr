<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo-light.svg" width="128" alt="Splicr logo: a sheet with an S">
  </picture>
</p>

<h1 align="center">Splicr</h1>

<p align="center">View, combine and export spreadsheets and CSV files. Open source, written entirely in Rust.</p>

Drop in your CSV files, Excel workbooks or zip archives full of them, look through them, and get
back the file you need: one workbook with a sheet each, one table with every row, or a file each.
No web views, no runtime, no account.

Splicr is the rewrite of Sheetify (Python and Qt), whose last version lives on the
[`legacy/python`](https://github.com/BradMoyetones/splicr/tree/legacy/python) branch. It shares
[Remapr](https://github.com/BradMoyetones/remapr)'s architecture and look.

## What it does

- Opens CSV, Excel (xlsx, xlsm, xlsb, xls) and OpenDocument (ods) files, and the ones inside zip
  archives, several at once: from a dialog, by dragging them onto the window, or from the command
  line.
- Reads CSV files the way they come: UTF-8, UTF-16 or Windows' own encodings, separated by commas,
  semicolons or tabs. `007` stays `007`.
- Shows every table in a grid that stays fast with a million rows: frozen header and row numbers,
  resizable columns, arrow keys, copy a cell.
- Combines tables into:
  - **one workbook**, with a sheet for each table;
  - **one stacked table**, every table's rows one after another, columns matched by their header,
    with a column saying where each row came from;
  - **a file each**, as xlsx or CSV.
- Names sheets and files the way Excel and every system accept them, asks before replacing a file,
  and never leaves half a file behind, even when an export is cancelled.
- Light, dark or the system's theme; English or Spanish, following the system.
- Updates itself: on Windows (the installer, with no administrator), on macOS (the app in
  Applications is replaced) and from an AppImage on Linux. Every package is checked against its
  SHA-256 and the project's signature before it is used.

Where it is going (relating tables, column types, filters, editing) is in
[ROADMAP.md](ROADMAP.md), and the architecture behind it in [docs/VISION.md](docs/VISION.md).

## Installing

- **Windows 10 or 11 (x64):** `Splicr-<version>-x64.msi`. It installs for your user only, so it
  asks for no administrator rights, and updates itself the same way.
- **macOS 11 or newer** (Apple silicon or Intel): `Splicr-<version>-macos-universal.dmg`. Drag
  Splicr to Applications. It isn't notarized yet: the first time, Control-click it and choose Open.
- **Linux on x86_64** (glibc 2.35 or newer: Ubuntu 22.04, Debian 12, Fedora 36 and later):
  `Splicr-<version>-linux-x86_64.deb` (`sudo apt install ./Splicr-….deb`), or the `.AppImage`,
  which runs as it is (`chmod +x` it first).

Settings and logs live in `%APPDATA%\Splicr` on Windows, `~/Library/Application Support/Splicr` on
macOS and `~/.config/Splicr` on Linux.

## Development

You need Rust 1.95 or newer, from [rustup.rs](https://rustup.rs). On Windows, the MSVC toolchain
and the Windows SDK's `rc.exe`, which embeds the icon into the executable.

```sh
cargo run                 # the app, in debug mode
cargo run -- data.csv     # opening files at start
```

Debug builds keep their own settings (`Splicr-dev`), so they never touch an installed copy's.
`RUST_LOG=debug cargo run` logs more; `SPLICR_LANG=es cargo run` tries the Spanish interface.

The checks CI runs, on Windows, macOS and Linux:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask layers
```

`cargo test -p splicr-ui --features snapshots` draws the design system, light and dark, into
`crates/ui/tests/snapshots/` (`UPDATE_SNAPSHOTS=1` writes new ones).

### Layout

```
apps/splicr/        the app: router, workspace, screens (views/), settings, i18n, updates
crates/core/        values, tables, naming rules, export plans: pure
crates/io/          reading and writing files
crates/engine/      the file work, on its own thread
crates/platform/    the operating system: settings folder, opening files, the title bar
crates/ui/          the design system and the data grid
crates/update/      the updater
xtask/              icons, packages, layer checks
packaging/          the MSI, the macOS bundle, the Linux packages
```

### Packages

`cargo xtask dist` builds this system's package into `target/dist`, with its SHA-256: the MSI on
Windows (WiX 5, fetched on first use; needs the .NET runtime 6 or newer), the universal DMG on
macOS (needs both Apple targets: `rustup target add aarch64-apple-darwin x86_64-apple-darwin`),
the deb and the AppImage on Linux.

### Releasing

1. Set the version in `Cargo.toml` (`[workspace.package]`), by a pull request into `main`.
2. Tag `main` and push the tag: `git tag v0.1.1 && git push origin v0.1.1`.
3. The release workflow builds the three systems' packages, signs them (`cargo xtask sign`, with
   the repository secret `SPLICR_SIGNING_KEY`) and publishes them, each with its SHA-256 and
   signature, as a GitHub release. Installed copies find it there.

The signing key was made once with `cargo xtask keygen <folder outside the repository>`: its
private half is the secret `SPLICR_SIGNING_KEY` (and an offline backup), its public half
`splicr_update::PUBLIC_KEY`. Without the secret, a release fails rather than publish unsigned
packages.

## Licence

MIT, see [LICENSE](LICENSE). Third-party parts keep their licences: [ATTRIBUTION.md](ATTRIBUTION.md).
