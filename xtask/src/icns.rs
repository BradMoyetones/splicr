//! Packs PNG images into a macOS `.icns`, on any system (`iconutil` only exists on macOS).
//!
//! From ArtCraft's craft-launcher (`xtask/src/icns.rs`, MIT OR Apache-2.0, Copyright (c) 2026
//! ArtCraft Team and the ArtCraft Launcher contributors). Changed: it takes each image with its
//! size, as `cargo xtask icons` renders them, instead of reading the size from the PNG.
//!
//! Every entry is a PNG, which macOS reads for all of these types since 10.7. Each PNG fills
//! every slot of its pixel size: a 32 px image is both the 32 pt icon and the 16 pt @2x one.

/// The PNG entry types, by pixel size.
const SLOTS: [(u32, &[u8; 4]); 11] = [
    (16, b"icp4"),
    (32, b"icp5"),
    (32, b"ic11"), // 16 @2x
    (64, b"icp6"),
    (64, b"ic12"), // 32 @2x
    (128, b"ic07"),
    (256, b"ic08"),
    (256, b"ic13"), // 128 @2x
    (512, b"ic09"),
    (512, b"ic14"),  // 256 @2x
    (1024, b"ic10"), // 512 @2x
];

/// The sizes [`SLOTS`] takes.
pub const SIZES: [u32; 7] = [16, 32, 64, 128, 256, 512, 1024];

/// An `.icns` file holding `pngs`, each a square PNG of the given size in pixels.
pub fn pack(pngs: &[(u32, Vec<u8>)]) -> Result<Vec<u8>, String> {
    if pngs.is_empty() {
        return Err("need at least one PNG".into());
    }
    let mut body = Vec::new();
    for (size, png) in pngs {
        let types: Vec<&[u8; 4]> = SLOTS.iter().filter(|(s, _)| s == size).map(|(_, kind)| *kind).collect();
        if types.is_empty() {
            return Err(format!("no .icns slot for a {size} px image (use {SIZES:?})"));
        }
        let len = u32::try_from(png.len() + 8).map_err(|_| "PNG too large for an .icns".to_string())?;
        for kind in types {
            body.extend_from_slice(kind);
            body.extend_from_slice(&len.to_be_bytes());
            body.extend_from_slice(png);
        }
    }
    let total = u32::try_from(body.len() + 8).map_err(|_| ".icns too large".to_string())?;
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&total.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The (type, payload) entries of an `.icns`.
    fn entries(icns: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut at = 8;
        while at < icns.len() {
            let len = u32::from_be_bytes(icns[at + 4..at + 8].try_into().unwrap()) as usize;
            out.push((String::from_utf8(icns[at..at + 4].to_vec()).unwrap(), icns[at + 8..at + len].to_vec()));
            at += len;
        }
        out
    }

    #[test]
    fn fills_every_slot_of_each_size() {
        let (a, b) = (b"png 32".to_vec(), b"png 1024".to_vec());
        let icns = pack(&[(32, a.clone()), (1024, b.clone())]).unwrap();
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize, icns.len());
        let e = entries(&icns);
        let kinds: Vec<&str> = e.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(kinds, ["icp5", "ic11", "ic10"]);
        assert_eq!(e[0].1, a);
        assert_eq!(e[2].1, b);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(pack(&[]).is_err());
        assert!(pack(&[(48, b"png".to_vec())]).is_err());
    }

    #[test]
    fn committed_icon_is_valid() {
        let icns = std::fs::read(crate::root().join("assets/icon.icns")).expect("run cargo xtask icons");
        assert_eq!(&icns[..4], b"icns");
        let kinds: Vec<String> = entries(&icns).into_iter().map(|(k, _)| k).collect();
        assert_eq!(kinds.len(), SLOTS.len(), "every slot filled: {kinds:?}");
    }
}
