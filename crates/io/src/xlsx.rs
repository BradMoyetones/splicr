//! Writing xlsx through rust_xlsxwriter, a sheet's rows straight to disk (its constant-memory
//! mode), so a big export needn't fit in memory twice.

use std::path::Path;
use std::sync::Arc;

use rust_xlsxwriter::{Format, Workbook, XlsxError};
use splicr_core::{MAX_COLS, MAX_ROWS, Table, Value};

use crate::Error;

/// How many rows are read to size the columns.
const WIDTH_SAMPLE: usize = 200;

pub fn write(sheets: &[(String, Arc<Table>)], path: &Path, on_row: &mut dyn FnMut() -> bool) -> Result<(), Error> {
    let err = |e: XlsxError| Error::Write(e.to_string());
    let header = Format::new().set_bold();
    let date = Format::new().set_num_format("yyyy-mm-dd");
    let date_time = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss");
    let mut workbook = Workbook::new();
    for (name, table) in sheets {
        if table.rows() > MAX_ROWS {
            return Err(Error::TooManyRows { sheet: name.clone() });
        }
        if table.cols() > MAX_COLS {
            return Err(Error::TooManyColumns { sheet: name.clone() });
        }
        let sheet = workbook.add_worksheet_with_constant_memory().map_err(err)?;
        sheet.set_name(name).map_err(err)?;
        for (col, width) in widths(table).into_iter().enumerate() {
            sheet.set_column_width(col as u16, width).map_err(err)?;
        }
        if table.rows() > 1 {
            sheet.set_freeze_panes(1, 0).map_err(err)?;
        }
        for r in 0..table.rows() {
            let row = r as u32;
            for (c, value) in table.row(r).iter().enumerate() {
                let col = c as u16;
                let bold = (r == 0).then_some(&header);
                let written = match value {
                    Value::Empty => continue,
                    Value::Text(s) | Value::Error(s) => match bold {
                        Some(f) => sheet.write_string_with_format(row, col, &**s, f),
                        None => sheet.write_string(row, col, &**s),
                    },
                    Value::Number(n) if n.is_finite() => sheet.write_number(row, col, *n),
                    Value::Number(n) => sheet.write_string(row, col, n.to_string()),
                    Value::Bool(b) => sheet.write_boolean(row, col, *b),
                    Value::DateTime(serial) => {
                        let format = if serial.fract() == 0.0 { &date } else { &date_time };
                        sheet.write_number_with_format(row, col, *serial, format)
                    }
                };
                written.map_err(err)?;
            }
            if !on_row() {
                return Err(Error::Cancelled);
            }
        }
    }
    if sheets.is_empty() {
        workbook.add_worksheet();
    }
    workbook.save(path).map_err(err)
}

/// Each column's width in characters, from its longest value in the first rows, between 8 and 60.
fn widths(table: &Table) -> Vec<f64> {
    (0..table.cols())
        .map(|c| {
            let longest = (0..table.rows().min(WIDTH_SAMPLE))
                .map(|r| table.get(r, c).display().chars().count())
                .max()
                .unwrap_or(0);
            (longest as f64 + 2.0).clamp(8.0, 60.0)
        })
        .collect()
}
