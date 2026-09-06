use crate::{
    app::Shared,
    model::{CommandError, GameAction, SavedData},
};
use starframe::{
    game::{self, GameView, Running},
    storage::Storage,
    windows_game,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant, SystemTime},
};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

enum Request {
    Discover,
    Select(String),
    Picked(Result<Option<PathBuf>, String>),
}
pub struct GameService {
    sender: SyncSender<Request>,
    busy: Arc<AtomicBool>,
}
impl GameService {
    pub fn request(&self, app: tauri::AppHandle, action: GameAction) -> Result<(), CommandError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(CommandError::new(
                "game_busy",
                "A game location check is already in progress.",
            ));
        }
        if let Err(error) = app
            .state::<Shared>()
            .lock()
            .expect("state lock")
            .begin_game_request()
        {
            self.busy.store(false, Ordering::SeqCst);
            return Err(error);
        }
        let request = match action {
            GameAction::Discover => Request::Discover,
            GameAction::Select { id } => Request::Select(id),
            GameAction::ChooseFolder => {
                let sender = self.sender.clone();
                let mut picker = app
                    .dialog()
                    .file()
                    .set_title("Choose the Sanctuary installation folder");
                if let Some(window) = app.get_webview_window("main") {
                    picker = picker.set_parent(&window);
                }
                picker.pick_folder(move |folder| {
                    let result = folder
                        .map(|p| p.into_path().map_err(|e| e.to_string()))
                        .transpose();
                    let _ = sender.send(Request::Picked(result));
                });
                return Ok(());
            }
        };
        self.sender.try_send(request).map_err(|_| {
            self.busy.store(false, Ordering::SeqCst);
            CommandError::new(
                "game_unavailable",
                "Game discovery is unavailable. Restart Starframe.",
            )
        })
    }
}

fn saved_status(storage: &Storage) -> Result<SavedData, String> {
    let records = storage.load().map_err(|e| e.to_string())?;
    Ok(SavedData::Ready {
        active_collection_name: records
            .collections
            .iter()
            .find(|c| Some(&c.id) == records.active_collection.as_ref())
            .map(|c| c.name.clone()),
        revision: records.revision.to_string(),
        library_count: records
            .library
            .len()
            .try_into()
            .map_err(|_| "Too many library records.")?,
        collection_count: records
            .collections
            .len()
            .try_into()
            .map_err(|_| "Too many collections.")?,
    })
}
fn discover(view: &mut GameView) {
    match windows_game::steam_root() {
        Ok(root) => {
            let (candidates, errors) = game::discover(&root);
            view.message = if candidates.is_empty() {
                "No supported Steam installation found. Choose the game folder.".into()
            } else {
                "Choose an installation to save its location.".into()
            };
            view.candidates = candidates;
            view.error = errors.join("\n");
        }
        Err(e) => {
            view.message = e;
            view.candidates.clear();
        }
    }
}
fn revalidate(view: &mut GameView, selected: &Option<(String, String)>) {
    if let Some((id, path)) = selected {
        match game::inspect(&PathBuf::from(path)) {
            Ok(mut item) => {
                if view.selected.is_none() {
                    view.error.clear();
                    view.message = "Saved game location is available.".into();
                }
                item.id = id.clone();
                view.selected = Some(item);
            }
            Err(e) => {
                view.selected = None;
                view.running = Running::Unknown;
                view.error = e;
            }
        }
    }
}

pub fn start(app: tauri::AppHandle, core: Shared) -> GameService {
    let (sender, receiver) = mpsc::sync_channel(1);
    let busy = Arc::new(AtomicBool::new(true));
    let worker_busy = busy.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut view = GameView::default();
        let result = crate::data_directory(&app).and_then(|root| {
            Storage::open(&root).map_err(|e| format!("{e} Data folder: {}", root.display()))
        });
        let mut storage = match result {
            Ok(storage) => {
                core.lock().expect("state lock").saved_data(
                    saved_status(&storage)
                        .unwrap_or_else(|message| SavedData::Unavailable { message }),
                );
                Some(storage)
            }
            Err(message) => {
                core.lock()
                    .expect("state lock")
                    .saved_data(SavedData::Unavailable { message });
                None
            }
        };
        let mut selected = None;
        if let Some(storage) = &storage {
            match storage.selected_game() {
                Ok(value) => selected = value,
                Err(e) => view.error = e.to_string(),
            }
        }
        view.selected_path = selected.as_ref().map(|(_, path)| path.clone());
        discover(&mut view);
        revalidate(&mut view, &selected);
        worker_busy.store(false, Ordering::SeqCst);
        let mut validated = Instant::now();
        let mut observed = SystemTime::now();
        loop {
            if core.lock().expect("state lock").stopped {
                break;
            }
            let now = SystemTime::now();
            if game::observation_expired(observed, now)
                || validated.elapsed() >= Duration::from_secs(30)
            {
                view.running = Running::Unknown;
                core.lock().expect("state lock").game(view.clone());
                revalidate(&mut view, &selected);
                validated = Instant::now();
            }
            observed = now;
            view.running = view.selected.as_ref().map_or(Running::Unknown, |game| {
                game::classify(&PathBuf::from(&game.executable), windows_game::processes())
            });
            view.busy = worker_busy.load(Ordering::SeqCst);
            core.lock().expect("state lock").game(view.clone());
            let request = match receiver.recv_timeout(Duration::from_secs(2)) {
                Ok(request) => request,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            view.error.clear();
            view.running = Running::Unknown;
            view.busy = true;
            core.lock().expect("state lock").game(view.clone());
            let chosen = match request {
                Request::Discover => {
                    discover(&mut view);
                    revalidate(&mut view, &selected);
                    Ok(None)
                }
                Request::Select(id) => view
                    .candidates
                    .iter()
                    .find(|c| c.id == id)
                    .ok_or_else(|| {
                        "This discovery result has expired. Find the game again.".to_owned()
                    })
                    .and_then(|c| game::inspect(&PathBuf::from(&c.path)))
                    .map(Some),
                Request::Picked(result) => {
                    result.and_then(|path| path.map(|path| game::inspect(&path)).transpose())
                }
            };
            match chosen {
                Ok(Some(item)) => {
                    let save = storage
                        .as_mut()
                        .ok_or_else(|| {
                            "Saved data is unavailable; the game selection was not changed."
                                .to_owned()
                        })
                        .and_then(|store| {
                            store
                                .select_game(&item.id, &item.path)
                                .map_err(|e| e.to_string())
                        });
                    match save {
                        Ok(_) => {
                            selected = Some((item.id.clone(), item.path.clone()));
                            view.selected_path = Some(item.path.clone());
                            view.selected = Some(item);
                            view.message =
                                "Game location saved. Game files were not changed.".into();
                            if let Some(store) = &storage {
                                core.lock().expect("state lock").saved_data(
                                    saved_status(store).unwrap_or_else(|message| {
                                        SavedData::Unavailable { message }
                                    }),
                                );
                            }
                        }
                        Err(e) => view.error = e,
                    }
                }
                Ok(None) => {}
                Err(e) => view.error = e,
            }
            worker_busy.store(false, Ordering::SeqCst);
        }
    });
    GameService { sender, busy }
}
