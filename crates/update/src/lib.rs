//! Splicr's updater, after Remapr's: finds a newer release on GitHub, downloads this system's
//! package, checks it against the published SHA-256 and the project's ed25519 signature, and puts it
//! in place ([`install`]):
//!
//! - Windows: the per-user MSI installs over the running Splicr and starts it again, with no
//!   administrator.
//! - macOS: the new `Splicr.app` is copied out of the DMG and swapped for the one running, as
//!   Tauri's updater does. No Developer ID is needed: the package is trusted by the project's own
//!   signature, and a download that no browser made carries no quarantine for Gatekeeper.
//! - Linux: an AppImage is replaced by the new one. The deb needs root: the app opens the release
//!   page instead, as for a copy that can't replace itself (one run from the DMG, a folder without
//!   write access).
//!
//! Only the version is read from GitHub's answer; download addresses are built from it, so an
//! unexpected response can't point the updater anywhere else. The signature makes the hosting
//! irrelevant: a package changed on GitHub, checksum and all, is still refused.
//!
//! The network goes through curl, which Windows 10 and later, macOS and most Linux systems ship, so
//! Splicr carries no TLS stack of its own.
//! shortcut: curl.exe ignores the system proxy settings (it honours `HTTPS_PROXY`); move to WinHTTP if
//! users behind a proxy report failed checks.
//!
//! Errors are an [`Error`]; the app puts them into words, in the user's language.

mod replace;

use std::io::Read;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub use replace::{Installer, install, installer};

pub const REPO: &str = "BradMoyetones/splicr";
/// The project's ed25519 public key, in hex: packages are signed with its private half in the
/// release workflow (`cargo xtask sign`, secret `SPLICR_SIGNING_KEY`). Never change it without a
/// release that carries both: copies out there only accept packages signed with the key they have.
pub const PUBLIC_KEY: &str = "";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// What went wrong, for the app to put into words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No connection, or curl couldn't run; curl's own words, possibly empty.
    Network(String),
    /// GitHub answered something unexpected, or refused (its message, like a rate limit).
    GitHub(String),
    /// The published checksum file isn't one.
    UnreadableChecksum,
    /// The download doesn't match the published checksum; it was deleted.
    ChecksumMismatch,
    /// The download isn't signed with the project's key; it was deleted.
    BadSignature,
    /// A file couldn't be written or read; the system's words.
    File(String),
    /// Windows Installer's exit code, or `None` when it couldn't start or was interrupted.
    Installer(Option<i32>),
    /// The new app couldn't be put in place of the old one, which stays; the system's words.
    Replace(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// The release page on GitHub, with its notes.
    pub notes_url: String,
    url: String,
    sha256_url: String,
    sig_url: String,
    /// The public key the package must be signed with, in hex.
    key: String,
    /// Bytes, for the download's progress.
    size: u64,
}

/// This system's package of `version`, as `cargo xtask dist` names it and the release publishes it:
/// on Linux the AppImage when Splicr runs from one, the deb otherwise.
pub fn package(version: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("Splicr-{version}-macos-universal.dmg")
    } else if cfg!(target_os = "linux") {
        let kind = if std::env::var_os("APPIMAGE").is_some() { "AppImage" } else { "deb" };
        format!("Splicr-{version}-linux-x86_64.{kind}")
    } else {
        format!("Splicr-{version}-x64.msi")
    }
}

/// The page of `version`'s release, with its notes ("Novedades").
pub fn release_page(version: &str) -> String {
    format!("https://github.com/{REPO}/releases/tag/v{version}")
}

/// Where releases come from: GitHub in the app, local files in tests.
pub struct Source {
    latest: String,
    downloads: String,
    key: String,
}

impl Source {
    pub fn github() -> Self {
        Self {
            latest: format!("https://api.github.com/repos/{REPO}/releases/latest"),
            downloads: format!("https://github.com/{REPO}/releases/download"),
            key: PUBLIC_KEY.to_owned(),
        }
    }

    /// A folder laid out like GitHub's: `latest.json` (the API's answer) and `v<version>/` with the
    /// files. For tests, and for trying an update in a debug build before publishing anything.
    pub fn local(dir: &Path) -> Self {
        let path = dir.display().to_string().replace('\\', "/").replace(' ', "%20");
        // `C:/…` on Windows, `/…` elsewhere: either way one slash after the empty host.
        let url = format!("file:///{}", path.trim_start_matches('/'));
        Self { latest: format!("{url}/latest.json"), downloads: url, key: PUBLIC_KEY.to_owned() }
    }

    /// Packages from this source must be signed with `key` (hex) rather than [`PUBLIC_KEY`]: for
    /// tests, and for a test folder signed with a test key.
    pub fn with_key(mut self, key: &str) -> Self {
        key.clone_into(&mut self.key);
        self
    }

    /// The latest release, if it is newer than `current` and this system's package is uploaded
    /// with its checksum and signature.
    pub fn check(&self, current: &str) -> Result<Option<Release>, Error> {
        // No `-f`: GitHub answers "no release yet" with a 404 whose JSON says so.
        let body = curl(&["-sSL", "--max-time", "20", "-H", "Accept: application/vnd.github+json", &self.latest])?;
        self.parse(&body, current)
    }

    fn parse(&self, body: &[u8], current: &str) -> Result<Option<Release>, Error> {
        #[derive(Deserialize)]
        struct Latest {
            tag_name: Option<String>,
            message: Option<String>,
            #[serde(default)]
            assets: Vec<Asset>,
        }
        #[derive(Deserialize)]
        struct Asset {
            name: String,
            size: u64,
        }

        let latest: Latest = serde_json::from_slice(body).map_err(|e| Error::GitHub(e.to_string()))?;
        let Some(tag) = latest.tag_name else {
            return match latest.message.as_deref() {
                None | Some("Not Found") => Ok(None),
                Some(message) => Err(Error::GitHub(message.to_owned())),
            };
        };
        let version = tag.trim_start_matches(['v', 'V']).to_owned();
        let (Some(new), Some(old)) = (parse_version(&version), parse_version(current)) else {
            return Err(Error::GitHub(format!("unexpected version {tag}")));
        };
        let package = package(&version);
        let (sha256, sig) = (format!("{package}.sha256"), format!("{package}.sig"));
        let size = latest.assets.iter().find(|a| a.name == package).map(|a| a.size);
        let has = |name: &str| latest.assets.iter().any(|a| a.name == name);
        // A release whose files are still uploading is picked up on a later check; one without a
        // signature, or a copy without a key to check it with, is never offered.
        let (true, Some(size), true, true, false) = (new > old, size, has(&sha256), has(&sig), self.key.is_empty())
        else {
            return Ok(None);
        };
        let at = |file: &str| format!("{}/v{version}/{file}", self.downloads);
        Ok(Some(Release {
            notes_url: release_page(&version),
            url: at(&package),
            sha256_url: at(&sha256),
            sig_url: at(&sig),
            key: self.key.clone(),
            size,
            version,
        }))
    }
}

/// `major.minor.patch`, ignoring a pre-release or build suffix.
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// Downloads `release`'s package into `dir`, calling `progress` with 0..=1, and checks it against
/// the published SHA-256 and signature. A file that doesn't match is deleted, never used.
pub fn download(release: &Release, dir: &Path, progress: impl Fn(f32)) -> Result<PathBuf, Error> {
    std::fs::create_dir_all(dir).map_err(|e| Error::File(e.to_string()))?;
    let sums = curl(&["-sSfL", "--max-time", "30", &release.sha256_url])?;
    let expected = String::from_utf8_lossy(&sums)
        .split_whitespace()
        .next()
        .map(str::to_ascii_lowercase)
        .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(Error::UnreadableChecksum)?;
    let signature = curl(&["-sSfL", "--max-time", "30", &release.sig_url])?;

    let file = dir.join(package(&release.version));
    let _ = std::fs::remove_file(&file);
    let mut child = command(&["-sSfL", "--max-time", "900", "-o"])
        .arg(&file)
        .arg(&release.url)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Network(e.to_string()))?;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| Error::Network(e.to_string()))? {
            break status;
        }
        let got = std::fs::metadata(&file).map_or(0, |m| m.len());
        progress((got as f32 / release.size.max(1) as f32).min(1.0));
        std::thread::sleep(Duration::from_millis(100));
    };
    if !status.success() {
        let mut stderr = String::new();
        let _ = child.stderr.take().map(|mut e| e.read_to_string(&mut stderr));
        let _ = std::fs::remove_file(&file);
        return Err(Error::Network(stderr.trim().to_owned()));
    }
    progress(1.0);

    let bytes = std::fs::read(&file).map_err(|e| Error::File(e.to_string()))?;
    let checked = if sha256(&bytes) != expected {
        Err(Error::ChecksumMismatch)
    } else if !verify(&bytes, &String::from_utf8_lossy(&signature), &release.key) {
        Err(Error::BadSignature)
    } else {
        Ok(file.clone())
    };
    if checked.is_err() {
        let _ = std::fs::remove_file(&file);
    }
    checked
}

/// Whether `signature` (hex, as `cargo xtask sign` writes it) is `key`'s (hex) over `bytes`.
pub fn verify(bytes: &[u8], signature: &str, key: &str) -> bool {
    let (Some(signature), Some(key)) = (hex::<64>(signature.trim()), hex::<32>(key.trim())) else { return false };
    let Ok(key) = VerifyingKey::from_bytes(&key) else { return false };
    key.verify_strict(bytes, &Signature::from_bytes(&signature)).is_ok()
}

/// Exactly `N` bytes from hex; `None` for anything else.
fn hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.as_bytes();
    if text.len() != 2 * N {
        return None;
    }
    let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = [0u8; N];
    for (byte, [high, low]) in out.iter_mut().zip(text.as_chunks::<2>().0) {
        *byte = digit(*high)? << 4 | digit(*low)?;
    }
    Some(out)
}

/// Windows Installer's exit code as a result: success (also "restart needed") or [`Error::Installer`].
pub fn install_result(code: Option<i32>) -> Result<(), Error> {
    match code {
        Some(0 | 3010 | 1641) => Ok(()),
        code => Err(Error::Installer(code)),
    }
}

fn curl(args: &[&str]) -> Result<Vec<u8>, Error> {
    let out = command(args).output().map_err(|e| Error::Network(e.to_string()))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(Error::Network(String::from_utf8_lossy(&out.stderr).trim().to_owned()))
    }
}

fn command(args: &[&str]) -> Command {
    #[cfg(windows)]
    let mut command = Command::new(system32("curl.exe"));
    // The one on the PATH: unlike Windows, Unix never looks in the working folder first.
    #[cfg(not(windows))]
    let mut command = Command::new("curl");
    // The header goes first: callers append options whose values follow them (`-o <file>`).
    command.arg("-H").arg(concat!("User-Agent: Splicr/", env!("CARGO_PKG_VERSION"))).args(args);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// The full path: never a same-named program from the working folder.
#[cfg(windows)]
fn system32(program: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into())).join("System32").join(program)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    /// A test key: never the project's.
    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn hex_of(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn public_key() -> String {
        hex_of(signing_key().verifying_key().as_bytes())
    }

    fn sign(bytes: &[u8]) -> String {
        hex_of(&signing_key().sign(bytes).to_bytes())
    }

    fn latest(tag: &str, assets: &[(&str, u64)]) -> String {
        let assets: Vec<String> = assets.iter().map(|(n, s)| format!(r#"{{"name":"{n}","size":{s}}}"#)).collect();
        format!(r#"{{"tag_name":"{tag}","assets":[{}]}}"#, assets.join(","))
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(parse_version("0.10.0") > parse_version("0.9.9"));
        assert_eq!(parse_version("1.2.3-rc.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), None);
    }

    #[test]
    fn signatures_are_checked() {
        let data = b"Splicr-0.2.0";
        assert!(verify(data, &sign(data), &public_key()));
        assert!(!verify(b"Splicr-0.2.1", &sign(data), &public_key()), "another file");
        let other = hex_of(SigningKey::from_bytes(&[8; 32]).verifying_key().as_bytes());
        assert!(!verify(data, &sign(data), &other), "another key");
        assert!(!verify(data, "not hex", &public_key()));
        assert!(!verify(data, &sign(data), ""), "no key");
        assert_eq!(hex::<2>("0aFf"), Some([0x0a, 0xff]));
        assert_eq!(hex::<2>("0aF"), None);
    }

    #[test]
    fn only_a_newer_complete_signed_release_is_offered() {
        let source = Source::github().with_key(&public_key());
        let package = package("0.3.0");
        let (sums, sig) = (format!("{package}.sha256"), format!("{package}.sig"));
        let full = [(package.as_str(), 10), (sums.as_str(), 90), (sig.as_str(), 128)];
        let found = source.parse(latest("v0.3.0", &full).as_bytes(), "0.2.9").unwrap().unwrap();
        assert_eq!(found.version, "0.3.0");
        assert_eq!(found.url, format!("https://github.com/BradMoyetones/splicr/releases/download/v0.3.0/{package}"));
        assert_eq!(found.notes_url, "https://github.com/BradMoyetones/splicr/releases/tag/v0.3.0");
        // Another system's package alone isn't an update for this one.
        let other = [("Splicr-0.3.0-linux-x64.AppImage", 10), ("Splicr-0.3.0-linux-x64.AppImage.sha256", 90)];
        assert_eq!(source.parse(latest("v0.3.0", &other).as_bytes(), "0.2.0").unwrap(), None);
        assert_eq!(source.parse(latest("v0.3.0", &full).as_bytes(), "0.3.0").unwrap(), None);
        // Still uploading, or unsigned.
        assert_eq!(source.parse(latest("v0.3.0", &full[..2]).as_bytes(), "0.2.0").unwrap(), None);
        // A copy with no key offers nothing.
        let keyless = Source::github().with_key("");
        assert_eq!(keyless.parse(latest("v0.3.0", &full).as_bytes(), "0.2.0").unwrap(), None);
        // No release at all, or GitHub refusing.
        assert_eq!(source.parse(br#"{"message":"Not Found"}"#, "0.2.0").unwrap(), None);
        assert!(source.parse(br#"{"message":"API rate limit exceeded"}"#, "0.2.0").is_err());
    }

    #[test]
    fn installer_codes_read_as_messages() {
        assert!(install_result(Some(0)).is_ok());
        assert!(install_result(Some(3010)).is_ok());
        assert_eq!(install_result(Some(1602)), Err(Error::Installer(Some(1602))));
    }

    /// The whole check and download through curl, against files standing in for GitHub.
    #[test]
    fn downloads_and_verifies_the_package() {
        let dir = std::env::temp_dir().join(format!("splicr-update-test-{}", std::process::id()));
        let release_dir = dir.join("v0.9.0");
        std::fs::create_dir_all(&release_dir).unwrap();
        let name = package("0.9.0");
        let (sums_name, sig_name) = (format!("{name}.sha256"), format!("{name}.sig"));
        let data = b"not really a package".repeat(1000);
        std::fs::write(release_dir.join(&name), &data).unwrap();
        std::fs::write(release_dir.join(&sums_name), format!("{}  {name}\n", sha256(&data))).unwrap();
        std::fs::write(release_dir.join(&sig_name), sign(&data)).unwrap();
        let assets = [(name.as_str(), data.len() as u64), (sums_name.as_str(), 90), (sig_name.as_str(), 128)];
        std::fs::write(dir.join("latest.json"), latest("v0.9.0", &assets)).unwrap();

        let source = Source::local(&dir).with_key(&public_key());
        let release = source.check("0.1.0").unwrap().expect("a newer release");
        let last = std::cell::Cell::new(0.0);
        let path = download(&release, &dir.join("out"), |p| last.set(p)).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), data);
        assert_eq!(last.get(), 1.0);

        // Signed with another key: refused and removed, though its checksum matches.
        let forged = SigningKey::from_bytes(&[9; 32]).sign(&data).to_bytes();
        std::fs::write(release_dir.join(&sig_name), hex_of(&forged)).unwrap();
        assert_eq!(download(&release, &dir.join("out"), |_| {}), Err(Error::BadSignature));
        assert!(!path.exists());

        // Changed, checksum and all: still refused by its signature.
        let tampered = b"tampered".to_vec();
        std::fs::write(release_dir.join(&name), &tampered).unwrap();
        std::fs::write(release_dir.join(&sums_name), format!("{}  {name}\n", sha256(&tampered))).unwrap();
        std::fs::write(release_dir.join(&sig_name), sign(&data)).unwrap();
        assert_eq!(download(&release, &dir.join("out"), |_| {}), Err(Error::BadSignature));
        let _ = std::fs::remove_dir_all(dir);
    }
}
