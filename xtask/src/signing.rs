//! The packages' ed25519 signatures, which the updater checks (`splicr_update::verify`).
//!
//! - `cargo xtask keygen <folder>` makes the project's key pair, once: the private key goes to
//!   `<folder>/splicr-signing.key` (readable by its owner only; never printed), the public key to
//!   `splicr-signing.pub` and the screen. The private key becomes the repository secret
//!   `SPLICR_SIGNING_KEY`; the public one, `splicr_update::PUBLIC_KEY`.
//! - `cargo xtask sign <files…>` writes `<file>.sig` beside each package, with the private key from
//!   `SPLICR_SIGNING_KEY`. The release workflow runs it. It refuses a key whose public half isn't
//!   the one the updater has: a release signed with it would be refused by every copy.

use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};

const SECRET: &str = "SPLICR_SIGNING_KEY";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    let text = text.trim().as_bytes();
    if text.len() != 64 {
        return None;
    }
    let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = [0u8; 32];
    for (byte, [high, low]) in out.iter_mut().zip(text.as_chunks::<2>().0) {
        *byte = digit(*high)? << 4 | digit(*low)?;
    }
    Some(out)
}

pub fn keygen(folder: &Path) -> Result<(), String> {
    let (private, public) = (folder.join("splicr-signing.key"), folder.join("splicr-signing.pub"));
    if private.exists() {
        return Err(format!("{} exists: a project has one key; delete it only on purpose", private.display()));
    }
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut seed = [0u8; 32];
    random(&mut seed)?;
    let key = SigningKey::from_bytes(&seed);
    write_private(&private, &hex(&seed))?;
    let public_hex = hex(key.verifying_key().as_bytes());
    std::fs::write(&public, format!("{public_hex}\n")).map_err(|e| e.to_string())?;
    println!("private key: {} (keep it secret, back it up)", private.display());
    println!("public key:  {public_hex}");
    Ok(())
}

/// 32 bytes from the system's random source.
fn random(out: &mut [u8]) -> Result<(), String> {
    use std::io::Read;
    if !cfg!(unix) {
        return Err("keygen reads /dev/urandom: run it on macOS or Linux".into());
    }
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(out)).map_err(|e| e.to_string())
}

/// The private key, readable by its owner only.
fn write_private(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(format!("{text}\n").as_bytes()).map_err(|e| e.to_string())
}

pub fn sign(files: &[String]) -> Result<(), String> {
    let secret = std::env::var(SECRET).map_err(|_| format!("{SECRET} isn't set"))?;
    let key = SigningKey::from_bytes(&unhex(&secret).ok_or_else(|| format!("{SECRET} isn't 32 bytes in hex"))?);
    let public = hex(key.verifying_key().as_bytes());
    if public != splicr_update::PUBLIC_KEY {
        return Err(format!(
            "{SECRET}'s public key ({public}) isn't splicr_update::PUBLIC_KEY ({:?}): the updater would refuse the release",
            splicr_update::PUBLIC_KEY
        ));
    }
    let packages: Vec<&String> = files.iter().filter(|f| !f.ends_with(".sha256") && !f.ends_with(".sig")).collect();
    if packages.is_empty() {
        return Err("usage: cargo xtask sign <package files…>".into());
    }
    for file in packages {
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        let signature = hex(&key.sign(&bytes).to_bytes());
        std::fs::write(format!("{file}.sig"), format!("{signature}\n")).map_err(|e| e.to_string())?;
        // Checked as the updater will check it.
        if !splicr_update::verify(&bytes, &signature, &public) {
            return Err(format!("{file}: the signature doesn't verify"));
        }
        println!("signed {file}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_through_hex() {
        let seed = [42u8; 32];
        assert_eq!(unhex(&hex(&seed)), Some(seed));
        assert_eq!(unhex("zz"), None);
    }

    /// keygen reads /dev/urandom: macOS and Linux only.
    #[cfg(unix)]
    #[test]
    fn keygen_never_overwrites_a_key() {
        let dir = std::env::temp_dir().join(format!("splicr-keygen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        keygen(&dir).unwrap();
        let private = std::fs::read_to_string(dir.join("splicr-signing.key")).unwrap();
        assert!(unhex(&private).is_some());
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("splicr-signing.key")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(keygen(&dir).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
