//! Splicr's files: reads CSV, xlsx, xlsm, xlsb, xls and ods, and the ones inside `.zip` archives,
//! into tables; writes tables as xlsx and CSV.
//!
//! Every file is treated as hostile, as GridCraft treats them: sizes are capped before anything is
//! read ([`MAX_FILE_BYTES`], [`MAX_ENTRIES`]), sheets keep to a spreadsheet's limits, and what
//! couldn't be read comes back as a [`Warning`] beside what could, never as a crash.

mod archive;
pub mod csv;
mod sheets;
mod xlsx;

use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::{Path, PathBuf};

use splicr_core::plan::{Output, OutputFormat};
use splicr_core::{Format, Source};

/// The biggest file, or file inside an archive, read.
pub const MAX_FILE_BYTES: u64 = 512 << 20;
/// The most files read from one archive.
pub const MAX_ENTRIES: usize = 1000;

/// Why a file couldn't be read or written. The app puts it into words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The system's words.
    Io(String),
    TooBig,
    /// Not a spreadsheet, or a damaged one; the reader's words.
    Unreadable(String),
    /// An archive with no CSV or spreadsheet inside.
    NothingInside,
    /// A table too big for a sheet.
    TooManyRows {
        sheet: String,
    },
    TooManyColumns {
        sheet: String,
    },
    /// The xlsx writer's words.
    Write(String),
    Cancelled,
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

/// What was read, but not all of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// Only the first `kept` rows of a table fit in a sheet.
    RowsCut {
        table: String,
        kept: usize,
    },
    ColumnsCut {
        table: String,
        kept: usize,
    },
    /// A sheet that couldn't be read; the reader's words.
    SheetSkipped {
        sheet: String,
        error: String,
    },
    /// Files in an archive that aren't tables, or couldn't be read.
    EntriesSkipped {
        names: Vec<String>,
    },
}

/// A file read: its sources (several for an archive) and what was left out.
#[derive(Debug, Default)]
pub struct Opened {
    pub sources: Vec<Source>,
    pub warnings: Vec<Warning>,
}

/// The extensions Splicr opens, for file dialogs.
pub const EXTENSIONS: &[&str] = &["csv", "tsv", "txt", "xlsx", "xlsm", "xlsb", "xls", "ods", "zip"];

/// What `bytes` hold, by their content rather than their name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Table(Format),
    Archive,
}

fn sniff(bytes: &[u8]) -> Kind {
    const OLE2: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    if bytes.starts_with(&OLE2) {
        return Kind::Table(Format::Xls);
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return archive::sniff(bytes);
    }
    Kind::Table(Format::Csv)
}

/// Reads the file at `path`.
pub fn open(path: &Path) -> Result<Opened, Error> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_FILE_BYTES {
        return Err(Error::TooBig);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(Error::TooBig);
    }
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut opened = Opened::default();
    match sniff(&bytes) {
        Kind::Archive => archive::read(path, &bytes, &mut opened)?,
        Kind::Table(format) => {
            let tables = read_tables(format, &bytes, &name, &mut opened.warnings)?;
            opened.sources.push(Source { path: path.to_owned(), entry: None, format, tables });
        }
    }
    Ok(opened)
}

fn read_tables(
    format: Format,
    bytes: &[u8],
    name: &str,
    warnings: &mut Vec<Warning>,
) -> Result<Vec<std::sync::Arc<splicr_core::Table>>, Error> {
    match format {
        Format::Csv => Ok(vec![std::sync::Arc::new(csv::read(bytes, name, warnings))]),
        format => sheets::read(format, bytes, warnings),
    }
}

/// Writes `output`, calling `on_row` after each row; it returns `false` to stop, which leaves no
/// file behind. The file is written beside its place and moved there once complete, so a failed
/// or cancelled export never leaves half a file, nor destroys the one it would have replaced.
pub fn write(output: &Output, mut on_row: impl FnMut() -> bool) -> Result<(), Error> {
    let partial = partial_path(&output.path);
    let result = match output.format {
        OutputFormat::Xlsx => xlsx::write(&output.sheets, &partial, &mut on_row),
        OutputFormat::Csv => {
            let table = output.sheets.first().map(|(_, t)| t.clone()).unwrap_or_default();
            let mut file = BufWriter::new(File::create(&partial)?);
            match csv::write(&table, &mut file, &mut on_row) {
                Ok(true) => file.into_inner().map(drop).map_err(|e| Error::Io(e.error().to_string())),
                Ok(false) => Err(Error::Cancelled),
                Err(e) => Err(e.into()),
            }
        }
    };
    match result {
        Ok(()) => std::fs::rename(&partial, &output.path).map_err(Error::from),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(e)
        }
    }
}

/// `report.xlsx` → `.report.xlsx.partial`, hidden on macOS and Linux while it is written.
fn partial_path(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.partial"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use splicr_core::plan::{Item, Mode, Plan};
    use splicr_core::{Table, Value};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("splicr-io-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample(name: &str) -> Arc<Table> {
        let mut t = Table::new(name);
        t.push_row(vec![Value::text("id"), Value::text("fecha"), Value::text("ok")]);
        t.push_row(vec![Value::text("007"), Value::DateTime(45_000.5), Value::Bool(true)]);
        t.push_row(vec![Value::Number(2.5), Value::Empty, Value::Error("#N/A".into())]);
        Arc::new(t)
    }

    #[test]
    fn a_workbook_reads_back_as_written() {
        let dir = temp_dir("roundtrip");
        let plan = Plan {
            mode: Mode::Workbook,
            format: OutputFormat::Xlsx,
            items: vec![
                Item { table: sample("a"), name: "Ventas".into() },
                Item { table: sample("b"), name: "ventas".into() },
            ],
            folder: dir.clone(),
            file_name: "Unido".into(),
            source_column: None,
        };
        let outputs = plan.outputs();
        write(&outputs[0], || true).unwrap();
        let opened = open(&dir.join("Unido.xlsx")).unwrap();
        assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
        let source = &opened.sources[0];
        assert_eq!(source.format, Format::Xlsx);
        let names: Vec<&str> = source.tables.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Ventas", "ventas (2)"]);
        let t = &source.tables[0];
        assert_eq!(t.get(1, 0), &Value::text("007"));
        assert_eq!(t.get(1, 1), &Value::DateTime(45_000.5));
        assert_eq!(t.get(1, 2), &Value::Bool(true));
        assert_eq!(t.get(2, 0), &Value::Number(2.5));
        assert_eq!(t.get(2, 2).display(), "#N/A");
        assert!(!dir.join(".Unido.xlsx.partial").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_cancelled_export_leaves_nothing() {
        let dir = temp_dir("cancel");
        let output =
            Output { path: dir.join("x.csv"), format: OutputFormat::Csv, sheets: vec![("x".into(), sample("x"))] };
        std::fs::write(&output.path, "before").unwrap();
        assert_eq!(write(&output, || false), Err(Error::Cancelled));
        assert_eq!(std::fs::read_to_string(&output.path).unwrap(), "before");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn content_decides_the_format() {
        assert_eq!(sniff(b"a,b\n1,2"), Kind::Table(Format::Csv));
        assert_eq!(sniff(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0]), Kind::Table(Format::Xls));
    }

    #[test]
    fn huge_files_are_refused_before_reading() {
        let dir = temp_dir("big");
        let path = dir.join("big.csv");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();
        assert_eq!(open(&path).unwrap_err(), Error::TooBig);
        let _ = std::fs::remove_dir_all(dir);
    }
}
