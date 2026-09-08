use crate::{
    app::Shared,
    model::{CatalogStatus, CommandError, GameAction, SavedData},
};
use starframe::{
    catalog::refresh::Refresh,
    deployment,
    game::{self, GameView, Running},
    launch::{self, LaunchView, Phase},
    packages::{self, Packages},
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

mod worker;

enum Request {
    Sharing(
        starframe::sharing::Action,
        tokio::sync::oneshot::Sender<Result<starframe::sharing::Reply, String>>,
    ),
    Mod(
        starframe::mods::Action,
        tokio::sync::oneshot::Sender<Result<starframe::mods::View, String>>,
    ),
    Package(
        packages::Action,
        tokio::sync::oneshot::Sender<Result<Vec<packages::Operation>, String>>,
    ),
    Discover,
    Setup,
    Launch,
    RemoveRuntime,
    Select(String),
    Picked(Result<Option<PathBuf>, String>),
}
pub struct GameService {
    sender: SyncSender<Request>,
    busy: Arc<AtomicBool>,
    writing: Arc<AtomicBool>,
}
impl GameService {
    pub async fn sharing(
        &self,
        action: starframe::sharing::Action,
    ) -> Result<starframe::sharing::Reply, CommandError> {
        let (reply, result) = tokio::sync::oneshot::channel();
        self.sender
            .try_send(Request::Sharing(action, reply))
            .map_err(|_| {
                CommandError::new("sharing_busy", "The storage worker is busy. Retry shortly.")
            })?;
        result
            .await
            .map_err(|_| {
                CommandError::new(
                    "sharing_unavailable",
                    "Collection sharing is unavailable. Restart Starframe.",
                )
            })?
            .map_err(|message| CommandError::new("sharing_failed", &message))
    }
    pub async fn mods(
        &self,
        action: starframe::mods::Action,
    ) -> Result<starframe::mods::View, CommandError> {
        let (reply, result) = tokio::sync::oneshot::channel();
        self.sender
            .try_send(Request::Mod(action, reply))
            .map_err(|_| {
                CommandError::new("mods_busy", "The storage worker is busy. Retry shortly.")
            })?;
        result
            .await
            .map_err(|_| {
                CommandError::new(
                    "mods_unavailable",
                    "Mod management is unavailable. Restart Starframe.",
                )
            })?
            .map_err(|message| CommandError::new("mods_failed", &message))
    }
    pub async fn package(
        &self,
        action: packages::Action,
    ) -> Result<Vec<packages::Operation>, CommandError> {
        let (reply, result) = tokio::sync::oneshot::channel();
        self.sender
            .try_send(Request::Package(action, reply))
            .map_err(|_| {
                CommandError::new("package_busy", "The storage worker is busy. Retry shortly.")
            })?;
        result
            .await
            .map_err(|_| {
                CommandError::new(
                    "package_unavailable",
                    "Package preparation is unavailable. Restart Starframe.",
                )
            })?
            .map_err(|message| CommandError::new("package_failed", &message))
    }
    pub fn writing(&self) -> bool {
        self.writing.load(Ordering::SeqCst)
    }

    pub fn request(&self, app: tauri::AppHandle, action: GameAction) -> Result<(), CommandError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(CommandError::new(
                "game_busy",
                "A game operation is already in progress.",
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
            GameAction::Setup => {
                self.writing.store(true, Ordering::SeqCst);
                Request::Setup
            }
            GameAction::Launch => {
                self.writing.store(true, Ordering::SeqCst);
                Request::Launch
            }
            GameAction::RemoveRuntime => {
                self.writing.store(true, Ordering::SeqCst);
                Request::RemoveRuntime
            }
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
            self.writing.store(false, Ordering::SeqCst);
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

fn resources(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(root) = std::env::var_os("STARFRAME_INTEGRATION_DIR") {
        return Ok(root.into());
    }
    app.path()
        .resource_dir()
        .map(|p| p.join("integration"))
        .map_err(|e| e.to_string())
}

fn prepare(
    app: &tauri::AppHandle,
    core: &Shared,
    store: &mut Storage,
    game: &game::Installation,
    dispatch: bool,
) -> Result<serde_json::Value, String> {
    let resources = resources(app)?;
    if !resources.join("runtime/runtime-package.json").is_file() {
        return Err("This Starframe build does not include the game runtime. Use a build with runtime support to finish setup.".into());
    }
    let store = std::cell::RefCell::new(store);
    launch::prepare_latest(
        || starframe::mods::requested(&store.borrow()),
        |activation| {
            deployment::prepare_desktop(
                &mut store.borrow_mut(),
                game,
                &resources,
                activation,
                &|| core.lock().expect("state lock").stopped,
            )
            .map(|_| ())
        },
        || {
            if core.lock().expect("state lock").stopped {
                return Err("Starframe closed before launch was requested.".into());
            }
            if dispatch {
                windows_game::launch(game)?;
            }
            Ok(())
        },
    )
}

pub fn start(app: tauri::AppHandle, core: Shared) -> GameService {
    let (sender, receiver) = mpsc::sync_channel(8);
    let busy = Arc::new(AtomicBool::new(true));
    let worker_busy = busy.clone();
    let writing = Arc::new(AtomicBool::new(false));
    let worker_writing = writing.clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker::run(app, core, worker_busy, worker_writing, receiver)
    });
    GameService {
        sender,
        busy,
        writing,
    }
}
