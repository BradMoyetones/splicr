# Third-party work in Splicr

Splicr is MIT-licensed (see [LICENSE](LICENSE)). These parts come from others and keep their own
licences. Code from Remapr, by the same author and under the same licence, isn't listed; its own
third-party parts that came along with it are.

| What | Where in Splicr | Author | Source | Licence |
|---|---|---|---|---|
| Inter 4.1, Regular and SemiBold, cut down (see the folder's README) | `crates/ui/assets/fonts/*.ttf`; the logo's S (`assets/logo-*.svg`), as a path | Rasmus Andersson | <https://github.com/rsms/inter> | SIL OFL 1.1, [`OFL-inter.txt`](crates/ui/assets/fonts/OFL-inter.txt) |
| Lucide icons 1.64.0, unchanged | `crates/ui/assets/icons/*.svg` | Lucide Icons and Contributors; some derived from Feather by Cole Bemis | <https://github.com/lucide-icons/lucide> (`icons/<name>.svg`) | ISC (Feather: MIT), [`LICENSE-lucide.txt`](crates/ui/assets/icons/LICENSE-lucide.txt) |
| CSV decoding (Windows-1252/1256 detection), delimiter sniffing and the record parser | `crates/io/src/csv.rs` | ArtCraft Team and the GridCraft contributors | `crates/xlsx/src/csv.rs` in <https://github.com/storytold/gridcraft> | MIT OR Apache-2.0, used under MIT ([`licenses/gridcraft-LICENSE-MIT.txt`](licenses/gridcraft-LICENSE-MIT.txt)); adapted (see the file's header) |
| Painting only the visible cells, frozen header and gutter drawn from the viewport | `crates/ui/src/grid.rs` | ArtCraft Team and the GridCraft contributors | `crates/ui-egui/src/grid/` in <https://github.com/storytold/gridcraft> | MIT OR Apache-2.0; the approach, rewritten |
| Packing PNGs into a macOS `.icns` | `xtask/src/icns.rs` | ArtCraft (storytold) | `craft-launcher/xtask/src/icns.rs` in <https://github.com/storytold/craft-launcher> | MIT OR Apache-2.0; adapted (see the file's header) |
| The macOS bundle and DMG: the steps of `package.sh`, the `Info.plist` template and the empty entitlements | `xtask/src/macos.rs`, `packaging/macos/` | ArtCraft (storytold) | `craft-launcher/packaging/macos/` in <https://github.com/storytold/craft-launcher> | MIT OR Apache-2.0; ported to Rust, without notarization |
| The Linux packages: the steps of `package.sh`, the nfpm file, the `.desktop` and AppStream files, the post-install script | `xtask/src/linux.rs`, `packaging/linux/` | ArtCraft (storytold) | `craft-launcher/packaging/linux/` in <https://github.com/storytold/craft-launcher> | MIT OR Apache-2.0; ported to Rust, deb and AppImage only |
| The title-bar drag detection, and how macOS's double-click setting is read | `crates/ui/src/title_bar.rs`, `crates/platform/src/unix.rs` (`title_bar_double_click`) | ArtCraft (storytold) | `craft-libs/crates/window/src/title_bar.rs` in <https://github.com/storytold/craft-libs> | Apache-2.0; adapted (see the comments there for what changed) |
