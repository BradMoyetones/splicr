//! Zip files: a workbook (xlsx, xlsb and ods are zips) or an archive of CSV and spreadsheet files,
//! each read as a source of its own. Archives inside archives are not opened.

use std::io::{Cursor, Read};
use std::path::Path;

use splicr_core::{Format, Source};
use zip::ZipArchive;

use crate::{Error, Kind, MAX_ENTRIES, MAX_FILE_BYTES, Opened, Warning};

/// A workbook by the part every one of its kind has, else an archive of files.
pub fn sniff(bytes: &[u8]) -> Kind {
    let Ok(zip) = ZipArchive::new(Cursor::new(bytes)) else { return Kind::Archive };
    let has = |name: &str| zip.index_for_name(name).is_some();
    if has("xl/workbook.xml") {
        Kind::Table(Format::Xlsx)
    } else if has("xl/workbook.bin") {
        Kind::Table(Format::Xlsb)
    } else if has("content.xml") && has("mimetype") {
        Kind::Table(Format::Ods)
    } else {
        Kind::Archive
    }
}

/// The format of an archive's file, by its name; `None` for files that aren't tables.
fn format_of(name: &str) -> Option<Format> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "csv" | "tsv" | "txt" => Format::Csv,
        "xlsx" | "xlsm" => Format::Xlsx,
        "xlsb" => Format::Xlsb,
        "xls" => Format::Xls,
        "ods" => Format::Ods,
        _ => return None,
    })
}

/// Reads every table file in the archive at `path`, in the archive's order. macOS's `__MACOSX`
/// shadow files and hidden files are passed over silently; other files are named in a warning.
pub fn read(path: &Path, bytes: &[u8], opened: &mut Opened) -> Result<(), Error> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| Error::Unreadable(e.to_string()))?;
    let mut skipped = Vec::new();
    for index in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(index) else { continue };
        // A name that isn't valid UTF-8 (or tries to leave the archive) is shown as it can be.
        let name =
            entry.name().map_or_else(|_| String::from_utf8_lossy(entry.name_raw()).into_owned(), |n| n.into_owned());
        let base = name.rsplit('/').next().unwrap_or(&name);
        if entry.is_dir() || name.starts_with("__MACOSX/") || base.starts_with('.') {
            continue;
        }
        let Some(format) = format_of(base) else {
            skipped.push(name);
            continue;
        };
        if opened.sources.len() >= MAX_ENTRIES || entry.size() > MAX_FILE_BYTES {
            skipped.push(name);
            continue;
        }
        let mut data = Vec::new();
        // The declared size can lie: the read is capped too.
        let read = (&mut entry).take(MAX_FILE_BYTES + 1).read_to_end(&mut data);
        if read.is_err() || data.len() as u64 > MAX_FILE_BYTES {
            skipped.push(name);
            continue;
        }
        let stem = base.rsplit_once('.').map_or(base, |(stem, _)| stem);
        // The content has the last word: a `.xls` that is really a CSV reads as one.
        let format = match crate::sniff(&data) {
            Kind::Table(sniffed) if format != Format::Csv => sniffed,
            _ => format,
        };
        match crate::read_tables(format, &data, stem, &mut opened.warnings) {
            Ok(tables) => opened.sources.push(Source { path: path.to_owned(), entry: Some(name), format, tables }),
            Err(_) => skipped.push(name),
        }
    }
    if !skipped.is_empty() {
        opened.warnings.push(Warning::EntriesSkipped { names: skipped });
    }
    if opened.sources.is_empty() {
        return Err(Error::NothingInside);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in files {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_the_tables_in_an_archive() {
        let bytes = archive(&[
            ("ventas/enero.csv", b"id,total\n1,10\n"),
            ("__MACOSX/ventas/._enero.csv", b"junk"),
            ("ventas/.DS_Store", b"junk"),
            ("leeme.pdf", b"%PDF"),
            ("ventas/febrero.CSV", b"id;total\n2;20\n"),
        ]);
        assert_eq!(sniff(&bytes), Kind::Archive);
        let mut opened = Opened::default();
        read(Path::new("/d/2026.zip"), &bytes, &mut opened).unwrap();
        let names: Vec<String> = opened.sources.iter().map(Source::file_name).collect();
        assert_eq!(names, ["enero.csv", "febrero.CSV"]);
        assert_eq!(opened.sources[1].tables[0].name, "febrero");
        assert_eq!(opened.warnings, [Warning::EntriesSkipped { names: vec!["leeme.pdf".into()] }]);
    }

    #[test]
    fn an_archive_without_tables_says_so() {
        let bytes = archive(&[("foto.png", b"\x89PNG")]);
        let mut opened = Opened::default();
        assert_eq!(read(Path::new("/d/x.zip"), &bytes, &mut opened), Err(Error::NothingInside));
    }
}
