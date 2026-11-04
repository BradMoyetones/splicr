//! Names that spreadsheets and file systems accept, made from whatever the user typed or the file
//! was called. Nothing is refused: a bad name is mended, and two names that would collide are told
//! apart with ` (2)`, ` (3)`… as file managers do.

/// Excel's longest sheet name, in UTF-16 units.
const SHEET_NAME_MAX: usize = 31;
/// Long enough for any real name, short enough for Windows' 260-character paths.
const FILE_STEM_MAX: usize = 120;

/// `raw` as a sheet name Excel accepts: none of `[ ] : * ? / \`, no apostrophe at either end, not
/// empty, at most 31 characters. `History` is Excel's own, so it becomes `History_`.
pub fn sheet_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_matches('\'').trim();
    let name = if trimmed.is_empty() { "Sheet" } else { trimmed };
    let name = if name.eq_ignore_ascii_case("history") { "History_" } else { name };
    truncate_utf16(name, SHEET_NAME_MAX).trim_end().to_owned()
}

/// Sheet names for `raw` names, in order, each valid ([`sheet_name`]) and different from the
/// others regardless of case, as Excel compares them.
pub fn unique_sheet_names<'a>(raw: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut taken: Vec<String> = Vec::new();
    raw.into_iter()
        .map(|raw| {
            let base = sheet_name(raw);
            let name = unique(&base, &taken, |base, suffix| {
                let room = SHEET_NAME_MAX - suffix.encode_utf16().count();
                format!("{}{suffix}", truncate_utf16(base, room).trim_end())
            });
            taken.push(name.to_lowercase());
            name
        })
        .collect()
}

/// `raw` as a file name without its extension, valid on Windows, macOS and Linux: none of
/// `< > : " / \ | ? *` or control characters, no dot or space at the end, not one of Windows'
/// device names (`CON`, `NUL`, `COM1`…), not empty.
pub fn file_stem(raw: &str) -> String {
    let cleaned: String =
        raw.chars()
            .map(|c| {
                if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() {
                    '_'
                } else {
                    c
                }
            })
            .collect();
    let truncated: String = cleaned.trim().chars().take(FILE_STEM_MAX).collect();
    let name = truncated.trim_end_matches(['.', ' ']).trim_start();
    if name.is_empty() {
        return "export".to_owned();
    }
    let device = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (device.len() == 4
            && (device.starts_with("COM") || device.starts_with("LPT"))
            && device.as_bytes()[3].is_ascii_digit());
    if reserved { format!("{name}_") } else { name.to_owned() }
}

/// File names for `raw` stems, in order, each valid ([`file_stem`]), ending in `.{extension}` and
/// different from the others regardless of case (Windows and macOS ignore it).
pub fn unique_file_names<'a>(raw: impl IntoIterator<Item = &'a str>, extension: &str) -> Vec<String> {
    let mut taken: Vec<String> = Vec::new();
    raw.into_iter()
        .map(|raw| {
            let stem = unique(&file_stem(raw), &taken, |base, suffix| format!("{base}{suffix}"));
            taken.push(stem.to_lowercase());
            format!("{stem}.{extension}")
        })
        .collect()
}

/// `base`, or the first `join(base, " (n)")` from 2 up whose lowercase isn't `taken`.
fn unique(base: &str, taken: &[String], join: impl Fn(&str, &str) -> String) -> String {
    let free = |name: &str| !taken.contains(&name.to_lowercase());
    if free(base) {
        return base.to_owned();
    }
    (2..).map(|n| join(base, &format!(" ({n})"))).find(|name| free(name)).unwrap_or_default()
}

/// The longest start of `s` that fits in `max` UTF-16 units, cut between characters.
fn truncate_utf16(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > max {
            return &s[..i];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_names_are_mended() {
        assert_eq!(sheet_name("Ventas [2026]: Q1/Q2"), "Ventas _2026__ Q1_Q2");
        assert_eq!(sheet_name("  'quoted'  "), "quoted");
        assert_eq!(sheet_name(""), "Sheet");
        assert_eq!(sheet_name("history"), "History_");
        let long = "Reporte de ventas mensual por región 2026";
        assert_eq!(sheet_name(long), "Reporte de ventas mensual por r");
        assert_eq!(sheet_name(long).encode_utf16().count(), 31);
        // An emoji is two UTF-16 units, as Excel counts it.
        assert_eq!(sheet_name(&"😀".repeat(20)).encode_utf16().count(), 30);
    }

    #[test]
    fn sheet_names_are_told_apart() {
        let names = unique_sheet_names(["Datos", "datos", "DATOS", "a".repeat(40).as_str(), &"a".repeat(31)]);
        assert_eq!(names[..3], ["Datos", "datos (2)", "DATOS (3)"]);
        assert_eq!(names[3], "a".repeat(31));
        assert_eq!(names[4], format!("{} (2)", "a".repeat(27)));
        assert!(names.iter().all(|n| n.encode_utf16().count() <= 31));
    }

    #[test]
    fn file_stems_are_mended() {
        assert_eq!(file_stem("ventas: enero/febrero?"), "ventas_ enero_febrero_");
        assert_eq!(file_stem("report. "), "report");
        assert_eq!(file_stem("  "), "export");
        assert_eq!(file_stem("con"), "con_");
        assert_eq!(file_stem("COM1.backup"), "COM1.backup_");
        assert_eq!(file_stem("COMPANY"), "COMPANY");
        assert_eq!(file_stem(&"x".repeat(300)).len(), FILE_STEM_MAX);
    }

    #[test]
    fn file_names_are_told_apart() {
        let names = unique_file_names(["Enero", "enero", "Febrero"], "xlsx");
        assert_eq!(names, ["Enero.xlsx", "enero (2).xlsx", "Febrero.xlsx"]);
    }
}
