//! Embeds the icons and version information into splicr.exe. Other systems have no such resources.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // Checking the Windows build from macOS or Linux (`cargo clippy --target x86_64-pc-windows-msvc`):
    // rc.exe only exists on Windows, so that build goes without icons. Releases are built on Windows.
    if !std::env::var("HOST").is_ok_and(|host| host.contains("windows")) {
        println!("cargo:warning=icons not embedded: rc.exe needs a Windows host");
        return;
    }
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    let mut res = winresource::WindowsResource::new();
    // Id 1 is read back as `app::ICON`, and is the one Explorer shows.
    res.set_icon_with_id("../../assets/icon.ico", "1")
        // Task Manager shows FileDescription as the process name.
        .set("FileDescription", "Splicr")
        .set("ProductName", "Splicr")
        .set("LegalCopyright", "Copyright (c) 2026 Brad Moyetones");
    res.compile().expect("embedding Windows resources (needs rc.exe from the Windows SDK)");
}
