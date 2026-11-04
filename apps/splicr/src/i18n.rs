//! Interface text in the user's language, the way ArtCraft's craft-i18n does it.
//!
//! The code holds the English text: `tr!("Add")`, `tr!("Removed {mapping}.", mapping = name)`,
//! `trn!("{n} table|{n} tables", count)`. Every other language is a catalog,
//! `locales/<code>.tsv`, embedded at build time; adding a language is adding its file to
//! [`CATALOGS`]. A catalog line is `context <TAB> English <TAB> translation`, as in craft-i18n:
//! context empty for most text, `@plural` for counts (English `one|other`, then
//! the language's own forms). A missing translation shows in English, never an error.
//!
//! The language is the first of the system's languages that Splicr speaks, matched by full tag
//! then by language alone (`es-CO` → `es`); English when none.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;

/// The languages Splicr speaks besides English, each with its catalog.
const CATALOGS: &[(&str, &str)] = &[("es", include_str!("../locales/es.tsv"))];

struct Language {
    /// context → English → translation.
    rows: HashMap<&'static str, HashMap<&'static str, &'static str>>,
}

/// The language in use, or `None` for English.
fn language() -> Option<&'static Language> {
    static LANGUAGE: OnceLock<Option<Language>> = OnceLock::new();
    LANGUAGE
        .get_or_init(|| {
            // Debug builds can be told which, to try another language: SPLICR_LANG=es.
            let forced = std::env::var("SPLICR_LANG").ok().filter(|_| cfg!(debug_assertions));
            let wanted = forced.map_or_else(splicr_platform::preferred_languages, |tag| vec![tag]);
            let code = pick(&wanted, CATALOGS.iter().map(|(code, _)| *code))?;
            let catalog = CATALOGS.iter().find(|(c, _)| *c == code).map(|(_, text)| *text)?;
            Some(Language { rows: parse(catalog) })
        })
        .as_ref()
}

/// The first wanted language (`es-CO`, `en-US`…) that is offered, by full tag then by language;
/// `None` when English comes first or nothing matches.
fn pick(wanted: &[String], offered: impl Iterator<Item = &'static str> + Clone) -> Option<&'static str> {
    for tag in wanted {
        let tag = tag.to_ascii_lowercase().replace('_', "-");
        let base = tag.split('-').next().unwrap_or_default();
        if base == "en" {
            return None;
        }
        if let Some(code) = offered.clone().find(|c| *c == tag).or_else(|| offered.clone().find(|c| *c == base)) {
            return Some(code);
        }
    }
    None
}

/// A catalog's rows. Lines that aren't three tab-separated columns are skipped (the test below
/// keeps them out of the files). Escapes aren't supported: Splicr's text has none.
fn parse(text: &'static str) -> HashMap<&'static str, HashMap<&'static str, &'static str>> {
    let mut rows: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    for line in text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if let [context, english, translation] = line.split('\t').collect::<Vec<_>>()[..] {
            rows.entry(context).or_default().entry(english).or_insert(translation);
        }
    }
    rows
}

/// `english` in the user's language, looked up under `context`.
pub fn lookup(context: &str, english: &'static str) -> &'static str {
    language().and_then(|l| l.rows.get(context)?.get(english).copied()).unwrap_or(english)
}

/// Fills `{name}` placeholders in one pass; a name not given stays as it is.
pub fn format(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}').map(|close| (&after[..close], close)) {
            Some((name, close)) if let Some((_, value)) = args.iter().find(|(n, _)| *n == name) => {
                out.push_str(&value.to_string());
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The form of `forms` (`|`-separated, in CLDR order) for the count `n`, with `{n}` filled.
pub fn plural(forms: &str, n: usize) -> String {
    let forms: Vec<&str> = forms.split('|').collect();
    // shortcut: every language Splicr speaks has CLDR's one/other rule (one is exactly 1). Add the
    // language's rule here with its catalog when one doesn't (French: 0 and 1; Polish: few, many).
    let index = usize::from(n != 1).min(forms.len() - 1);
    format(forms[index], &[("n", &n)])
}

/// Interface text: `tr!("Add")`, or with placeholders `tr!("Removed {name}.", name = x)`.
macro_rules! tr {
    ($english:literal) => {
        $crate::i18n::lookup("", $english)
    };
    ($english:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::format($crate::i18n::lookup("", $english), &[$((stringify!($name), &$value)),+])
    };
}

/// A count: `trn!("{n} table|{n} tables", count)`.
macro_rules! trn {
    ($english:literal, $n:expr) => {
        $crate::i18n::plural($crate::i18n::lookup("@plural", $english), $n)
    };
}

pub(crate) use {tr, trn};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_first_spoken_language() {
        let offered = ["es", "pt-br"].into_iter();
        let pick = |tags: &[&str]| pick(&tags.iter().map(|t| t.to_string()).collect::<Vec<_>>(), offered.clone());
        assert_eq!(pick(&["es-CO", "en-US"]), Some("es"));
        assert_eq!(pick(&["en-US", "es-ES"]), None);
        assert_eq!(pick(&["fr-FR", "pt-BR"]), Some("pt-br"));
        assert_eq!(pick(&["de-DE"]), None);
    }

    #[test]
    fn fills_placeholders_and_plurals() {
        assert_eq!(format("{a} → {b}, {c}", &[("a", &1), ("b", &"x")]), "1 → x, {c}");
        assert_eq!(plural("{n} tabla|{n} tablas", 1), "1 tabla");
        assert_eq!(plural("{n} tabla|{n} tablas", 3), "3 tablas");
    }

    /// Every text the code asks for has a translation in every catalog, with the same
    /// placeholders, and no catalog line is malformed: a missing one would quietly show English.
    #[test]
    fn catalogs_cover_the_code() {
        fn rust_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    rust_files(&path, files);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    files.push(path);
                }
            }
        }
        let mut files = Vec::new();
        rust_files(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut wanted = Vec::new();
        for path in files {
            // This module's docs show the macros with made-up text.
            if path.ends_with("i18n.rs") {
                continue;
            }
            let code = std::fs::read_to_string(path).unwrap();
            for (macro_name, context) in [("tr!(\"", ""), ("trn!(\"", "@plural")] {
                for (at, _) in code.match_indices(macro_name) {
                    // Not the end of a longer name: `include_str!("` isn't `tr!("`.
                    if code[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let part = &code[at + macro_name.len()..];
                    let english = &part[..part.find('"').unwrap()];
                    if !english.is_empty() {
                        wanted.push((context, english.to_owned()));
                    }
                }
            }
        }
        assert!(wanted.len() > 100, "the scan found only {} texts", wanted.len());
        let placeholders = |s: &str| {
            let mut names: Vec<String> =
                s.split('{').skip(1).filter_map(|p| p.split_once('}')).map(|(n, _)| n.to_owned()).collect();
            names.sort();
            names.dedup();
            names
        };
        for (code, text) in CATALOGS {
            for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.is_empty() && !l.starts_with('#')) {
                assert_eq!(line.split('\t').count(), 3, "{code}.tsv line {}: needs 3 tab-separated columns", i + 1);
            }
            let rows = parse(text);
            for (context, english) in &wanted {
                let translation = rows.get(context).and_then(|r| r.get(english.as_str()));
                let translation =
                    translation.unwrap_or_else(|| panic!("{code}.tsv has no {context:?} row for {english:?}"));
                assert_eq!(placeholders(english), placeholders(translation), "{code}.tsv: placeholders of {english:?}");
            }
        }
    }
}
