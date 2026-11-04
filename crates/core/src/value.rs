//! One cell's value, and how it reads on screen.

use std::borrow::Cow;
use std::sync::Arc;

/// What a cell holds, as the file had it. Text stays text: a CSV's `007` is never turned into 7.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Value {
    #[default]
    Empty,
    Text(Arc<str>),
    Number(f64),
    Bool(bool),
    /// A date and time as spreadsheets store it: days since 1899-12-30, the fraction being the time
    /// of day. (Serials below 61 are a day off, as Excel's own 1900 leap-year bug makes them.)
    DateTime(f64),
    /// A formula's error as the file had it: `#N/A`, `#DIV/0!`…
    Error(Arc<str>),
}

impl Value {
    pub fn text(s: &str) -> Self {
        if s.is_empty() { Value::Empty } else { Value::Text(Arc::from(s)) }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Value::Empty)
    }

    /// Numbers and dates line up on the right, as spreadsheets show them.
    pub fn is_numeric(&self) -> bool {
        matches!(self, Value::Number(_) | Value::DateTime(_))
    }

    /// The value as it reads in a cell, and in a CSV file.
    pub fn display(&self) -> Cow<'_, str> {
        match self {
            Value::Empty => Cow::Borrowed(""),
            Value::Text(s) | Value::Error(s) => Cow::Borrowed(s),
            Value::Number(n) => Cow::Owned(number(*n)),
            Value::Bool(true) => Cow::Borrowed("TRUE"),
            Value::Bool(false) => Cow::Borrowed("FALSE"),
            Value::DateTime(serial) => Cow::Owned(date_time(*serial)),
        }
    }
}

/// Whole numbers without a decimal point, others in the shortest form that reads back the same.
fn number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}

/// `2026-11-04`, or `2026-11-04 14:30` (seconds only when there are some): ISO 8601, which every
/// language reads the same way and every spreadsheet reads back as a date.
pub fn date_time(serial: f64) -> String {
    if !serial.is_finite() || !(0.0..3_000_000.0).contains(&serial) {
        return number(serial);
    }
    let days = serial.floor();
    // Rounded to the second: 0.99999 of a day is 23:59:59.99, which reads as midnight otherwise.
    let mut seconds = ((serial - days) * 86_400.0).round() as i64;
    let mut days = days as i64;
    if seconds >= 86_400 {
        days += 1;
        seconds -= 86_400;
    }
    // 1899-12-30 is day 0; 1970-01-01 is day 25569.
    let (y, m, d) = civil_date(days - 25_569);
    let date = format!("{y:04}-{m:02}-{d:02}");
    match seconds {
        0 => date,
        s if s % 60 == 0 => format!("{date} {:02}:{:02}", s / 3600, s / 60 % 60),
        s => format!("{date} {:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60),
    }
}

/// Year, month and day of `days` since 1970-01-01, in the proleptic Gregorian calendar (Howard
/// Hinnant's `civil_from_days`).
fn civil_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_as_spreadsheets_show_them() {
        assert_eq!(Value::Number(42.0).display(), "42");
        assert_eq!(Value::Number(-3.5).display(), "-3.5");
        assert_eq!(Value::Number(0.1 + 0.2).display(), "0.30000000000000004");
        assert_eq!(Value::Bool(true).display(), "TRUE");
        assert_eq!(Value::text("007").display(), "007");
        assert_eq!(Value::text(""), Value::Empty);
    }

    #[test]
    fn dates_read_as_iso() {
        assert_eq!(date_time(45_000.0), "2023-03-15");
        assert_eq!(date_time(25_569.5), "1970-01-01 12:00");
        assert_eq!(date_time(45_000.0 + 1.0 / 86_400.0), "2023-03-15 00:00:01");
        // A hair before midnight rounds to the next day, not to 24:00.
        assert_eq!(date_time(45_000.999_999_9), "2023-03-16");
        assert_eq!(date_time(-1.0), "-1");
    }
}
