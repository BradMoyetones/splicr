# Roadmap

Each phase ships a working app. A phase is done when it is merged, CI is green and it has been used
on real files. [docs/VISION.md](docs/VISION.md) explains the architecture and the screens.

## Phase 0.1: The rewrite (in progress)

What Sheetify did, in Rust, done right:

- Cargo workspace after Remapr's: `core`, `io`, `engine`, `platform`, `ui`, `update`, the app and
  `xtask`, with `cargo xtask layers`.
- Opening CSV, xlsx, xlsm, xlsb, xls, ods and zip archives of them, by dialog, by drag and drop, or
  from the command line. Encodings (UTF-8, UTF-16, Windows-1252, Windows-1256) and delimiters
  (`,` `;` tab) guessed. Leading zeros kept.
- The data grid: a million rows, frozen header and row numbers, resizable columns, arrow keys,
  copying a cell.
- Combining: one workbook with a sheet each, one stacked table (columns matched by header, with a
  column of origin), or a file each, as xlsx or CSV. Valid, unique names; replacing files only
  after asking; progress and cancel; no half files.
- Remapr's design system and branding; light, dark or the system's theme; English and Spanish.
- Settings saved atomically, logs, recent files.
- Updater, per-user MSI, universal DMG, deb and AppImage, CI on the three systems.

## Phase 0.2: Better reading

- Choosing the encoding, delimiter and header row of a CSV by hand when the guess is wrong, with a
  preview.
- Opening big files without holding them twice in memory: streaming CSV reading.
- Find in a table.
- File associations: "Open with Splicr" on Windows and macOS.

## Phase 0.3: Columns

- Column types (text, number, date, boolean) detected and changeable; dates read from text and
  from ODS.
- Filters and sorting in the grid.
- Choosing which columns an export takes, and their order.
- Splitting a table: by the value of a column, or every N rows.

## Phase 0.4: Relate

- Joining two tables by a key column: inner, left, full; a preview with the rows that matched and
  those that didn't.
- Removing duplicates.

## Phase 0.5: Edit

- Editing cells, renaming, adding and removing columns and rows, with undo.
- Saving presets: an export or a join to run again on next month's files.

## Later

- Signing: Windows (Azure Trusted Signing) and macOS (Developer ID, notarization).
- More languages.
- Parquet and JSON.
