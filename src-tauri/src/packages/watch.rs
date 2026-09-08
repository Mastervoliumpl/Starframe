use super::*;
use crate::local_import::{LocalSource, WatchState};
use std::{collections::BTreeMap, time::Instant};

pub(super) mod assembly;
mod changes;
#[cfg(test)]
mod tests;

const QUIET: Duration = Duration::from_secs(2);
const RECHECK: Duration = Duration::from_secs(30);
const MAX_WATCHES: usize = 256;

struct Input {
    sources: Vec<LocalSource>,
    paused: bool,
    resume: bool,
}
enum Event {
    Status(LocalSource, WatchState, String),
    Ready(LocalSource, Box<PreparedImport>),
}
struct Source {
    local: LocalSource,
    changes: Option<changes::Changes>,
    due: Instant,
    candidate: Option<String>,
}

pub struct Watcher {
    input: mpsc::SyncSender<Input>,
    events: mpsc::Receiver<Event>,
    cancel: Cancel,
    resume_pending: bool,
}

impl Watcher {
    pub fn new(store: &Storage) -> Self {
        Self::start(store, true)
    }

    fn start(store: &Storage, notifications: bool) -> Self {
        let root = store.package_root().to_owned();
        let (input, receiver) = mpsc::sync_channel(1);
        let (sender, events) = mpsc::sync_channel(1);
        let cancel = Cancel::default();
        let worker_cancel = cancel.clone();
        std::thread::spawn(move || run(root, receiver, sender, worker_cancel, notifications));
        Self {
            input,
            events,
            cancel,
            resume_pending: false,
        }
    }

    pub fn poll(&mut self, store: &mut Storage, paused: bool, resume: bool) -> Result<bool> {
        self.resume_pending |= resume;
        let watches = store.local_watches().map_err(|e| e.to_string())?;
        for watch in watches.iter().skip(MAX_WATCHES) {
            store
                .watch_status(
                    &watch.source,
                    WatchState::Error,
                    "Automatic watching supports 256 local sources. Import this build manually.",
                )
                .map_err(|e| e.to_string())?;
        }
        let paused = paused
            || !store
                .pending_removals()
                .map_err(|e| e.to_string())?
                .is_empty();
        let input = Input {
            sources: watches
                .into_iter()
                .take(MAX_WATCHES)
                .map(|w| w.source)
                .collect(),
            paused,
            resume: self.resume_pending,
        };
        match self.input.try_send(input) {
            Ok(()) => self.resume_pending = false,
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(_) => {
                return Err("Local build watching stopped. Restart Starframe to resume it.".into());
            }
        }
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Status(source, state, message) => store
                    .watch_status(&source, state, &message)
                    .map_err(|e| e.to_string())?,
                Event::Ready(previous, build) => {
                    let local = build
                        .local
                        .as_ref()
                        .ok_or("Local rebuild metadata is missing.")?;
                    match store.complete_watch(&previous, local, &build.prepared) {
                        Ok(saved) => changed |= saved,
                        Err(error) => store.watch_status(&previous, WatchState::Error, &format!("{error} The previous build is still available. Watching will retry.")).map_err(|e| e.to_string())?,
                    }
                }
            }
        }
        Ok(changed)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn run(
    root: PathBuf,
    input: mpsc::Receiver<Input>,
    events: mpsc::SyncSender<Event>,
    cancel: Cancel,
    notifications: bool,
) {
    let listen = |path: &str| {
        if notifications {
            changes::Changes::open(Path::new(path)).ok()
        } else {
            None
        }
    };
    let mut sources = BTreeMap::<String, Source>::new();
    let mut paused = true;
    while cancel.check().is_ok() {
        match input.recv_timeout(Duration::from_millis(100)) {
            Ok(update) => {
                paused = update.paused;
                sources.retain(|_, source| update.sources.contains(&source.local));
                for local in update.sources {
                    let source = sources
                        .entry(local.reference.mod_id.clone())
                        .or_insert_with(|| Source {
                            changes: listen(&local.path),
                            local,
                            due: Instant::now(),
                            candidate: None,
                        });
                    if update.resume {
                        source.due = Instant::now();
                        source.candidate = None;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        for source in sources.values_mut() {
            if source
                .changes
                .as_mut()
                .is_some_and(|watch| watch.changed().unwrap_or(true))
            {
                source.due = Instant::now() + QUIET;
                source.candidate = None;
                source.changes = listen(&source.local.path);
            }
        }
        if paused {
            continue;
        }
        let Some(source) = sources
            .values_mut()
            .filter(|s| s.due <= Instant::now())
            .min_by_key(|s| s.due)
        else {
            continue;
        };
        let copying = source.candidate.is_some();
        let mode = source
            .candidate
            .as_deref()
            .map_or(local::Mode::Inspect, local::Mode::Copy);
        let result = local::prepare_source(
            &root,
            &Uuid::new_v4().to_string(),
            Path::new(&source.local.path),
            &cancel,
            &AtomicU64::new(0),
            mode,
        );
        if cancel.check().is_err() {
            break;
        }
        let changed_during_read = source
            .changes
            .as_mut()
            .is_some_and(|w| w.changed().unwrap_or(true));
        source.due = Instant::now() + RECHECK;
        source.changes = listen(&source.local.path);
        let event = if changed_during_read {
            source.candidate = None;
            source.due = Instant::now() + QUIET;
            Event::Status(source.local.clone(), WatchState::Settling, "The build changed while it was being checked. Waiting for writes to settle; the previous copy is kept.".into())
        } else {
            match result {
                Ok(build)
                    if build
                        .local
                        .as_ref()
                        .is_some_and(|l| l.reference.mod_id != source.local.reference.mod_id) =>
                {
                    source.candidate = None;
                    Event::Status(source.local.clone(), WatchState::Error, "This source now identifies a different mod. Import it explicitly; the previous build is kept.".into())
                }
                Ok(build) if build.prepared.hash == source.local.reference.hash => {
                    source.candidate = None;
                    Event::Status(source.local.clone(), WatchState::Watching, "Watching the source while Starframe is open. The verified copy is current.".into())
                }
                Ok(build) if copying => {
                    source.candidate = None;
                    Event::Ready(source.local.clone(), Box::new(build))
                }
                Ok(build) => {
                    source.candidate = Some(build.prepared.hash);
                    source.due = Instant::now() + QUIET;
                    Event::Status(
                        source.local.clone(),
                        WatchState::Settling,
                        "New output found. Waiting for writes to settle before saving a copy."
                            .into(),
                    )
                }
                Err(error) => {
                    source.candidate = None;
                    Event::Status(
                        source.local.clone(),
                        WatchState::Error,
                        format!("{error} The previous copy is kept. Watching will retry."),
                    )
                }
            }
        };
        if events.send(event).is_err() {
            break;
        }
    }
}
