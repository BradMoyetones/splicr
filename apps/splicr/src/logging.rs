//! Splicr's logger: `log` records from every crate go to standard error and to
//! `<settings folder>/logs/splicr.log`. A release build has no console, so the file is what a bug
//! report can attach. Each start moves the previous log to `splicr.1.log` (and that one to
//! `splicr.2.log`), so the log of a run that crashed survives the next start.
//!
//! Levels: `info` for Splicr's own crates, `warn` for the rest. `RUST_LOG` replaces that with
//! directives: `debug`, `warn,splicr_engine=debug`; a name ending in `*` matches every target that
//! starts with it (`splicr*=debug`).
//!
//! Logging never panics: a file that can't be made or written leaves standard error alone. Nothing
//! logs from a keyboard hook's callback (see CLAUDE.md).
//!
//! After ArtCraft's vectorcraft logger (`apps/vectorcraft/src/logging.rs`, Apache-2.0): the same
//! rotation, file cap and directives, written for Splicr.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Log, Metadata, Record};

const FILE: &str = "splicr.log";
/// How many earlier logs are kept: `splicr.1.log` … `splicr.<KEEP>.log`.
const KEEP: usize = 2;
/// The file stops growing past this: a runaway warning can't fill the disk.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// The filter when `RUST_LOG` is unset or empty.
const DEFAULT_FILTER: &str = "warn,splicr*=info";

/// Installs the logger, writing into `dir/logs`. Call once, early in `main`.
pub fn init(dir: &Path) {
    let spec = std::env::var("RUST_LOG").ok().filter(|s| !s.trim().is_empty());
    let filter = Filter::parse(spec.as_deref().unwrap_or(DEFAULT_FILTER));
    let file = rotate(&dir.join("logs")).ok().map(|file| Sink { file, written: 0 });
    log::set_max_level(filter.max());
    let _ = log::set_boxed_logger(Box::new(Logger { filter, file: Mutex::new(file) }));
}

struct Sink {
    file: File,
    written: u64,
}

struct Logger {
    filter: Filter,
    file: Mutex<Option<Sink>>,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.filter.level_for(metadata.target())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let thread = std::thread::current();
        let line = format_line(
            &timestamp(SystemTime::now()),
            record.level(),
            thread.name().unwrap_or("?"),
            record.target(),
            &record.args().to_string(),
        );
        let _ = io::stderr().write_all(line.as_bytes());
        // A poisoned lock means a write panicked; the file is still there to try again.
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sink) = file.as_mut().filter(|s| s.written < MAX_FILE_BYTES)
            && sink.file.write_all(line.as_bytes()).is_ok()
        {
            sink.written += line.len() as u64;
        }
    }

    fn flush(&self) {
        if let Some(sink) = self.file.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = sink.file.flush();
        }
    }
}

/// Which records pass, per target, from directives like `warn,splicr_engine=debug`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Filter {
    default: LevelFilter,
    directives: Vec<(String, LevelFilter)>,
}

impl Filter {
    /// Unknown levels and empty parts are skipped. With no bare level, other targets log errors.
    fn parse(spec: &str) -> Self {
        let mut filter = Self { default: LevelFilter::Error, directives: Vec::new() };
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('=') {
                Some((name, level)) => {
                    if let (false, Ok(level)) = (name.trim().is_empty(), level.trim().parse()) {
                        filter.directives.push((name.trim().to_owned(), level));
                    }
                }
                None => match part.parse() {
                    Ok(level) => filter.default = level,
                    // A bare name: everything from that target.
                    Err(_) => filter.directives.push((part.to_owned(), LevelFilter::Trace)),
                },
            }
        }
        filter
    }

    /// The level for `target`: the most specific directive that names it, or the default.
    fn level_for(&self, target: &str) -> LevelFilter {
        let named = self.directives.iter().filter(|(name, _)| matches(name, target));
        named.max_by_key(|(name, _)| name.len()).map_or(self.default, |&(_, level)| level)
    }

    /// The most a target can log, for `log::set_max_level`.
    fn max(&self) -> LevelFilter {
        self.directives.iter().map(|&(_, level)| level).fold(self.default, Ord::max)
    }
}

/// `name` matches its target and the target's modules (`a` matches `a` and `a::b`, not `ab`);
/// `name*` every target that starts with `name`.
fn matches(name: &str, target: &str) -> bool {
    match name.strip_suffix('*') {
        Some(prefix) => target.starts_with(prefix),
        None => target.strip_prefix(name).is_some_and(|rest| rest.is_empty() || rest.starts_with("::")),
    }
}

/// Moves `splicr.log` to `splicr.1.log`, that to `.2` (dropping the oldest), and starts a fresh
/// `splicr.log` in `dir`, made if missing.
fn rotate(dir: &Path) -> io::Result<File> {
    let numbered = |n: usize| -> PathBuf { if n == 0 { dir.join(FILE) } else { dir.join(format!("splicr.{n}.log")) } };
    std::fs::create_dir_all(dir)?;
    for n in (1..=KEEP).rev() {
        match std::fs::rename(numbered(n - 1), numbered(n)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    File::create(numbered(0))
}

/// `<time> <LEVEL> [<thread>] <target>: <message>`, one line.
fn format_line(time: &str, level: Level, thread: &str, target: &str, message: &str) -> String {
    format!("{time} {level:<5} [{thread}] {target}: {}\n", message.trim_end_matches('\n'))
}

/// `2026-10-30T15:16:24.123Z`, in UTC. A clock before 1970 reads as 1970.
fn timestamp(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = i64::try_from(since.as_secs()).unwrap_or(i64::MAX);
    let (days, of_day) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_date(days);
    let (h, min, s) = (of_day / 3600, of_day % 3600 / 60, of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}.{:03}Z", since.subsec_millis())
}

/// The (year, month, day) `days` after 1970-01-01, in the proleptic Gregorian calendar (Howard
/// Hinnant's `civil_from_days`).
fn civil_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    (year_of_era + era * 400 + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn directives_pick_a_level_per_target() {
        let f = Filter::parse(DEFAULT_FILTER);
        assert_eq!(f.level_for("splicr"), LevelFilter::Info);
        assert_eq!(f.level_for("splicr_engine::windows::hook"), LevelFilter::Info);
        assert_eq!(f.level_for("winit"), LevelFilter::Warn);
        assert_eq!(f.max(), LevelFilter::Info);

        let f = Filter::parse("info, splicr_engine=debug, eframe=off, nonsense=loud, ,");
        assert_eq!(f.level_for("splicr_engine::windows"), LevelFilter::Debug);
        assert_eq!(f.level_for("splicr_engineering"), LevelFilter::Info);
        assert_eq!(f.level_for("eframe"), LevelFilter::Off);
        assert_eq!(f.level_for("nonsense"), LevelFilter::Info);
        assert_eq!(Filter::parse("splicr").level_for("splicr"), LevelFilter::Trace);
        assert_eq!(Filter::parse("").level_for("anything"), LevelFilter::Error);
    }

    #[test]
    fn timestamps_are_utc_dates() {
        let at = |secs: u64, millis: u64| timestamp(UNIX_EPOCH + Duration::from_millis(secs * 1000 + millis));
        assert_eq!(at(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(at(951_868_799, 5), "2000-02-29T23:59:59.005Z");
        assert_eq!(at(1_793_373_384, 728), "2026-10-30T15:16:24.728Z");
        assert_eq!(timestamp(UNIX_EPOCH - Duration::from_secs(10)), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn each_start_keeps_the_last_two_logs() {
        let dir = std::env::temp_dir().join(format!("splicr-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for run in 1..=4 {
            let mut file = rotate(&dir).unwrap();
            write!(file, "run {run}").unwrap();
        }
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(
            (read("splicr.log"), read("splicr.1.log"), read("splicr.2.log")),
            ("run 4".into(), "run 3".into(), "run 2".into())
        );
        assert!(!dir.join("splicr.3.log").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lines_have_time_level_thread_and_target() {
        let line = format_line("T", Level::Warn, "main", "splicr::config", "couldn't save\n");
        assert_eq!(line, "T WARN  [main] splicr::config: couldn't save\n");
    }
}
