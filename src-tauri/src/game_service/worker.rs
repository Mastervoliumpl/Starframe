use super::*;

#[path = "requests.rs"]
mod requests;
#[path = "session.rs"]
mod session;
use session::Session;

type Observation = (String, String, Running, Option<(u32, String)>);

struct Worker {
    app: tauri::AppHandle,
    core: Shared,
    busy: Arc<AtomicBool>,
    writing: Arc<AtomicBool>,
    storage: Option<Storage>,
    packages: Result<Option<Packages>, String>,
    local_watcher: Option<packages::watch::Watcher>,
    catalog: Option<Refresh>,
    view: GameView,
    selected: Option<(String, String)>,
    validated: Instant,
    observed: SystemTime,
    recovery_observation: Option<Observation>,
    session: Session,
    last_auto_attempt: Option<(String, Option<i64>, Running)>,
}

pub(super) fn run(
    app: tauri::AppHandle,
    core: Shared,
    busy: Arc<AtomicBool>,
    writing: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Request>,
) {
    let mut view = GameView::default();
    let result = crate::data_directory(&app).and_then(|root| {
        Storage::open(&root).map_err(|e| format!("{e} Data folder: {}", root.display()))
    });
    let mut storage = match result {
        Ok(storage) => {
            core.lock().expect("state lock").saved_data(
                saved_status(&storage).unwrap_or_else(|message| SavedData::Unavailable { message }),
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
    if let Some(store) = storage.as_mut()
        && let Err(error) = starframe::mods::cleanup(store)
    {
        view.error = error;
    }
    let packages = storage.as_mut().map(Packages::open).transpose();
    let local_watcher = storage.as_ref().map(packages::watch::Watcher::new);
    let catalog = match storage.as_ref().map(Refresh::load).transpose() {
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
    busy.store(false, Ordering::SeqCst);
    let mut worker = Worker {
        app,
        core,
        busy,
        writing,
        storage,
        packages,
        local_watcher,
        catalog,
        view,
        selected,
        validated: Instant::now(),
        observed: SystemTime::now(),
        recovery_observation: None,
        session: Session::default(),
        last_auto_attempt: None,
    };
    loop {
        if worker.core.lock().expect("state lock").stopped {
            if worker.writing.load(Ordering::SeqCst) {
                worker.app.exit(0);
            }
            break;
        }
        if worker.tick() {
            break;
        }
        worker.view.busy = worker.busy.load(Ordering::SeqCst);
        worker
            .core
            .lock()
            .expect("state lock")
            .game(worker.view.clone());
        match receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(request) => worker.handle(request),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(_) => break,
        }
    }
}

impl Worker {
    fn tick(&mut self) -> bool {
        let now = SystemTime::now();
        if let (Some(watcher), Some(store)) = (&mut self.local_watcher, &mut self.storage) {
            let paused = self
                .packages
                .as_ref()
                .ok()
                .and_then(|q| q.as_ref())
                .is_none_or(|q| q.busy());
            match watcher.poll(store, paused, game::observation_expired(self.observed, now)) {
                Ok(true) => self.core.lock().expect("state lock").saved_data(
                    saved_status(store)
                        .unwrap_or_else(|message| SavedData::Unavailable { message }),
                ),
                Ok(false) => {}
                Err(message) => self.view.error = message,
            }
        }
        if let (Ok(Some(queue)), Some(store)) = (&mut self.packages, &mut self.storage) {
            match queue.poll(store).and_then(|changed| {
                starframe::sharing::poll(store, queue)
                    .map(|imports_changed| changed || imports_changed)
            }) {
                Ok(true) => self.core.lock().expect("state lock").saved_data(
                    saved_status(store)
                        .unwrap_or_else(|message| SavedData::Unavailable { message }),
                ),
                Ok(false) => (),
                Err(message) => self
                    .core
                    .lock()
                    .expect("state lock")
                    .saved_data(SavedData::Unavailable { message }),
            }
        }
        if let (Some(refresh), Some(store)) = (&mut self.catalog, &mut self.storage) {
            let (ready, stopped) = {
                let snapshot = self.core.lock().expect("state lock");
                (snapshot.shell_ready(), snapshot.stopped)
            };
            refresh.tick(
                store,
                now.duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                ready,
                stopped,
            );
            self.core
                .lock()
                .expect("state lock")
                .catalog(CatalogStatus {
                    revision: refresh
                        .cache
                        .catalog
                        .as_ref()
                        .map(|c| c.catalog_revision.clone()),
                    release_count: refresh.cache.catalog.as_ref().map_or(0, |c| {
                        c.releases().filter(|(_, r)| !r.withdrawn).count() as u32
                    }),
                    checking: refresh.checking,
                    last_checked: refresh.cache.last_checked.map(|t| t.to_string()),
                    last_success: refresh.cache.last_success.map(|t| t.to_string()),
                    error: refresh.cache.error.clone(),
                });
        }
        if game::observation_expired(self.observed, now)
            || self.validated.elapsed() >= Duration::from_secs(30)
        {
            self.view.running = Running::Unknown;
            self.core
                .lock()
                .expect("state lock")
                .game(self.view.clone());
            revalidate(&mut self.view, &self.selected);
            self.validated = Instant::now();
        }
        self.observed = now;
        let processes = windows_game::observe();
        self.view.running = self
            .view
            .selected
            .as_ref()
            .map_or(Running::Unknown, |game| {
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
                self.view
                    .selected
                    .as_ref()
                    .is_some_and(|g| std::path::Path::new(&g.executable) == p.path)
            })
        });
        let observation = self.view.selected.as_ref().map(|game| {
            (
                game.executable.clone(),
                game.build.clone(),
                self.view.running.clone(),
                process.map(|p| (p.pid, p.start.clone())),
            )
        });
        if observation != self.recovery_observation
            && self.view.running == Running::Stopped
            && let (Some(store), Some(game)) = (self.storage.as_mut(), self.view.selected.as_ref())
        {
            match deployment::recover(store, game) {
                Ok(true) => {
                    self.view.message =
                        "Previous bootstrap deployment restored after interruption.".into()
                }
                Ok(false) => {}
                Err(message) => self.view.error = message,
            }
        }
        if observation != self.recovery_observation {
            self.session.wait = None;
            self.session.prepared = match (self.storage.as_ref(), self.view.selected.as_ref()) {
                (Some(store), Some(game)) => match deployment::prepared_activation(store, game) {
                    Ok(value) => value,
                    Err(error) => {
                        self.view.launch = LaunchView::new(Phase::Failed, &error);
                        None
                    }
                },
                _ => None,
            };
            if self.view.running == Running::Stopped
                && self.session.requested.is_none()
                && self.view.launch.phase != Phase::Failed
            {
                self.view.launch = if self.session.prepared.is_some() {
                    LaunchView::new(
                        Phase::Ready,
                        "Runtime installed. The latest setup will be checked before launch.",
                    )
                } else {
                    LaunchView::new(
                        Phase::SetupRequired,
                        "Starframe will prepare the game runtime automatically.",
                    )
                };
            }
        }
        let observed_launch = process.map(|process| {
            if let (Some(activation), Some(game)) = (&self.session.prepared, &self.view.selected) {
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
            }
        });
        self.session.observe(
            &mut self.view.launch,
            &self.view.running,
            observed_launch,
            Instant::now(),
        );
        if let (Some(store), Some(game)) = (self.storage.as_mut(), self.view.selected.as_ref()) {
            let revision = store.load().map(|r| r.revision).ok();
            let attempt = (game.executable.clone(), revision, self.view.running.clone());
            let differs = self.session.prepared.as_ref().is_none_or(|activation| {
                activation["deploymentRevision"].as_str()
                    != revision.map(|r| r.to_string()).as_deref()
            });
            if differs
                && self.view.running != Running::Stopped
                && let Some(revision) = revision
            {
                self.view.launch.details = vec![format!(
                    "Waiting for game to close. Saved collection revision {} will apply after exit.",
                    revision
                )];
            }
            if (differs || self.last_auto_attempt.is_none())
                && self.view.running == Running::Stopped
                && self.session.requested.is_none()
                && self.last_auto_attempt.as_ref() != Some(&attempt)
            {
                self.last_auto_attempt = Some(attempt);
                self.writing.store(true, Ordering::SeqCst);
                self.view.launch = LaunchView::new(
                    Phase::Preparing,
                    "Preparing the game runtime and saved mod setup…",
                );
                self.core
                    .lock()
                    .expect("state lock")
                    .game(self.view.clone());
                match prepare(&self.app, &self.core, store, game, false) {
                    Ok(value) => {
                        self.session.prepared = Some(value);
                        self.view.launch = LaunchView::new(
                            Phase::Ready,
                            "The saved mod setup is ready to launch.",
                        );
                    }
                    Err(error) => self.view.launch = LaunchView::new(Phase::Failed, &error),
                }
                self.writing.store(false, Ordering::SeqCst);
                if self.core.lock().expect("state lock").stopped {
                    self.app.exit(0);
                    return true;
                }
            }
        }
        self.recovery_observation = observation;
        false
    }
}
