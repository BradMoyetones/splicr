//! The system's file dialogs, through rfd on every system. Each is waited for on a thread, as
//! Remapr and WordCraft show theirs: on macOS a sheet on the window, as a modal panel would run
//! AppKit's loop inside winit's event handler; on Linux the desktop portal's, which would otherwise
//! block the window until it closes. The answer comes back as a [`Picked`] through a channel.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::mpsc::Sender;

use eframe::egui;

use crate::i18n::tr;

/// A dialog the app asked for, shown at the end of the frame, which has the window it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// Files to open.
    Open,
    /// The folder exports go to.
    ExportFolder,
}

/// What the user picked.
#[derive(Debug)]
pub enum Picked {
    Files(Vec<PathBuf>),
    Folder(PathBuf),
}

type Picking = Pin<Box<dyn Future<Output = Option<Picked>> + Send>>;

/// Shows `dialog` over `frame`'s window; the choice goes to `answers`, from a thread.
pub fn show(
    dialog: Dialog,
    frame: &eframe::Frame,
    start_in: Option<PathBuf>,
    answers: Sender<Picked>,
    ctx: egui::Context,
) {
    let mut file = rfd::AsyncFileDialog::new().set_parent(frame);
    if let Some(dir) = start_in.filter(|d| d.is_dir()) {
        file = file.set_directory(dir);
    }
    // Built here, on the UI thread: macOS shows the sheet now.
    let picking: Picking = match dialog {
        Dialog::Open => {
            let pick = file
                .set_title(tr!("Open files"))
                .add_filter(tr!("Spreadsheets and CSV"), splicr_engine::EXTENSIONS)
                .pick_files();
            Box::pin(async move {
                let files = pick.await?;
                Some(Picked::Files(files.iter().map(|f| f.path().to_path_buf()).collect()))
            })
        }
        Dialog::ExportFolder => {
            let pick = file.set_title(tr!("Where to save")).pick_folder();
            Box::pin(async move { Some(Picked::Folder(pick.await?.path().to_path_buf())) })
        }
    };
    let waiting = move || {
        if let Some(picked) = pollster::block_on(picking) {
            let _ = answers.send(picked);
            ctx.request_repaint();
        }
    };
    if let Err(e) = std::thread::Builder::new().name("splicr-file-dialog".into()).spawn(waiting) {
        log::error!("couldn't wait for the file dialog: {e}");
    }
}
