//! Workbooks (xlsx, xlsm, xlsb, xls, ods) through calamine: one table per sheet.

use std::io::Cursor;
use std::sync::Arc;

use calamine::{Data, Ods, Range, Reader, Xls, Xlsb, Xlsx, open_workbook_from_rs};
use splicr_core::{Format, MAX_COLS, MAX_ROWS, Table, Value};

use crate::{Error, Warning};

/// Every sheet of the workbook in `bytes`, already known to be `format`. A sheet that can't be read
/// is left out with a warning; a workbook with none readable is an error.
pub fn read(format: Format, bytes: &[u8], warnings: &mut Vec<Warning>) -> Result<Vec<Arc<Table>>, Error> {
    let cursor = Cursor::new(bytes);
    let unreadable = |e: &dyn std::fmt::Display| Error::Unreadable(e.to_string());
    match format {
        Format::Xlsx => each_sheet(open_workbook_from_rs::<Xlsx<_>, _>(cursor).map_err(|e| unreadable(&e))?, warnings),
        Format::Xlsb => each_sheet(open_workbook_from_rs::<Xlsb<_>, _>(cursor).map_err(|e| unreadable(&e))?, warnings),
        Format::Xls => each_sheet(open_workbook_from_rs::<Xls<_>, _>(cursor).map_err(|e| unreadable(&e))?, warnings),
        Format::Ods => each_sheet(open_workbook_from_rs::<Ods<_>, _>(cursor).map_err(|e| unreadable(&e))?, warnings),
        Format::Csv => Err(Error::Unreadable("not a workbook".into())),
    }
}

fn each_sheet<R, RS>(mut workbook: R, warnings: &mut Vec<Warning>) -> Result<Vec<Arc<Table>>, Error>
where
    R: Reader<RS>,
    RS: std::io::Read + std::io::Seek,
    R::Error: std::fmt::Display,
{
    let mut tables = Vec::new();
    let mut first_error = None;
    for name in workbook.sheet_names() {
        match workbook.worksheet_range(&name) {
            Ok(range) => tables.push(Arc::new(table(&name, &range, warnings))),
            Err(e) => {
                let error = e.to_string();
                first_error.get_or_insert_with(|| error.clone());
                warnings.push(Warning::SheetSkipped { sheet: name, error });
            }
        }
    }
    match (tables.is_empty(), first_error) {
        (true, Some(error)) => Err(Error::Unreadable(error)),
        _ => Ok(tables),
    }
}

/// The sheet's cells where the file has them: a range that starts at C5 keeps two empty columns and
/// four empty rows before it, as the sheet shows them.
fn table(name: &str, range: &Range<Data>, warnings: &mut Vec<Warning>) -> Table {
    let (top, left) = range.start().map_or((0, 0), |(r, c)| (r as usize, c as usize));
    let mut table = Table::new(name);
    let mut cols_cut = false;
    for _ in 0..top.min(MAX_ROWS) {
        table.push_row(Vec::new());
    }
    for row in range.rows() {
        if table.rows() >= MAX_ROWS {
            warnings.push(Warning::RowsCut { table: name.to_owned(), kept: MAX_ROWS });
            break;
        }
        let room = MAX_COLS.saturating_sub(left);
        cols_cut |= row.len() > room;
        let cells = std::iter::repeat_n(Value::Empty, left.min(MAX_COLS)).chain(row.iter().take(room).map(value));
        table.push_row(cells.collect());
    }
    if cols_cut {
        warnings.push(Warning::ColumnsCut { table: name.to_owned(), kept: MAX_COLS });
    }
    table
}

fn value(data: &Data) -> Value {
    match data {
        Data::Empty => Value::Empty,
        Data::Int(i) => Value::Number(*i as f64),
        Data::Float(f) => Value::Number(*f),
        Data::String(s) => Value::text(s),
        Data::Bool(b) => Value::Bool(*b),
        Data::DateTime(dt) if dt.is_datetime() => Value::DateTime(dt.as_f64()),
        // A duration, in days.
        Data::DateTime(dt) => Value::Number(dt.as_f64()),
        // shortcut: ODS dates come as ISO text (`2026-11-04T00:00:00`); kept as text until the
        // column types of 0.2 can read them as dates.
        Data::DateTimeIso(s) | Data::DurationIso(s) => Value::text(s),
        Data::Error(e) => Value::Error(e.to_string().into()),
    }
}
