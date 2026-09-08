use super::*;

impl Worker {
    pub(super) fn handle(&mut self, request: Request) {
        if let Request::Sharing(action, reply) = request {
            let result = (|| {
                if self.core.lock().expect("state lock").stopped {
                    return Err("Starframe is closing.".into());
                }
                let store = self.storage.as_mut().ok_or("Saved data is unavailable.")?;
                let result = starframe::sharing::action(store, action)?;
                self.core.lock().expect("state lock").saved_data(
                    saved_status(store)
                        .unwrap_or_else(|message| SavedData::Unavailable { message }),
                );
                Ok(result)
            })();
            let _ = reply.send(result);
            return;
        }
        if let Request::Mod(action, reply) = request {
            let result = (|| {
                if self.core.lock().expect("state lock").stopped {
                    return Err("Starframe is closing.".into());
                }
                if let starframe::mods::Action::Uninstall { reference, .. } = &action
                    && self
                        .packages
                        .as_ref()
                        .ok()
                        .and_then(|q| q.as_ref())
                        .is_some_and(|q| q.busy_hash(&reference.hash))
                {
                    return Err("This package is being prepared. Finish or cancel its download before uninstalling.".into());
                }
                let store = self.storage.as_mut().ok_or("Saved data is unavailable.")?;
                let result = starframe::mods::action(store, action)?;
                self.core.lock().expect("state lock").saved_data(
                    saved_status(store)
                        .unwrap_or_else(|message| SavedData::Unavailable { message }),
                );
                Ok(result)
            })();
            let _ = reply.send(result);
            return;
        }
        if let Request::Package(action, reply) = request {
            let result = (|| {
                if self.core.lock().expect("state lock").stopped {
                    return Err("Starframe is closing.".into());
                }
                let queue = self
                    .packages
                    .as_mut()
                    .map_err(|e| e.clone())?
                    .as_mut()
                    .ok_or("Package storage is unavailable.")?;
                let store = self.storage.as_mut().ok_or("Saved data is unavailable.")?;
                match action {
                    packages::Action::Prepare {
                        request_id,
                        release_id,
                    } => {
                        queue.start(store, &request_id, &release_id)?;
                    }
                    packages::Action::Cancel { operation_id } => {
                        queue.cancel(store, &operation_id)?
                    }
                    packages::Action::List => (),
                }
                queue.operations(store)
            })();
            let _ = reply.send(result);
            return;
        }
        self.recovery_observation = None;
        self.view.error.clear();
        self.view.running = Running::Unknown;
        self.view.busy = true;
        self.core
            .lock()
            .expect("state lock")
            .game(self.view.clone());
        if matches!(
            request,
            Request::Setup | Request::Launch | Request::RemoveRuntime
        ) {
            self.view.launch = LaunchView::new(
                Phase::Preparing,
                if matches!(request, Request::RemoveRuntime) {
                    "Removing the Starframe runtime…"
                } else {
                    "Preparing mods…"
                },
            );
            self.core
                .lock()
                .expect("state lock")
                .game(self.view.clone());
            let result = (|| {
                if self.core.lock().expect("state lock").stopped {
                    return Err("Starframe closed before setup started.".into());
                }
                let game = self
                    .view
                    .selected
                    .as_ref()
                    .ok_or("Choose an available game installation first.")?;
                let store = self
                    .storage
                    .as_mut()
                    .ok_or("Saved data is unavailable. Setup was not changed.")?;
                if self.session.requested.is_some() {
                    return Err("A launch request is still waiting for its game process.".into());
                }
                if matches!(request, Request::RemoveRuntime) {
                    deployment::remove(store, game)?;
                    self.session.prepared = None;
                    return Ok(LaunchView::new(
                        Phase::SetupRequired,
                        "Starframe runtime removed. Mod settings and unowned files were retained.",
                    ));
                }
                let dispatch = matches!(request, Request::Launch);
                self.session.prepared =
                    Some(prepare(&self.app, &self.core, store, game, dispatch)?);
                if dispatch {
                    self.session.requested = Some(Instant::now());
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
            self.view.launch =
                result.unwrap_or_else(|error: String| LaunchView::new(Phase::Failed, &error));
            self.writing.store(false, Ordering::SeqCst);
            self.busy.store(false, Ordering::SeqCst);
            if self.core.lock().expect("state lock").stopped {
                self.app.exit(0);
                return;
            }
            return;
        }
        let chosen = match request {
            Request::Setup
            | Request::Launch
            | Request::RemoveRuntime
            | Request::Mod(..)
            | Request::Sharing(..)
            | Request::Package(..) => unreachable!(),
            Request::Discover => {
                discover(&mut self.view);
                revalidate(&mut self.view, &self.selected);
                Ok(None)
            }
            Request::Select(id) => self
                .view
                .candidates
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| "This discovery result has expired. Find the game again.".to_owned())
                .and_then(|c| game::inspect(&PathBuf::from(&c.path)))
                .map(Some),
            Request::Picked(result) => {
                result.and_then(|path| path.map(|path| game::inspect(&path)).transpose())
            }
        };
        match chosen {
            Ok(Some(item)) => {
                let save = self
                    .storage
                    .as_mut()
                    .ok_or_else(|| {
                        "Saved data is unavailable; the game selection was not changed.".to_owned()
                    })
                    .and_then(|store| {
                        store
                            .select_game(&item.id, &item.path)
                            .map_err(|e| e.to_string())
                    });
                match save {
                    Ok(_) => {
                        self.selected = Some((item.id.clone(), item.path.clone()));
                        self.view.selected_path = Some(item.path.clone());
                        self.view.selected = Some(item);
                        self.view.launch = LaunchView::default();
                        self.session.reset();
                        self.view.message =
                            "Game location saved. Game files were not changed.".into();
                        if let Some(store) = &self.storage {
                            self.core.lock().expect("state lock").saved_data(
                                saved_status(store)
                                    .unwrap_or_else(|message| SavedData::Unavailable { message }),
                            );
                        }
                    }
                    Err(e) => self.view.error = e,
                }
            }
            Ok(None) => {}
            Err(e) => self.view.error = e,
        }
        self.busy.store(false, Ordering::SeqCst);
    }
}
