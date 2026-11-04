//! The user's settings, in `config.toml` in Splicr's settings folder (`%APPDATA%\Splicr` on
//! Windows, `~/Library/Application Support/Splicr` on macOS, `~/.config/Splicr` on Linux).
//!
//! Files say which `version` of the format they are in, so a later change of the format can convert
//! the files before it, as Remapr does. Version 1 is the first.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::i18n::tr;

/// The format this Splicr writes.
pub const VERSION: i64 = 1;
/// How many recent files Home lists.
pub const RECENT: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The format's version, [`VERSION`]; first, so it opens the file.
    pub version: i64,
    pub theme: Theme,
    /// The sidebar shows icons only.
    pub compact_sidebar: bool,
    /// Files opened lately, newest first.
    pub recent: Vec<PathBuf>,
    /// Where the last export went.
    pub export_folder: Option<PathBuf>,
    /// The version that last ran, to say "Splicr se actualizó" once after an update.
    pub last_version: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            theme: Theme::System,
            compact_sidebar: false,
            recent: Vec::new(),
            export_folder: None,
            last_version: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Config {
    /// Puts `path` first among the recent files.
    pub fn remember(&mut self, path: PathBuf) {
        self.recent.retain(|p| *p != path);
        self.recent.insert(0, path);
        self.recent.truncate(RECENT);
    }
}

/// Splicr's own settings folder: `config.toml` and `logs/`.
pub fn dir() -> PathBuf {
    let base = splicr_platform::config_dir().unwrap_or_else(std::env::temp_dir);
    // Debug builds keep their own settings, so trying things never touches the installed copy's.
    base.join(if cfg!(debug_assertions) { "Splicr-dev" } else { "Splicr" })
}

fn path() -> PathBuf {
    dir().join("config.toml")
}

/// What a settings file turned out to be.
#[derive(Debug, PartialEq)]
enum Read {
    Current(Config),
    /// Written by a newer Splicr, in a format this one doesn't know.
    Newer,
}

fn read(text: &str) -> Result<Read, toml::de::Error> {
    let table: toml::Table = text.parse()?;
    let version = table.get("version").and_then(toml::Value::as_integer).unwrap_or(0);
    match version {
        VERSION => Ok(Read::Current(table.try_into()?)),
        v if v > VERSION => Ok(Read::Newer),
        v => Err(serde::de::Error::custom(format!("unknown settings version {v}"))),
    }
}

impl Config {
    /// The saved config, or defaults, with a message for the user when something needs saying.
    ///
    /// A file that can't be read is set aside as `config.toml.bad`, never overwritten. A file from a
    /// newer Splicr is copied to `config.newer.toml` before this one starts with defaults.
    pub fn load() -> (Self, Option<String>) {
        let path = path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => {
                log::error!("couldn't read {}: {e}", path.display());
                return (Self::default(), Some(tr!("Couldn't read the settings: {error}", error = e)));
            }
        };
        match read(&text) {
            Ok(Read::Current(config)) => (config, None),
            Ok(Read::Newer) => {
                log::warn!("{} is from a newer Splicr", path.display());
                let newer = path.with_file_name("config.newer.toml");
                let _ = std::fs::copy(&path, &newer);
                let msg =
                    tr!("The settings are from a newer Splicr. A copy was kept as {file}", file = newer.display());
                (Self::default(), Some(msg))
            }
            Err(e) => {
                log::error!("{} is unreadable: {e}", path.display());
                let bad = path.with_extension("toml.bad");
                let _ = std::fs::rename(&path, &bad);
                let msg = tr!("The settings were damaged. A copy was saved as {file}", file = bad.display());
                (Self::default(), Some(msg))
            }
        }
    }

    /// Writes beside the file, then swaps it in, so a crash mid-save never leaves a broken config.
    pub fn save(&self) -> io::Result<()> {
        let path = path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(io::Error::other)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let mut config = Config { theme: Theme::Dark, compact_sidebar: true, ..Config::default() };
        config.remember("/a.csv".into());
        config.remember("/b.xlsx".into());
        config.remember("/a.csv".into());
        assert_eq!(config.recent, [PathBuf::from("/a.csv"), PathBuf::from("/b.xlsx")]);
        let text = toml::to_string_pretty(&config).unwrap();
        assert!(text.starts_with("version = 1"), "{text}");
        assert_eq!(read(&text).unwrap(), Read::Current(config));
    }

    #[test]
    fn other_versions_are_not_read_as_this_one() {
        assert_eq!(read("version = 99\ntheme = \"dark\"").unwrap(), Read::Newer);
        assert!(read("theme = \"dark\"").is_err());
        // Unknown keys from a later 1.x are ignored, missing ones take their defaults.
        let Read::Current(config) = read("version = 1\nfuture = true").unwrap() else { panic!() };
        assert_eq!(config, Config::default());
    }

    #[test]
    fn the_recent_list_is_capped() {
        let mut config = Config::default();
        for i in 0..20 {
            config.remember(format!("/{i}.csv").into());
        }
        assert_eq!(config.recent.len(), RECENT);
        assert_eq!(config.recent[0], PathBuf::from("/19.csv"));
    }
}
