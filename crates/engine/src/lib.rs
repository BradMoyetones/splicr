//! Splicr's file work, on a thread of its own so the window never waits on a disk: the app sends
//! [`Command`]s and drains [`Event`]s once per frame, as Remapr's app talks to its engine. An export
//! reports its progress and can be cancelled between rows.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use splicr_core::plan::Plan;
pub use splicr_io::{EXTENSIONS, Error, Opened, Warning};

/// What the app asks for.
pub enum Command {
    /// Read these files, one after another.
    Open(Vec<PathBuf>),
    /// Write the files of this plan.
    Export(Plan),
}

/// What the engine reports, in the order it happens.
#[derive(Debug)]
pub enum Event {
    /// Started reading `path`, the `index`th of `count` files asked for.
    Opening {
        path: PathBuf,
        index: usize,
        count: usize,
    },
    Opened {
        path: PathBuf,
        result: Result<Opened, Error>,
    },
    /// `done` of `total` rows written; `file` is being written.
    Exporting {
        file: PathBuf,
        done: usize,
        total: usize,
    },
    /// The files written, and why the export stopped early when it did.
    Exported {
        written: Vec<PathBuf>,
        error: Option<Error>,
    },
}

/// How often an export reports its progress.
const REPORT_EVERY: Duration = Duration::from_millis(50);

pub struct Engine {
    commands: Sender<(u64, Command)>,
    events: Receiver<Event>,
    /// The last command sent, by number.
    sent: u64,
    /// The number of the export to stop. Numbers, not a flag: a cancel that arrives as one export
    /// ends can't stop the next.
    cancel: Arc<AtomicU64>,
}

impl Engine {
    /// Starts the engine's thread. `wake` is called after each event, to repaint the window.
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        let (commands, inbox) = mpsc::channel::<(u64, Command)>();
        let (outbox, events) = mpsc::channel();
        let cancel = Arc::new(AtomicU64::new(0));
        let stop = cancel.clone();
        let worker = move || {
            let send = |event: Event| {
                let _ = outbox.send(event);
                wake();
            };
            for (number, command) in inbox {
                match command {
                    Command::Open(paths) => open(&paths, &send),
                    Command::Export(plan) => export(&plan, || stop.load(Ordering::Relaxed) == number, &send),
                }
            }
        };
        if let Err(e) = std::thread::Builder::new().name("splicr-engine".into()).spawn(worker) {
            log::error!("couldn't start the engine's thread: {e}");
        }
        Self { commands, events, sent: 0, cancel }
    }

    pub fn send(&mut self, command: Command) {
        self.sent += 1;
        let _ = self.commands.send((self.sent, command));
    }

    /// Stops the last export sent after the row it is writing; its unfinished file is removed.
    pub fn cancel(&self) {
        self.cancel.store(self.sent, Ordering::Relaxed);
    }

    /// The events since the last call.
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.try_iter()
    }
}

fn open(paths: &[PathBuf], send: &impl Fn(Event)) {
    for (index, path) in paths.iter().enumerate() {
        send(Event::Opening { path: path.clone(), index, count: paths.len() });
        let started = Instant::now();
        let result = splicr_io::open(path);
        match &result {
            Ok(o) => log::info!("read {} ({} sources) in {:?}", path.display(), o.sources.len(), started.elapsed()),
            Err(e) => log::warn!("couldn't read {}: {e:?}", path.display()),
        }
        send(Event::Opened { path: path.clone(), result });
    }
}

fn export(plan: &Plan, cancelled: impl Fn() -> bool, send: &impl Fn(Event)) {
    let outputs = plan.outputs();
    let total: usize = outputs.iter().flat_map(|o| &o.sheets).map(|(_, t)| t.rows()).sum();
    let mut done = 0;
    let mut written = Vec::new();
    let mut last_report = Instant::now();
    for output in &outputs {
        send(Event::Exporting { file: output.path.clone(), done, total });
        let result = splicr_io::write(output, || {
            done += 1;
            if last_report.elapsed() >= REPORT_EVERY {
                last_report = Instant::now();
                send(Event::Exporting { file: output.path.clone(), done, total });
            }
            !cancelled()
        });
        if let Err(error) = result {
            log::warn!("export stopped at {}: {error:?}", output.path.display());
            send(Event::Exported { written, error: Some(error) });
            return;
        }
        written.push(output.path.clone());
    }
    log::info!("exported {} files, {total} rows", written.len());
    send(Event::Exported { written, error: None });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use splicr_core::plan::{Item, Mode, OutputFormat};
    use splicr_core::{Table, Value};

    use super::*;

    fn next(engine: &Engine) -> Event {
        engine.events.recv_timeout(Duration::from_secs(20)).expect("an event")
    }

    #[test]
    fn opens_and_exports_on_its_thread() {
        let dir = std::env::temp_dir().join(format!("splicr-engine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("datos.csv");
        std::fs::write(&csv, "id,total\n1,10\n2,20\n").unwrap();

        let mut engine = Engine::start(|| {});
        engine.send(Command::Open(vec![csv.clone()]));
        assert!(matches!(next(&engine), Event::Opening { index: 0, count: 1, .. }));
        let Event::Opened { result: Ok(opened), .. } = next(&engine) else { panic!("not opened") };
        let table = opened.sources[0].tables[0].clone();
        assert_eq!(table.get(2, 1), &Value::Number(20.0));

        let plan = Plan {
            mode: Mode::Files,
            format: OutputFormat::Xlsx,
            items: vec![Item { table, name: "Datos".into() }],
            folder: dir.clone(),
            file_name: String::new(),
            source_column: None,
        };
        engine.send(Command::Export(plan));
        let written = loop {
            match next(&engine) {
                Event::Exporting { total, .. } => assert_eq!(total, 3),
                Event::Exported { written, error: None } => break written,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(written, [dir.join("Datos.xlsx")]);
        assert!(written[0].exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_cancelled_export_says_so() {
        let dir = std::env::temp_dir().join(format!("splicr-engine-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut big = Table::new("big");
        for i in 0..200_000 {
            big.push_row(vec![Value::Number(f64::from(i))]);
        }
        let mut engine = Engine::start(|| {});
        let plan = Plan {
            mode: Mode::Files,
            format: OutputFormat::Csv,
            items: vec![Item { table: Arc::new(big), name: "big".into() }],
            folder: dir.clone(),
            file_name: String::new(),
            source_column: None,
        };
        engine.send(Command::Export(plan));
        engine.cancel();
        loop {
            match next(&engine) {
                Event::Exporting { .. } => {}
                Event::Exported { written, error } => {
                    assert!(written.is_empty());
                    assert_eq!(error, Some(Error::Cancelled));
                    break;
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(!dir.join("big.csv").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
