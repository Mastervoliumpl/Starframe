use crate::{
    app::Shared,
    model::{CatalogStatus, CommandError, GameAction, SavedData},
};
use starframe::{
    catalog::refresh::Refresh,
    deployment,
    game::{self, GameView, Running},
    launch::{self, LaunchView, Phase},
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
        || launch::requested(&store.borrow().load().map_err(|e| e.to_string())?),
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
    let (sender, receiver) = mpsc::sync_channel(1);
    let busy = Arc::new(AtomicBool::new(true));
    let worker_busy = busy.clone();
    let writing = Arc::new(AtomicBool::new(false));
    let worker_writing = writing.clone();
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
        let mut catalog = match storage.as_ref().map(Refresh::load).transpose() {
            Ok(value) => value,
            Err(error) => {
                core.lock().expect("state lock").catalog(CatalogStatus {
                    error: Some(error),
                    ..Default::default()
                });
                None
            }
        };
        if storage.is_none() {
            core.lock().expect("state lock").catalog(CatalogStatus {
                error: Some(
                    "Catalog refresh is unavailable because saved data could not be opened.".into(),
                ),
                ..Default::default()
            });
        }
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
        let mut recovery_observation = None;
        let mut prepared = None;
        let mut launch_requested: Option<Instant> = None;
        let mut process_seen = false;
        let mut process_wait: Option<Instant> = None;
        loop {
            if core.lock().expect("state lock").stopped {
                if worker_writing.load(Ordering::SeqCst) {
                    app.exit(0);
                }
                break;
            }
            let now = SystemTime::now();
            if let (Some(catalog), Some(storage)) = (&mut catalog, &mut storage) {
                let (ready, stopped) = {
                    let core = core.lock().expect("state lock");
                    (core.shell_ready(), core.stopped)
                };
                catalog.tick(
                    storage,
                    now.duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    ready,
                    stopped,
                );
                core.lock().expect("state lock").catalog(CatalogStatus {
                    revision: catalog
                        .cache
                        .catalog
                        .as_ref()
                        .map(|c| c.catalog_revision.clone()),
                    release_count: catalog.cache.catalog.as_ref().map_or(0, |c| {
                        c.releases().filter(|(_, r)| !r.withdrawn).count() as u32
                    }),
                    checking: catalog.checking,
                    last_checked: catalog.cache.last_checked.map(|t| t.to_string()),
                    last_success: catalog.cache.last_success.map(|t| t.to_string()),
                    error: catalog.cache.error.clone(),
                });
            }
            if game::observation_expired(observed, now)
                || validated.elapsed() >= Duration::from_secs(30)
            {
                view.running = Running::Unknown;
                core.lock().expect("state lock").game(view.clone());
                revalidate(&mut view, &selected);
                validated = Instant::now();
            }
            observed = now;
            let processes = windows_game::observe();
            view.running = view.selected.as_ref().map_or(Running::Unknown, |game| {
                game::classify(
                    &PathBuf::from(&game.executable),
                    processes
                        .as_ref()
                        .map(|v| {
                            v.iter()
                                .map(|p| p.as_ref().map(|p| p.path.clone()))
                                .collect()
                        })
                        .map_err(Clone::clone),
                )
            });
            let process = processes.as_ref().ok().and_then(|v| {
                v.iter().flatten().find(|p| {
                    view.selected
                        .as_ref()
                        .is_some_and(|g| std::path::Path::new(&g.executable) == p.path)
                })
            });
            let observation = view.selected.as_ref().map(|game| {
                (
                    game.executable.clone(),
                    game.build.clone(),
                    view.running.clone(),
                    process.map(|p| (p.pid, p.start.clone())),
                )
            });
            if observation != recovery_observation
                && view.running == Running::Stopped
                && let (Some(store), Some(game)) = (storage.as_mut(), view.selected.as_ref())
            {
                match deployment::recover(store, game) {
                    Ok(true) => {
                        view.message =
                            "Previous bootstrap deployment restored after interruption.".into()
                    }
                    Ok(false) => {}
                    Err(message) => view.error = message,
                }
            }
            if observation != recovery_observation {
                process_wait = None;
                prepared = match (storage.as_ref(), view.selected.as_ref()) {
                    (Some(store), Some(game)) => match deployment::prepared_activation(store, game)
                    {
                        Ok(value) => value,
                        Err(error) => {
                            view.launch = LaunchView::new(Phase::Failed, &error);
                            None
                        }
                    },
                    _ => None,
                };
                if view.running == Running::Stopped
                    && launch_requested.is_none()
                    && view.launch.phase != Phase::Failed
                {
                    view.launch = if prepared.is_some() {
                        LaunchView::new(
                            Phase::Ready,
                            "Runtime installed. The latest setup will be checked before launch.",
                        )
                    } else {
                        LaunchView::new(
                            Phase::SetupRequired,
                            "Install the Starframe runtime to finish setup.",
                        )
                    };
                }
            }
            if let Some(process) = process {
                process_seen = true;
                let waited = process_wait.get_or_insert_with(Instant::now).elapsed();
                view.launch = if let (Some(activation), Some(game)) = (&prepared, &view.selected) {
                    let report = launch::read_report(
                        &PathBuf::from(&game.executable)
                            .parent()
                            .unwrap()
                            .join("Starframe/report.json"),
                    );
                    match report {
                        Ok(report) => launch::runtime_view(
                            activation,
                            report.as_ref(),
                            process.pid,
                            &process.start,
                        ),
                        Err(error) => LaunchView::new(Phase::ProcessObserved, &error),
                    }
                } else {
                    LaunchView::new(
                        Phase::ProcessObserved,
                        "Game running. Starframe runtime activation is unverified.",
                    )
                };
                if waited >= Duration::from_secs(60) && view.launch.phase == Phase::ProcessObserved
                {
                    view.launch.details = vec!["No matching runtime result was confirmed within 60 seconds. Check the game's BepInEx log after it closes.".into()];
                }
            } else if view.running == Running::Stopped {
                process_wait = None;
                if process_seen {
                    process_seen = false;
                    launch_requested = None;
                    view.launch = if prepared.is_some() {
                        LaunchView::new(
                            Phase::Ready,
                            "Game closed. Ready to prepare the next launch.",
                        )
                    } else {
                        LaunchView::new(
                            Phase::SetupRequired,
                            "Game closed. Finish setup to enable the runtime.",
                        )
                    };
                } else if launch_requested
                    .is_some_and(|time| time.elapsed() >= Duration::from_secs(30))
                {
                    launch_requested = None;
                    view.launch = LaunchView::new(
                        Phase::Failed,
                        "Windows accepted the launch request, but no game process was observed within 30 seconds. Retry setup before launching again.",
                    );
                }
            }
            recovery_observation = observation;
            view.busy = worker_busy.load(Ordering::SeqCst);
            core.lock().expect("state lock").game(view.clone());
            let request = match receiver.recv_timeout(Duration::from_secs(2)) {
                Ok(request) => request,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            recovery_observation = None;
            view.error.clear();
            view.running = Running::Unknown;
            view.busy = true;
            core.lock().expect("state lock").game(view.clone());
            if matches!(
                request,
                Request::Setup | Request::Launch | Request::RemoveRuntime
            ) {
                view.launch = LaunchView::new(
                    Phase::Preparing,
                    if matches!(request, Request::RemoveRuntime) {
                        "Removing the Starframe runtime…"
                    } else {
                        "Preparing mods…"
                    },
                );
                core.lock().expect("state lock").game(view.clone());
                let result = (|| {
                    if core.lock().expect("state lock").stopped {
                        return Err("Starframe closed before setup started.".into());
                    }
                    let game = view
                        .selected
                        .as_ref()
                        .ok_or("Choose an available game installation first.")?;
                    let store = storage
                        .as_mut()
                        .ok_or("Saved data is unavailable. Setup was not changed.")?;
                    if launch_requested.is_some() {
                        return Err(
                            "A launch request is still waiting for its game process.".into()
                        );
                    }
                    if matches!(request, Request::RemoveRuntime) {
                        deployment::remove(store, game)?;
                        prepared = None;
                        return Ok(LaunchView::new(
                            Phase::SetupRequired,
                            "Starframe runtime removed. Mod settings and unowned files were retained.",
                        ));
                    }
                    let dispatch = matches!(request, Request::Launch);
                    prepared = Some(prepare(&app, &core, store, game, dispatch)?);
                    if dispatch {
                        launch_requested = Some(Instant::now());
                        Ok(LaunchView::new(
                            Phase::LaunchRequested,
                            "Windows accepted the launch request. Waiting for the game process…",
                        ))
                    } else {
                        Ok(LaunchView::new(
                            Phase::Ready,
                            "Runtime installed. Ready to launch Sanctuary Shattered Sun.",
                        ))
                    }
                })();
                view.launch =
                    result.unwrap_or_else(|error: String| LaunchView::new(Phase::Failed, &error));
                worker_writing.store(false, Ordering::SeqCst);
                worker_busy.store(false, Ordering::SeqCst);
                if core.lock().expect("state lock").stopped {
                    app.exit(0);
                    break;
                }
                continue;
            }
            let chosen = match request {
                Request::Setup | Request::Launch | Request::RemoveRuntime => unreachable!(),
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
                            view.launch = LaunchView::default();
                            launch_requested = None;
                            process_seen = false;
                            process_wait = None;
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
    GameService {
        sender,
        busy,
        writing,
    }
}
