//! A table: one sheet of a workbook, or one CSV file.

use std::path::PathBuf;
use std::sync::Arc;

use crate::Value;

static EMPTY: Value = Value::Empty;

/// Rows of cells, as read. Rows may be shorter than the widest one: what is missing is empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    /// The sheet's name, or the file's name without its extension.
    pub name: String,
    rows: Vec<Box<[Value]>>,
    width: usize,
}

impl Table {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), rows: Vec::new(), width: 0 }
    }

    /// Adds a row at the bottom. Empty cells at its end are dropped: they hold nothing.
    pub fn push_row(&mut self, mut row: Vec<Value>) {
        while row.last().is_some_and(Value::is_empty) {
            row.pop();
        }
        self.width = self.width.max(row.len());
        self.rows.push(row.into_boxed_slice());
    }

    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    /// The widest row's length.
    pub fn cols(&self) -> usize {
        self.width
    }

    /// The cell at `row`, `col`; empty outside the table.
    pub fn get(&self, row: usize, col: usize) -> &Value {
        self.rows.get(row).and_then(|r| r.get(col)).unwrap_or(&EMPTY)
    }

    /// A row's cells, without the empty ones at its end.
    pub fn row(&self, row: usize) -> &[Value] {
        self.rows.get(row).map_or(&[], |r| r)
    }

    /// The column names: the first row, read as text. Unnamed columns are called by their letter,
    /// as spreadsheets label them (`C`).
    pub fn header(&self) -> Vec<String> {
        (0..self.width)
            .map(|col| match self.get(0, col).display().trim() {
                "" => column_letters(col),
                name => name.to_owned(),
            })
            .collect()
    }
}

/// A spreadsheet's name for column `col` (0-based): `A`, `Z`, `AA`, `XFD`.
pub fn column_letters(col: usize) -> String {
    let mut n = col + 1;
    let mut letters = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    letters.reverse();
    String::from_utf8(letters).unwrap_or_default()
}

/// The kind of file a source came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Xlsx,
    Xlsb,
    Xls,
    Ods,
}

impl Format {
    /// How the format is labelled on screen.
    pub fn label(self) -> &'static str {
        match self {
            Format::Csv => "CSV",
            Format::Xlsx => "XLSX",
            Format::Xlsb => "XLSB",
            Format::Xls => "XLS",
            Format::Ods => "ODS",
        }
    }
}

/// A file read into Splicr, with its tables: one for a CSV, one per sheet for a workbook. A file
/// inside a `.zip` archive is a source of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub path: PathBuf,
    /// The file's path inside the archive at `path`, when it came from one.
    pub entry: Option<String>,
    pub format: Format,
    pub tables: Vec<Arc<Table>>,
}

impl Source {
    /// The file's own name: the entry's in an archive.
    pub fn file_name(&self) -> String {
        match &self.entry {
            Some(entry) => entry.rsplit('/').next().unwrap_or(entry).to_owned(),
            None => self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        }
    }

    /// The archive's name, when the file came from one.
    pub fn archive_name(&self) -> Option<String> {
        self.entry.as_ref()?;
        self.path.file_name().map(|n| n.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_may_be_ragged() {
        let mut t = Table::new("t");
        t.push_row(vec![Value::text("a"), Value::text("b"), Value::Empty]);
        t.push_row(vec![Value::Number(1.0)]);
        assert_eq!((t.rows(), t.cols()), (2, 2));
        assert_eq!(t.get(1, 1), &Value::Empty);
        assert_eq!(t.get(9, 9), &Value::Empty);
        assert_eq!(t.row(0).len(), 2);
    }

    #[test]
    fn unnamed_columns_take_their_letter() {
        let mut t = Table::new("t");
        t.push_row(vec![Value::text("id"), Value::Empty, Value::text(" name ")]);
        assert_eq!(t.header(), ["id", "B", "name"]);
    }

    #[test]
    fn column_letters_follow_spreadsheets() {
        assert_eq!(column_letters(0), "A");
        assert_eq!(column_letters(25), "Z");
        assert_eq!(column_letters(26), "AA");
        assert_eq!(column_letters(16_383), "XFD");
    }

    #[test]
    fn sources_name_their_file() {
        let zip = Source {
            path: PathBuf::from("/data/2026.zip"),
            entry: Some("ventas/enero.csv".into()),
            format: Format::Csv,
            tables: vec![],
        };
        assert_eq!(zip.file_name(), "enero.csv");
        assert_eq!(zip.archive_name().as_deref(), Some("2026.zip"));
    }
}
