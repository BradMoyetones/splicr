# Vision

Where Splicr is headed and in what order. [ROADMAP.md](../ROADMAP.md) lists the phases; this
document explains the reasoning behind them: the architecture they build toward, the screens and
routes of the app, and the decisions already taken.

Written on 2026-11-04, at version 0.1.0. Update it when a phase changes the picture.

## Where Splicr is going

Splicr started as Sheetify, a Python and Qt app that turned CSV files (loose or inside zip archives)
into Excel workbooks. Its last version is kept on the `legacy/python` branch and the `python-final`
tag. Splicr is that app rewritten from scratch in Rust, and growing into a place to work with
tabular files:

- **Look** at CSV, xlsx, xls, xlsb and ods files, one or many at once, in a grid that stays fast
  with a million rows.
- **Combine** them: one workbook with a sheet each, one stacked table, or a file each.
- **Relate** them: join two tables by a column they share.
- **Edit** them: cells, columns, types, filters.
- **Export** what comes out as xlsx or CSV.

It is not a spreadsheet. GridCraft is that (formulas, charts, styles). Splicr works on tables as
data: rows and columns, read from many files and written to new ones.

Three things don't change along the way:

- It stays pure Rust and MIT, with Remapr's architecture and look.
- Every file is hostile input: Splicr never crashes on a file, and never loses one.
- Every phase ships an app that works.

## Architecture

```
crates/
  core/      splicr-core      Value, Table, Source, naming rules, export plans, stacking.
                              Pure: no I/O, no system or UI crates, no unsafe.
  io/        splicr-io        Reading CSV (encoding and delimiter sniffing), xlsx/xls/xlsb/ods
                              (calamine) and zip archives; writing xlsx (rust_xlsxwriter) and CSV.
                              Size caps and warnings: no file can crash it.
  engine/    splicr-engine    The file work on a thread of its own: commands in, events out,
                              progress and cancelling.
  platform/  splicr-platform  Settings folder, languages, opening and revealing files, the native
                              title bar: one facade, one module per system.
  ui/        splicr-ui        Remapr's design system (tokens, Inter, Lucide, widgets), plus
                              Splicr's own widgets and the data grid.
  update/    splicr-update    Remapr's updater: GitHub Releases, SHA-256, the per-user MSI.
apps/splicr/                  The shell: router, workspace, screens, settings, i18n, logging.
xtask/                        icons, dist (MSI, DMG, deb, AppImage), layers.
packaging/{windows,macos,linux}/
```

Apps depend on crates, never the other way around; `cargo xtask layers` enforces it:

| Layer | Crates | May use |
|---|---|---|
| 0 | `splicr-core` | nothing: it is pure |
| 0 | `splicr-platform`, `splicr-update` | system crates |
| 1 | `splicr-io` | layer 0 |
| 2 | `splicr-engine` | layers 0 and 1 |
| 3 | `splicr-ui` | UI crates only: it knows nothing about tables or files |
| – | `splicr`, `xtask` | anything |

### How the app is put together

Remapr's split between what happens and what is drawn, applied to files:

- **`workspace.rs`** holds what the user has open (sources, their tables) and what an export takes
  (which tables, under which names, how and where). It is tested without a window.
- **`router.rs`** holds where the user is: a `Route` per screen, a history for Back, and the rule
  that a closed table leaves the history.
- **`views/<screen>.rs`** draw one screen each from the app's state. They never touch files: they
  send commands to the engine (`Command::Open`, `Command::Export`).
- **`app.rs`** draws the frame (sidebar, header, notices, status bar), drains the engine's events
  once per frame, and saves the settings.

The engine owns the slow work. Opening a 300 MB CSV or writing a million rows happens on its thread,
with progress events. The window never waits on a disk.

## Screens and routes

The sidebar has the sections. Each section is one or more routes:

| Section | Route | Screen | Since |
|---|---|---|---|
| Home | `Home` | Drop zone, recent files, what Splicr does | 0.1 |
| Data | `Data(Option<TableKey>)` | Open files as cards, their tables as a list, the picked one in the grid | 0.1 |
| Combine | `Combine` | 1. how (workbook, a file each, stacked) 2. which tables and names 3. where; export with progress | 0.1 |
| Relate | `Relate` | Two tables, the key column of each, the kind of join, a preview | 0.4 |
| Data | `Data(Some(key))` + column panel | Column types, filters, sorting, find | 0.3 |
| Data | `Data(Some(key))` editing | Cells and columns, undo | 0.5 |
| Settings | `Settings` | Theme, sidebar, updates, about | 0.1 |

Shortcuts: `Cmd/Ctrl+O` opens files, `Cmd/Ctrl+1…3` the sections, `Cmd/Ctrl+,` Settings,
`Cmd/Ctrl+[` goes back. In the grid, the arrows move the selection and `Cmd/Ctrl+C` copies the cell.

## Decisions

1. **Remapr's architecture and design system, copied, not shared.** `splicr-ui` starts as a copy of
   `remapr-ui`: the same palette (indigo accent, near-black dark theme, contrast checked by a test),
   Inter, Lucide and the title bar rules. Each app owns its UI crate in its own repository, and the
   two may drift apart as each needs. Remapr's key widgets (`kbd`) were left out; Splicr's
   navigation, list items, choice cards, drop zone and grid were added in the same style.
2. **GridCraft is a reference, not a dependency.** It is MIT OR Apache-2.0, so code may be copied
   with its notice (ATTRIBUTION.md). Taken: the CSV decoding, delimiter sniffing and record parser
   (`crates/io/src/csv.rs`), sniffing formats by content, warnings next to results instead of
   failures, size caps on hostile input, and the grid's way of painting only the visible span with
   frozen parts drawn from the viewport. Not taken: its xlsx crate (it brings formulas, styles and
   charts), its engine and command system, and its forks of egui and winit.
3. **calamine to read, rust_xlsxwriter to write.** Both pure Rust and permissively licensed. Splicr
   detects the format itself (OLE2 signature, the parts inside a zip) and opens calamine's reader
   for it directly, so a file's bytes are never copied once per format tried.
   `rust_xlsxwriter`'s constant-memory mode writes rows straight to disk.
4. **Text stays text.** A CSV field becomes a number only when it is written as a plain one:
   `007`, `+57…`, `1.234,5`, `1e5` and IDs longer than 15 digits stay text, so nothing is lost.
   Typed columns, chosen by the user, come in 0.3.
5. **An export never destroys anything.** Names are made valid and unique (`ventas (2)`), files
   that would be replaced are listed and confirmed first, and each file is written beside its
   place and moved there once complete. A failed or cancelled export leaves no half file.
6. **Per-user installs, self-updates everywhere they can be.** The Windows MSI installs into
   `%LOCALAPPDATA%\Programs\Splicr`: no administrator rights to install or to update. On macOS
   the updater swaps the `.app` for the one in the new DMG, as Tauri's does, and on Linux the
   AppImage for the new one; then the new Splicr starts. A deb, or an app run from its DMG or a
   folder it can't write to, gets the release page instead.
7. **A fresh version line.** The Python app's releases (v1.0.0 to v1.2.1) had no updater, so no
   installed copy depends on them. Splicr starts at 0.1.0.
8. **Trust comes from the project's signature, not the host.** Every package carries an ed25519
   signature made in the release workflow with the project's key; the updater checks it, and the
   SHA-256, before using anything. A Developer ID isn't needed for updates: a package the app
   downloads itself carries no quarantine, and the signature proves where it came from. Apple's
   signing and notarization still matter for the first install (ROADMAP, Later).
