// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod dialogs;
mod i18n;
mod logging;
mod router;
mod updates;
mod views;
mod workspace;

fn main() -> eframe::Result {
    logging::init(&config::dir());
    log::info!("Splicr {} on {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS);
    // Files passed on the command line (or by "Open with" on Windows and Linux) open at start.
    let files: Vec<std::path::PathBuf> =
        std::env::args_os().skip(1).map(std::path::PathBuf::from).filter(|p| p.is_file()).collect();

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Splicr")
        // Linux's Wayland app id and X11 class: what the `.desktop` file names, so the desktop shows
        // Splicr's icon and name for the window.
        .with_app_id(splicr_platform::APP_ID)
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([760.0, 520.0])
        // No egui logo: on Windows the app sets the exe's own icon once the window exists.
        .with_icon(eframe::egui::IconData::default())
        // Files can be dropped on the window.
        .with_drag_and_drop(true);
    if cfg!(target_os = "macos") {
        // The sidebar and the header run under a transparent title bar, beside the native window
        // buttons, as ArtCraft's apps and Remapr do. Windows keeps its own frame, tinted by the
        // app; Linux the desktop's.
        viewport = viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Splicr", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, files)))))
}
