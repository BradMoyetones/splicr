//! What an export makes: which files, with which sheets, from which tables. Worked out here, apart
//! from writing, so names and collisions are settled (and tested) before anything touches the disk.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::names::{file_stem, unique_file_names, unique_sheet_names};
use crate::{Table, Value};

/// How the chosen tables become files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// One workbook, one sheet per table.
    #[default]
    Workbook,
    /// One file per table.
    Files,
    /// One table: every table's rows one after another, columns matched by their header.
    Stack,
}

/// The kind of file written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Xlsx,
    /// UTF-8 with a byte order mark, which Excel needs to read the accents right.
    Csv,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Xlsx => "xlsx",
            OutputFormat::Csv => "csv",
        }
    }
}

/// A table to export, under the name the user gave it (a sheet's or a file's).
#[derive(Clone, Debug)]
pub struct Item {
    pub table: Arc<Table>,
    pub name: String,
}

/// What the user asked for.
#[derive(Clone, Debug)]
pub struct Plan {
    pub mode: Mode,
    /// A CSV holds one table: with [`Mode::Workbook`] it is ignored and the workbook is xlsx.
    pub format: OutputFormat,
    pub items: Vec<Item>,
    pub folder: PathBuf,
    /// The file's name, without extension, for [`Mode::Workbook`] and [`Mode::Stack`].
    pub file_name: String,
    /// [`Mode::Stack`]: a first column naming each row's table, with this title.
    pub source_column: Option<String>,
}

/// One file to write.
#[derive(Clone, Debug)]
pub struct Output {
    pub path: PathBuf,
    pub format: OutputFormat,
    /// Its sheets: name and table. A CSV has one, whose name is unused.
    pub sheets: Vec<(String, Arc<Table>)>,
}

impl Plan {
    /// The files this plan writes, with every name made valid and unique.
    pub fn outputs(&self) -> Vec<Output> {
        match self.mode {
            Mode::Workbook => {
                let names = unique_sheet_names(self.items.iter().map(|i| i.name.as_str()));
                let sheets = names.into_iter().zip(&self.items).map(|(n, i)| (n, i.table.clone())).collect();
                vec![self.single(OutputFormat::Xlsx, sheets)]
            }
            Mode::Files => {
                let names = unique_file_names(self.items.iter().map(|i| i.name.as_str()), self.format.extension());
                names
                    .into_iter()
                    .zip(&self.items)
                    .map(|(file, item)| {
                        let sheet = unique_sheet_names([item.name.as_str()]).remove(0);
                        Output {
                            path: self.folder.join(file),
                            format: self.format,
                            sheets: vec![(sheet, item.table.clone())],
                        }
                    })
                    .collect()
            }
            Mode::Stack => {
                let tables: Vec<(&str, &Table)> = self.items.iter().map(|i| (i.name.as_str(), &*i.table)).collect();
                let stacked = stack(&tables, self.source_column.as_deref());
                let sheet = unique_sheet_names([self.file_name.as_str()]).remove(0);
                vec![self.single(self.format, vec![(sheet, Arc::new(stacked))])]
            }
        }
    }

    fn single(&self, format: OutputFormat, sheets: Vec<(String, Arc<Table>)>) -> Output {
        let file = format!("{}.{}", file_stem(&self.file_name), format.extension());
        Output { path: self.folder.join(file), format, sheets }
    }

    /// The files this plan would replace, for asking before it does.
    pub fn existing(&self, exists: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
        self.outputs().into_iter().map(|o| o.path).filter(|p| exists(p)).collect()
    }
}

/// Every table's rows under one header. Each table's first row is its header; a column goes under
/// the header cell of the same name (the first `id` under the first `id`, a second `id` under a
/// second), and names first seen in a later table are added on the right. With `source_column`,
/// a first column of that title names each row's table.
pub fn stack(tables: &[(&str, &Table)], source_column: Option<&str>) -> Table {
    let mut header: Vec<(String, usize)> = Vec::new();
    let mut maps: Vec<Vec<usize>> = Vec::new();
    for (_, table) in tables {
        let mut seen: Vec<(String, usize)> = Vec::new();
        let map = table
            .header()
            .into_iter()
            .map(|name| {
                let key = name.to_lowercase();
                let nth = seen.iter().filter(|(k, _)| *k == key).count();
                seen.push((key.clone(), nth));
                match header.iter().position(|(k, n)| *k == key && *n == nth) {
                    Some(at) => at,
                    None => {
                        header.push((key, nth));
                        header.len() - 1
                    }
                }
            })
            .collect();
        maps.push(map);
    }
    // The header's text as the first table that has the column wrote it.
    let mut names = vec![String::new(); header.len()];
    for ((_, table), map) in tables.iter().zip(&maps) {
        for (name, &at) in table.header().into_iter().zip(map) {
            if names[at].is_empty() {
                names[at] = name;
            }
        }
    }

    let offset = usize::from(source_column.is_some());
    let mut out = Table::new("stack");
    out.push_row(source_column.into_iter().map(Value::text).chain(names.iter().map(|n| Value::text(n))).collect());
    for ((name, table), map) in tables.iter().zip(&maps) {
        let source = Value::text(name);
        for r in 1..table.rows() {
            let mut row = vec![Value::Empty; offset + header.len()];
            if offset == 1 {
                row[0] = source.clone();
            }
            for (c, value) in table.row(r).iter().enumerate() {
                if let Some(&at) = map.get(c) {
                    row[offset + at] = value.clone();
                }
            }
            out.push_row(row);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(name: &str, rows: &[&[&str]]) -> Table {
        let mut t = Table::new(name);
        for row in rows {
            t.push_row(row.iter().map(|s| Value::text(s)).collect());
        }
        t
    }

    fn texts(t: &Table) -> Vec<Vec<String>> {
        (0..t.rows()).map(|r| (0..t.cols()).map(|c| t.get(r, c).display().into_owned()).collect()).collect()
    }

    fn plan(mode: Mode, names: &[&str]) -> Plan {
        Plan {
            mode,
            format: OutputFormat::Xlsx,
            items: names.iter().map(|n| Item { table: Arc::new(table(n, &[&["a"]])), name: (*n).into() }).collect(),
            folder: PathBuf::from("/out"),
            file_name: "Unido".into(),
            source_column: None,
        }
    }

    #[test]
    fn a_workbook_gets_unique_sheet_names() {
        let outputs = plan(Mode::Workbook, &["Ventas", "ventas", "a:b"]).outputs();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].path, Path::new("/out/Unido.xlsx"));
        let sheets: Vec<&str> = outputs[0].sheets.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(sheets, ["Ventas", "ventas (2)", "a_b"]);
    }

    #[test]
    fn files_get_unique_file_names() {
        let mut p = plan(Mode::Files, &["Enero", "enero"]);
        p.format = OutputFormat::Csv;
        let paths: Vec<PathBuf> = p.outputs().into_iter().map(|o| o.path).collect();
        assert_eq!(paths, [PathBuf::from("/out/Enero.csv"), PathBuf::from("/out/enero (2).csv")]);
        assert_eq!(p.existing(|path| path.ends_with("Enero.csv")), [PathBuf::from("/out/Enero.csv")]);
    }

    #[test]
    fn stacking_matches_columns_by_header() {
        let a = table("a", &[&["id", "Name"], &["1", "Ana"], &["2", "Luis"]]);
        let b = table("b", &[&["name", "ID", "city"], &["Eva", "3", "Lima"]]);
        let stacked = stack(&[("enero", &a), ("febrero", &b)], Some("Origen"));
        assert_eq!(
            texts(&stacked),
            [
                ["Origen", "id", "Name", "city"],
                ["enero", "1", "Ana", ""],
                ["enero", "2", "Luis", ""],
                ["febrero", "3", "Eva", "Lima"],
            ]
        );
    }

    #[test]
    fn repeated_headers_keep_their_order() {
        let a = table("a", &[&["x", "x"], &["1", "2"]]);
        let b = table("b", &[&["x"], &["3"]]);
        assert_eq!(texts(&stack(&[("a", &a), ("b", &b)], None)), [["x", "x"], ["1", "2"], ["3", ""]]);
    }
}
