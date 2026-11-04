//! Splicr's data, apart from files and screens: values, tables, the names spreadsheets accept and
//! what an export makes. Pure: no I/O, no system or UI crates, no `unsafe`.

#![forbid(unsafe_code)]

pub mod names;
pub mod plan;
mod table;
mod value;

pub use table::{Format, Source, Table, column_letters};
pub use value::{Value, date_time};

/// Excel's limits: a sheet holds at most this many rows and columns.
pub const MAX_ROWS: usize = 1_048_576;
pub const MAX_COLS: usize = 16_384;
