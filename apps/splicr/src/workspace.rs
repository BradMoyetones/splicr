//! What the user has open and what an export takes from it, apart from drawing: the files read,
//! each table's place in the export and its name there, and the export's settings. The screens
//! show and change it; [`Workspace::plan`] turns it into what the engine writes. Tested without a
//! window, as Remapr's session is.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use splicr_core::plan::{Item, Mode, OutputFormat, Plan};
use splicr_core::{Source, Table};

/// A table, by its source's number and its place in that source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableKey {
    pub source: u64,
    pub index: usize,
}

/// A source as the workspace holds it.
#[derive(Debug)]
pub struct Entry {
    /// Stays the same while the source is open, even when it is read again.
    pub id: u64,
    pub source: Source,
    /// One per table: whether the export takes it, and its name there.
    pub picks: Vec<Pick>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub include: bool,
    pub name: String,
}

/// How the next export is made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportSettings {
    pub mode: Mode,
    pub format: OutputFormat,
    pub folder: Option<PathBuf>,
    /// The file's name for [`Mode::Workbook`] and [`Mode::Stack`].
    pub file_name: String,
    /// [`Mode::Stack`]: a first column naming each row's table.
    pub source_column: bool,
}

#[derive(Debug, Default)]
pub struct Workspace {
    pub entries: Vec<Entry>,
    next_id: u64,
    pub export: ExportSettings,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            mode: Mode::Workbook,
            format: OutputFormat::Xlsx,
            folder: None,
            file_name: String::new(),
            source_column: true,
        }
    }
}

impl Workspace {
    /// Adds sources read from one file. A source already open (the same file, or the same file of
    /// the same archive) is replaced where it is, keeping the user's picks for tables of the same
    /// name. Returns the first table added, to show it.
    pub fn add(&mut self, sources: Vec<Source>) -> Option<TableKey> {
        let mut first = None;
        for source in sources {
            let picks: Vec<Pick> =
                source.tables.iter().map(|t| Pick { include: true, name: default_name(&source, t) }).collect();
            let same = |e: &Entry| e.source.path == source.path && e.source.entry == source.entry;
            let id = match self.entries.iter_mut().find(|e| same(e)) {
                Some(entry) => {
                    let kept = |t: &Arc<Table>, fresh: Pick| {
                        let old = entry.source.tables.iter().position(|o| o.name == t.name);
                        old.and_then(|i| entry.picks.get(i).cloned()).unwrap_or(fresh)
                    };
                    entry.picks = source.tables.iter().zip(picks).map(|(t, p)| kept(t, p)).collect();
                    entry.source = source;
                    entry.id
                }
                None => {
                    self.next_id += 1;
                    self.entries.push(Entry { id: self.next_id, source, picks });
                    self.next_id
                }
            };
            first.get_or_insert(TableKey { source: id, index: 0 });
        }
        if self.export.file_name.is_empty() {
            self.export.file_name = self.entries.first().map(|e| file_stem(&e.source)).unwrap_or_default();
        }
        first
    }

    pub fn remove(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
    }

    pub fn entry(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn table(&self, key: TableKey) -> Option<&Arc<Table>> {
        self.entry(key.source)?.source.tables.get(key.index)
    }

    pub fn pick_mut(&mut self, key: TableKey) -> Option<&mut Pick> {
        self.entries.iter_mut().find(|e| e.id == key.source)?.picks.get_mut(key.index)
    }

    /// Every table, in order.
    pub fn keys(&self) -> impl Iterator<Item = TableKey> + '_ {
        self.entries.iter().flat_map(|e| (0..e.source.tables.len()).map(move |index| TableKey { source: e.id, index }))
    }

    pub fn table_count(&self) -> usize {
        self.entries.iter().map(|e| e.source.tables.len()).sum()
    }

    /// The tables the export takes.
    pub fn included(&self) -> usize {
        self.entries.iter().flat_map(|e| &e.picks).filter(|p| p.include).count()
    }

    /// Takes every table, or none when all are taken already.
    pub fn toggle_all(&mut self) {
        let all = self.included() == self.table_count();
        for pick in self.entries.iter_mut().flat_map(|e| &mut e.picks) {
            pick.include = !all;
        }
    }

    /// What the export writes, once it has a folder and a table; `source_title` heads the column
    /// naming each row's table when stacking.
    pub fn plan(&self, source_title: &str) -> Option<Plan> {
        let folder = self.export.folder.clone()?;
        let items: Vec<Item> = self
            .entries
            .iter()
            .flat_map(|e| e.source.tables.iter().zip(&e.picks))
            .filter(|(_, pick)| pick.include)
            .map(|(table, pick)| Item { table: table.clone(), name: pick.name.clone() })
            .collect();
        if items.is_empty() {
            return None;
        }
        let file_name = match self.export.file_name.trim() {
            "" => "Splicr".to_owned(),
            name => name.to_owned(),
        };
        Some(Plan {
            mode: self.export.mode,
            // A workbook is xlsx whatever was picked for the others.
            format: if self.export.mode == Mode::Workbook { OutputFormat::Xlsx } else { self.export.format },
            items,
            folder,
            file_name,
            source_column: (self.export.mode == Mode::Stack && self.export.source_column)
                .then(|| source_title.to_owned()),
        })
    }
}

/// A file's name without its extension.
fn file_stem(source: &Source) -> String {
    let name = source.file_name();
    Path::new(&name).file_stem().map_or(name.clone(), |s| s.to_string_lossy().into_owned())
}

/// A table's name in an export: the file's for a CSV or a workbook of one sheet, the sheet's when
/// a workbook has several.
fn default_name(source: &Source, table: &Table) -> String {
    if source.tables.len() > 1 { table.name.clone() } else { file_stem(source) }
}

#[cfg(test)]
mod tests {
    use splicr_core::{Format, Value};

    use super::*;

    fn source(path: &str, sheets: &[&str]) -> Source {
        let tables = sheets
            .iter()
            .map(|name| {
                let mut t = Table::new(*name);
                t.push_row(vec![Value::text("id")]);
                Arc::new(t)
            })
            .collect();
        let format = if sheets.len() > 1 { Format::Xlsx } else { Format::Csv };
        Source { path: PathBuf::from(path), entry: None, format, tables }
    }

    #[test]
    fn tables_are_named_after_their_file_or_sheet() {
        let mut ws = Workspace::default();
        let first = ws.add(vec![source("/d/enero.csv", &["enero"])]);
        ws.add(vec![source("/d/libro.xlsx", &["Ventas", "Gastos"])]);
        assert_eq!(first, Some(TableKey { source: 1, index: 0 }));
        let names: Vec<&str> = ws.entries.iter().flat_map(|e| &e.picks).map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["enero", "Ventas", "Gastos"]);
        assert_eq!(ws.export.file_name, "enero");
        assert_eq!(ws.table_count(), 3);
    }

    #[test]
    fn opening_a_file_again_replaces_it_and_keeps_the_picks() {
        let mut ws = Workspace::default();
        ws.add(vec![source("/d/libro.xlsx", &["Ventas", "Gastos"])]);
        let gastos = TableKey { source: 1, index: 1 };
        *ws.pick_mut(gastos).unwrap() = Pick { include: false, name: "Costos".into() };
        ws.add(vec![source("/d/libro.xlsx", &["Gastos", "Nueva"])]);
        assert_eq!(ws.entries.len(), 1);
        assert_eq!(ws.entries[0].id, 1);
        assert_eq!(ws.entries[0].picks[0], Pick { include: false, name: "Costos".into() });
        assert_eq!(ws.entries[0].picks[1].name, "Nueva");
    }

    #[test]
    fn the_plan_takes_the_included_tables() {
        let mut ws = Workspace::default();
        ws.add(vec![source("/d/a.csv", &["a"]), source("/d/b.csv", &["b"])]);
        assert!(ws.plan("Origen").is_none(), "no folder yet");
        ws.export.folder = Some(PathBuf::from("/out"));
        ws.pick_mut(TableKey { source: 1, index: 0 }).unwrap().include = false;
        let plan = ws.plan("Origen").unwrap();
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].name, "b");
        assert_eq!(plan.format, OutputFormat::Xlsx);
        assert_eq!(plan.source_column, None, "only when stacking");

        ws.export.mode = Mode::Stack;
        ws.export.format = OutputFormat::Csv;
        let plan = ws.plan("Origen").unwrap();
        assert_eq!((plan.format, plan.source_column.as_deref()), (OutputFormat::Csv, Some("Origen")));

        ws.toggle_all();
        assert_eq!(ws.included(), 2);
        ws.toggle_all();
        assert!(ws.plan("Origen").is_none(), "nothing taken");
    }
}
