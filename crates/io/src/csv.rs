//! CSV files: RFC 4180 with the usual real-world leniency, in whatever encoding and delimiter the
//! file came in.
//!
//! Decoding (`decode_text` and its Windows-1256 evidence), delimiter sniffing (`sniff_delimiter`)
//! and the record parser (`parse_records`) are adapted from GridCraft's `crates/xlsx/src/csv.rs`
//! (<https://github.com/storytold/gridcraft>), Copyright (c) 2026 ArtCraft Team and the GridCraft
//! contributors, MIT OR Apache-2.0 (used here under MIT: `licenses/gridcraft-LICENSE-MIT.txt`).
//! Changed: records feed Splicr's `Table` instead of GridCraft's workbook; fields stay text unless
//! they are plain numbers ([`field`]); GridCraft's date inference is left out; the writer is new.

use std::io::Write;

use splicr_core::{MAX_COLS, MAX_ROWS, Table, Value};

use crate::Warning;

/// Windows-1252 code points for bytes 0x80–0x9F (others map to Latin-1).
const CP1252: [u32; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D,
    0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D,
    0x017E, 0x0178,
];

/// Windows-1256 code points for bytes 0x80–0xFF (unicode.org `CP1256.TXT`).
const CP1256: [u32; 128] = [
    0x20AC, 0x067E, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0679, 0x2039, 0x0152, 0x0686,
    0x0698, 0x0688, 0x06AF, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x06A9, 0x2122, 0x0691, 0x203A,
    0x0153, 0x200C, 0x200D, 0x06BA, 0x00A0, 0x060C, 0x00A2, 0x00A3, 0x00A4, 0x00A5, 0x00A6, 0x00A7, 0x00A8, 0x00A9,
    0x06BE, 0x00AB, 0x00AC, 0x00AD, 0x00AE, 0x00AF, 0x00B0, 0x00B1, 0x00B2, 0x00B3, 0x00B4, 0x00B5, 0x00B6, 0x00B7,
    0x00B8, 0x00B9, 0x061B, 0x00BB, 0x00BC, 0x00BD, 0x00BE, 0x061F, 0x06C1, 0x0621, 0x0622, 0x0623, 0x0624, 0x0625,
    0x0626, 0x0627, 0x0628, 0x0629, 0x062A, 0x062B, 0x062C, 0x062D, 0x062E, 0x062F, 0x0630, 0x0631, 0x0632, 0x0633,
    0x0634, 0x0635, 0x0636, 0x00D7, 0x0637, 0x0638, 0x0639, 0x063A, 0x0640, 0x0641, 0x0642, 0x0643, 0x00E0, 0x0644,
    0x00E2, 0x0645, 0x0646, 0x0647, 0x0648, 0x00E7, 0x00E8, 0x00E9, 0x00EA, 0x00EB, 0x0649, 0x064A, 0x00EE, 0x00EF,
    0x064B, 0x064C, 0x064D, 0x064E, 0x00F4, 0x064F, 0x0650, 0x00F7, 0x0651, 0x00F9, 0x0652, 0x00FB, 0x00FC, 0x200E,
    0x200F, 0x06D2,
];

/// Evidence that bytes which aren't UTF-8 are Windows-1256 Arabic rather than Windows-1252.
#[derive(Default)]
struct Cp1256Evidence {
    /// High bytes that decode to different characters in the two code pages, apart from invisible
    /// marks and ordinals after a digit.
    differing: usize,
    /// Of those, Arabic letters and Arabic punctuation (، ؛ ؟) in Windows-1256.
    arabic: usize,
    /// Arabic bytes next to an ASCII letter: Western accents sit inside Latin words, Arabic doesn't.
    arabic_touching_latin: usize,
    /// Accented Latin bytes the two code pages share, next to another high byte: Hebrew
    /// Windows-1255 letters land there, Arabic and Western text rarely do.
    shared_letters_in_runs: usize,
}

impl Cp1256Evidence {
    fn of(bytes: &[u8]) -> Self {
        let mut e = Self::default();
        for (i, &b) in bytes.iter().enumerate() {
            if b < 0x80 {
                continue;
            }
            let before = i.checked_sub(1).and_then(|j| bytes.get(j)).copied();
            let after = bytes.get(i + 1).copied();
            let next_to = |pred: fn(u8) -> bool| before.is_some_and(pred) || after.is_some_and(pred);
            if same_in_cp1252_and_cp1256(b) {
                if is_shared_latin_letter(b) && next_to(|x| x >= 0x80) {
                    e.shared_letters_in_runs += 1;
                }
                continue;
            }
            if is_invisible_mark(b) || is_ordinal_after_digit(b, before) {
                continue;
            }
            e.differing += 1;
            if is_cp1256_arabic(b) {
                e.arabic += 1;
                if next_to(|x| x.is_ascii_alphabetic()) {
                    e.arabic_touching_latin += 1;
                }
            }
        }
        e
    }

    /// GridCraft's thresholds, chosen against month, day and language names from 95 glibc locales
    /// (Western in Windows-1252, Arabic, Persian and Urdu in 1256, Hebrew and Yiddish in 1255) plus
    /// CSV samples.
    fn is_arabic(&self) -> bool {
        self.arabic >= 2
            && self.arabic * 10 >= self.differing * 9
            && self.arabic_touching_latin * 10 <= self.arabic * 3
            && self.shared_letters_in_runs * 20 <= self.arabic
    }
}

fn same_in_cp1252_and_cp1256(b: u8) -> bool {
    matches!(
        b,
        0x80 | 0x82..=0x89
            | 0x8B..=0x8C
            | 0x91..=0x97
            | 0x99
            | 0x9B..=0x9C
            | 0xA0
            | 0xA2..=0xA9
            | 0xAB..=0xB9
            | 0xBB..=0xBE
            | 0xD7
            | 0xE0
            | 0xE2
            | 0xE7..=0xEB
            | 0xEE..=0xEF
            | 0xF4
            | 0xF7
            | 0xF9
            | 0xFB..=0xFC
    )
}

/// ZWNJ, ZWJ, LRM and RLM: common inside Persian words and around Arabic cells, but no evidence
/// either way.
fn is_invisible_mark(b: u8) -> bool {
    matches!(b, 0x9D | 0x9E | 0xFD | 0xFE)
}

/// `º` and `ª` after a digit (`1º`, `2ª`) are Western ordinals, though Windows-1256 reads the bytes
/// as `؛` and `ھ`; either way they are no evidence.
fn is_ordinal_after_digit(b: u8, before: Option<u8>) -> bool {
    matches!(b, 0xAA | 0xBA) && before.is_some_and(|x| x.is_ascii_digit())
}

fn is_shared_latin_letter(b: u8) -> bool {
    matches!(b, 0xE0 | 0xE2 | 0xE7..=0xEB | 0xEE..=0xEF | 0xF4 | 0xF9 | 0xFB..=0xFC)
}

fn is_cp1256_arabic(b: u8) -> bool {
    matches!(
        b,
        0x81 | 0x8A
            | 0x8D..=0x90
            | 0x98
            | 0x9A
            | 0x9F
            | 0xA1
            | 0xAA
            | 0xBA
            | 0xBF..=0xD6
            | 0xD8..=0xDB
            | 0xDD..=0xDF
            | 0xE1
            | 0xE3..=0xE6
            | 0xEC..=0xED
            | 0xFF
    )
}

/// Decodes CSV bytes with BOM-aware UTF-8 and UTF-16 handling. Without a BOM, valid UTF-8 is kept.
/// Other bytes decode as Windows-1256 when they read as Arabic (see [`Cp1256Evidence`]), else as
/// Windows-1252: what Excel writes on Spanish and other Western Windows systems.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    let utf16 = |b: &[u8], le: bool| -> String {
        let units =
            b.as_chunks::<2>().0.iter().map(|c| if le { u16::from_le_bytes(*c) } else { u16::from_be_bytes(*c) });
        char::decode_utf16(units).map(|r| r.unwrap_or('\u{FFFD}')).collect()
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => {
            let use_windows_1256 = Cp1256Evidence::of(bytes).is_arabic();
            let table = |table: &[u32], index: u8| {
                char::from_u32(table.get(usize::from(index)).copied().unwrap_or(0xFFFD)).unwrap_or('\u{FFFD}')
            };
            bytes
                .iter()
                .map(|&b| match b {
                    0x80..=0xFF if use_windows_1256 => table(&CP1256, b - 0x80),
                    0x80..=0x9F => table(&CP1252, b - 0x80),
                    _ => b as char,
                })
                .collect()
        }
    }
}

/// Picks comma, semicolon or tab: the candidate with the most consistent count per line (outside
/// quotes) over the first non-blank lines. A candidate must appear on every such line, otherwise
/// comma wins.
pub fn sniff_delimiter(text: &str, quote: char) -> char {
    let mut best = (',', 0usize, 0usize);
    for d in [',', ';', '\t'] {
        let mut counts = vec![];
        let mut n = 0usize;
        let mut in_q = false;
        let mut blank = true;
        for ch in text.chars().take(64 * 1024) {
            if !in_q && ch == '\n' {
                // Blank lines carry no delimiter evidence.
                if !blank {
                    counts.push(n);
                }
                n = 0;
                blank = true;
                if counts.len() >= 20 {
                    break;
                }
                continue;
            }
            if ch != '\r' {
                blank = false;
            }
            if ch == quote {
                in_q = !in_q;
            } else if !in_q && ch == d {
                n += 1;
            }
        }
        if !blank {
            counts.push(n);
        }
        let min = counts.iter().copied().min().unwrap_or(0);
        let total: usize = counts.iter().sum();
        if min > 0 && (min, total) > (best.1, best.2) {
            best = (d, min, total);
        }
    }
    best.0
}

/// Splits CSV text into records of fields. A record keeps at most [`MAX_COLS`] fields; the rest are
/// dropped as they are read, so a row of endless delimiters cannot allocate a string per field.
/// `on_record` returns `false` to stop.
fn parse_records(text: &str, delim: char, quote: char, mut on_record: impl FnMut(Vec<String>, bool) -> bool) {
    let mut field = String::new();
    let mut record: Vec<String> = vec![];
    let mut cut = false;
    let mut in_q = false;
    let mut chars = text.chars().peekable();
    let mut field_started_quoted = false;
    let push = |record: &mut Vec<String>, cut: &mut bool, field: String| {
        if record.len() < MAX_COLS {
            record.push(field);
        } else {
            *cut = true;
        }
    };
    while let Some(c) = chars.next() {
        if in_q {
            if c == quote {
                if chars.peek() == Some(&quote) {
                    field.push(quote);
                    chars.next();
                } else {
                    in_q = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        if c == quote && field.is_empty() && !field_started_quoted {
            in_q = true;
            field_started_quoted = true;
        } else if c == delim {
            push(&mut record, &mut cut, std::mem::take(&mut field));
            field_started_quoted = false;
        } else if c == '\r' || c == '\n' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            push(&mut record, &mut cut, std::mem::take(&mut field));
            field_started_quoted = false;
            if !on_record(std::mem::take(&mut record), std::mem::take(&mut cut)) {
                return;
            }
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !record.is_empty() || field_started_quoted {
        push(&mut record, &mut cut, field);
        on_record(record, cut);
    }
}

/// A field's value: a number when it is written as one plainly enough that nothing is lost by
/// reading it so, text otherwise. `007`, `+57 300…`, `1.234,5`, `1e5` and IDs longer than 15
/// digits (beyond what a spreadsheet number holds exactly) stay text.
pub fn field(text: &str) -> Value {
    if plain_number(text)
        && let Ok(n) = text.parse::<f64>()
    {
        return Value::Number(n);
    }
    Value::text(text)
}

fn plain_number(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    let (whole, fraction) = match digits.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (digits, None),
    };
    let all_digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    let whole_ok = all_digits(whole) && (whole == "0" || !whole.starts_with('0'));
    let fraction_ok = fraction.is_none_or(all_digits);
    let significant = whole.trim_start_matches('0').len() + fraction.map_or(0, str::len);
    whole_ok && fraction_ok && significant <= 15
}

/// The table in CSV `bytes`, named `name`. Rows past a sheet's limit are left out, with a warning.
pub fn read(bytes: &[u8], name: &str, warnings: &mut Vec<Warning>) -> Table {
    let text = decode_text(bytes);
    let quote = '"';
    let delim = sniff_delimiter(&text, quote);
    let mut table = Table::new(name);
    let mut cols_cut = false;
    let mut rows_cut = false;
    parse_records(&text, delim, quote, |record, cut| {
        if table.rows() >= MAX_ROWS {
            rows_cut = true;
            return false;
        }
        cols_cut |= cut;
        table.push_row(record.iter().map(|f| field(f)).collect());
        true
    });
    if rows_cut {
        warnings.push(Warning::RowsCut { table: name.to_owned(), kept: MAX_ROWS });
    }
    if cols_cut {
        warnings.push(Warning::ColumnsCut { table: name.to_owned(), kept: MAX_COLS });
    }
    table
}

/// Writes `table` as CSV: UTF-8 with a byte order mark (without it, Excel reads the accents
/// wrong), comma-separated, CRLF line ends, fields quoted when they need it. `on_row` is called
/// after each row and returns `false` to stop.
pub fn write(table: &Table, out: &mut impl Write, mut on_row: impl FnMut() -> bool) -> std::io::Result<bool> {
    out.write_all(&[0xEF, 0xBB, 0xBF])?;
    let mut line = String::new();
    for r in 0..table.rows() {
        line.clear();
        for c in 0..table.cols() {
            if c > 0 {
                line.push(',');
            }
            let text = table.get(r, c).display();
            let needs_quotes = text.contains([',', '"', '\r', '\n'])
                || text.starts_with(char::is_whitespace)
                || text.ends_with(char::is_whitespace);
            if needs_quotes {
                line.push('"');
                line.push_str(&text.replace('"', "\"\""));
                line.push('"');
            } else {
                line.push_str(&text);
            }
        }
        line.push_str("\r\n");
        out.write_all(line.as_bytes())?;
        if !on_row() {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(t: &Table) -> Vec<Vec<String>> {
        (0..t.rows()).map(|r| (0..t.cols()).map(|c| t.get(r, c).display().into_owned()).collect()).collect()
    }

    #[test]
    fn reads_quotes_and_line_breaks() {
        let mut w = vec![];
        let t = read(b"id,note\r\n1,\"a, b\"\r\n2,\"say \"\"hi\"\"\nthere\"\r\n", "t", &mut w);
        assert_eq!(cells(&t), [["id", "note"], ["1", "a, b"], ["2", "say \"hi\"\nthere"]]);
        assert!(w.is_empty());
    }

    #[test]
    fn guesses_the_delimiter() {
        let mut w = vec![];
        let t = read(b"nombre;edad\nAna;30\nLuis;41\n", "t", &mut w);
        assert_eq!(cells(&t), [["nombre", "edad"], ["Ana", "30"], ["Luis", "41"]]);
        assert_eq!(sniff_delimiter("a\tb\n1\t2\n", '"'), '\t');
    }

    #[test]
    fn reads_excels_windows_1252() {
        // "Año;Señal" as Excel saves it on a Spanish Windows.
        let bytes = b"A\xF1o;Se\xF1al\n2026;\x80\n";
        let mut w = vec![];
        assert_eq!(cells(&read(bytes, "t", &mut w)), [["Año", "Señal"], ["2026", "€"]]);
        assert_eq!(decode_text(&[0xEF, 0xBB, 0xBF, b'h', 0xC3, 0xA9]), "hé");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'h', 0, 0xE9, 0]), "hé");
    }

    #[test]
    fn only_plain_numbers_become_numbers() {
        for number in ["0", "42", "-3.5", "0.25", "123456789012345"] {
            assert!(matches!(field(number), Value::Number(_)), "{number}");
        }
        for text in ["007", "+57", "1.234,5", "1e5", "1234567890123456", " 4", "1.", ".5", "-", "12a"] {
            assert!(matches!(field(text), Value::Text(_)), "{text}");
        }
    }

    #[test]
    fn writes_what_it_reads() {
        let mut w = vec![];
        let t = read(b"a,b\n\"x, y\",\" padded\"\n\"q\"\"q\",007\n", "t", &mut w);
        let mut out = vec![];
        assert!(write(&t, &mut out, || true).unwrap());
        assert_eq!(&out[..3], &[0xEF, 0xBB, 0xBF]);
        assert_eq!(cells(&read(&out, "t", &mut w)), cells(&t));
        assert_eq!(String::from_utf8_lossy(&out[3..]), "a,b\r\n\"x, y\",\" padded\"\r\n\"q\"\"q\",007\r\n");
    }

    #[test]
    fn stops_when_asked() {
        let mut w = vec![];
        let t = read(b"a\nb\nc\n", "t", &mut w);
        let mut rows = 0;
        assert!(
            !write(&t, &mut Vec::new(), || {
                rows += 1;
                rows < 2
            })
            .unwrap()
        );
        assert_eq!(rows, 2);
    }
}
