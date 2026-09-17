use super::*;
use starframe::updates::{self, Action, Phase, Saved, Schedule, View};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

enum ResultMessage {
    Checked(Result<Option<updates::Release>, updates::CheckError>),
    Downloaded(Result<(Box<Update>, Vec<u8>), updates::CheckError>),
}
pub(super) struct Updates {
    saved: Saved,
    pub view: View,
    schedule: Schedule,
    pending: Option<(
        tauri::async_runtime::JoinHandle<()>,
        mpsc::Receiver<ResultMessage>,
    )>,
    received: Arc<std::sync::atomic::AtomicU64>,
    ready: Option<(Box<Update>, Vec<u8>)>,
}
impl Drop for Updates {
    fn drop(&mut self) {
        if let Some((task, _)) = self.pending.take() {
            task.abort();
        }
    }
}
impl Updates {
    pub fn new(storage: &Storage) -> Result<Self, String> {
        let mut saved = storage.update_preferences().map_err(|e| e.to_string())?;
        if saved.release.as_ref().is_some_and(|r| {
            semver::Version::parse(&r.version).unwrap()
                <= semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
                || (saved.channel == updates::Channel::Stable
                    && !semver::Version::parse(&r.version).unwrap().pre.is_empty())
        }) {
            saved.release = None;
        }
        let mut result = Self {
            saved,
            view: View::default(),
            schedule: Schedule::default(),
            pending: None,
            received: Default::default(),
            ready: None,
        };
        result.publish_saved();
        result.view.message = "Updates have not been checked in this session.".into();
        Ok(result)
    }
    fn publish_saved(&mut self) {
        self.view.channel = self.saved.channel;
        self.view.release = self.saved.release.clone();
        self.view.last_success = self.saved.last_success.map(|n| n.to_string());
        self.view.dismissed = self
            .saved
            .release
            .as_ref()
            .is_some_and(|r| self.saved.dismissed.as_ref() == Some(&r.version));
    }
    fn save(&mut self, storage: &mut Storage, saved: Saved) -> Result<(), String> {
        storage
            .save_update_preferences(&saved)
            .map_err(|e| e.to_string())?;
        self.saved = saved;
        self.publish_saved();
        Ok(())
    }
    fn check(&mut self, now: u64, manual: bool) {
        if self.view.phase != Phase::Idle || !self.schedule.start(now, manual, true, false) {
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let channel = self.saved.channel;
        let task = tauri::async_runtime::spawn(async move {
            let _ = sender.send(ResultMessage::Checked(
                updates::discover(channel, now).await,
            ));
        });
        self.pending = Some((task, receiver));
        self.view.phase = Phase::Checking;
        self.view.message = "Checking GitHub releases…".into();
    }
    pub fn action(
        &mut self,
        action: Action,
        app: &tauri::AppHandle,
        storage: &mut Storage,
        now: u64,
    ) -> Result<(), String> {
        match action {
            Action::Check => self.check(now, true),
            Action::ViewRelease => {
                let release = self
                    .saved
                    .release
                    .as_ref()
                    .ok_or("No update is available.")?;
                app.opener()
                    .open_url(release.url(), None::<&str>)
                    .map_err(|_| "Could not open the release page.")?;
            }
            Action::Later { version } => {
                if self
                    .saved
                    .release
                    .as_ref()
                    .is_none_or(|r| r.version != version)
                {
                    return Err("The available release changed. Review it again.".into());
                }
                let mut saved = self.saved.clone();
                saved.dismissed = Some(version);
                self.save(storage, saved)?;
            }
            Action::Channel { channel } => {
                if matches!(
                    self.view.phase,
                    Phase::Downloading | Phase::Waiting | Phase::Installing
                ) {
                    return Err("Cancel this update before changing channels.".into());
                }
                let mut saved = self.saved.clone();
                saved.channel = channel;
                saved.release = None;
                saved.last_success = None;
                self.save(storage, saved)?;
                if let Some((task, _)) = self.pending.take() {
                    task.abort();
                }
                self.schedule.in_flight = false;
                self.schedule.next = 0;
                self.view.phase = Phase::Idle;
                self.check(now, true);
            }
            Action::Cancel => {
                if self.view.phase == Phase::Installing {
                    return Err("The installer is starting.".into());
                }
                if let Some((task, _)) = self.pending.take() {
                    task.abort();
                }
                self.ready = None;
                self.schedule.in_flight = false;
                self.schedule.next = now.saturating_add(updates::INTERVAL);
                self.view.phase = Phase::Idle;
                self.view.message = "Update cancelled. The installed version is unchanged.".into();
            }
            Action::Install { version } => {
                if self.view.phase != Phase::Idle {
                    return Err("Wait for the current update operation.".into());
                }
                if !self.schedule.retry_allowed(now) {
                    return Err(
                        "The last check failed. Wait for the scheduled retry before updating."
                            .into(),
                    );
                }
                let release = self
                    .saved
                    .release
                    .clone()
                    .filter(|r| r.version == version)
                    .ok_or("The available release changed. Review it again.")?;
                let app = app.clone();
                let channel = self.saved.channel;
                let received = self.received.clone();
                received.store(0, Ordering::SeqCst);
                let (sender, receiver) = mpsc::sync_channel(1);
                let task = tauri::async_runtime::spawn(async move {
                    let result = async {
                        let current = updates::discover(channel, now).await.map_err(|mut error| {
                            error.retry_after = Some(error.retry_after.unwrap_or(now.saturating_add(updates::INTERVAL)));
                            error
                        })?;
                        if current.as_ref().is_none_or(|r| r.version != release.version) { return Err("The release changed or was removed. Check for updates again.".into()); }
                        let builder = app.updater_builder();
                        #[cfg(debug_assertions)]
                        let builder = if let Some(fixture) = updates::fixture()? { builder.pubkey(fixture.pubkey) } else { builder };
                        let mut update = builder
                            .target("windows-x86_64-nsis")
                            .endpoints(vec![updates::endpoint(&release.manifest_url())?.parse().map_err(|_| "Invalid update endpoint.")?]).map_err(|e| e.to_string())?
                            .timeout(Duration::from_secs(120))
                            .configure_client(|client| client.https_only(!updates::fixture_enabled()).redirect(reqwest::redirect::Policy::limited(5)))
                            .build().map_err(|e| e.to_string())?
                            .check().await.map_err(|_| "Could not read the update information.")?
                            .ok_or("This release is no longer an eligible update.")?;
                        if update.version != release.version || update.download_url.as_str() != release.installer_url() {
                            return Err("Update metadata does not match the selected official release.".into());
                        }
                        update.download_url = updates::endpoint(&release.installer_url())?.parse().map_err(|_| "Invalid download URL.")?;
                        let (limit, mut exceeded) = tokio::sync::watch::channel(false);
                        let downloaded = update.download(move |bytes, total| {
                            let count = received.fetch_add(bytes as u64, Ordering::SeqCst) + bytes as u64;
                            if count > 256 * 1024 * 1024 || total.is_some_and(|n| n > 256 * 1024 * 1024) { let _ = limit.send(true); }
                        }, || {});
                        let bytes = tokio::select! {
                            biased;
                            _ = exceeded.wait_for(|value| *value) => return Err("The update exceeds the supported download size.".into()),
                            result = downloaded => result.map_err(|_| "The update download or signature verification failed. Nothing was installed. Retry the check or use the official release page.")?,
                        };
                        if bytes.len() > 256 * 1024 * 1024 { return Err("The update exceeds the supported download size.".into()); }
                        Ok((Box::new(update), bytes))
                    }.await;
                    let _ = sender.send(ResultMessage::Downloaded(result));
                });
                self.pending = Some((task, receiver));
                self.view.phase = Phase::Downloading;
                self.view.error = None;
                self.view.message =
                    "Checking the selected release, then downloading and verifying its installer…"
                        .into();
            }
        }
        Ok(())
    }
    pub fn tick(&mut self, storage: &mut Storage, now: u64, shell_ready: bool) {
        self.view.received = self.received.load(Ordering::SeqCst).to_string();
        let result = self
            .pending
            .as_ref()
            .and_then(|(_, receiver)| match receiver.try_recv() {
                Ok(value) => Some(value),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(_) => Some(ResultMessage::Downloaded(Err(
                    "The update worker stopped. Try again.".into(),
                ))),
            });
        if let Some(result) = result {
            self.pending.take();
            self.view.phase = Phase::Idle;
            match result {
                ResultMessage::Checked(result) => {
                    self.schedule.complete(
                        now,
                        result.is_ok(),
                        result.as_ref().err().and_then(|e| e.retry_after),
                    );
                    match result {
                        Ok(release) => {
                            let mut saved = self.saved.clone();
                            saved.release = release;
                            saved.last_success = Some(now);
                            self.view.error = self.save(storage, saved).err();
                            self.view.message = if self.saved.release.is_some() {
                                "An update is available."
                            } else {
                                "No newer release is available on this channel."
                            }
                            .into();
                            if self.view.error.is_some() {
                                self.view.message = "Update information could not be saved. The previous notice was retained.".into();
                            }
                        }
                        Err(error) => {
                            self.view.error = Some(error.message);
                            self.view.message = "Update status could not be confirmed. Previously found releases remain visible.".into();
                        }
                    }
                }
                ResultMessage::Downloaded(Ok(download)) => {
                    self.ready = Some(download);
                    self.view.phase = Phase::Waiting;
                    self.view.message =
                        "Installer verified. Waiting for the game and active file work to finish…"
                            .into();
                }
                ResultMessage::Downloaded(Err(error)) => {
                    if error.retry_after.is_some() {
                        self.schedule.complete(now, false, error.retry_after);
                    }
                    self.view.error = Some(error.message);
                    self.view.message = "Nothing was installed.".into();
                }
            }
        }
        if shell_ready {
            self.check(now, false);
        }
    }
    pub fn waiting(&self) -> bool {
        self.ready.is_some()
    }
    pub fn install(&mut self) -> Result<(), String> {
        let (update, bytes) = self.ready.take().ok_or("No verified installer is ready.")?;
        update
            .install(bytes)
            .map_err(|e| format!("Could not start the installer: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_preference_save_keeps_the_confirmed_channel_and_notice() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).unwrap();
        let mut updates = Updates::new(&store).unwrap();
        let previous = updates.view.clone();
        let mut candidate = updates.saved.clone();
        candidate.channel = updates::Channel::Stable;
        candidate.release = Some(updates::Release {
            version: "0.9.0".into(),
            notes: "Fixture".into(),
        });
        candidate.dismissed = Some("0.9.0".into());
        let other = rusqlite::Connection::open(root.path().join("sqlite/state.db")).unwrap();
        other.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(updates.save(&mut store, candidate.clone()).is_err());
        assert_eq!(updates.view, previous);
        assert_eq!(updates.saved, store.update_preferences().unwrap());
        other.execute_batch("ROLLBACK").unwrap();
        updates.save(&mut store, candidate.clone()).unwrap();
        assert_eq!(updates.saved, candidate);
        assert!(updates.view.dismissed);
        assert_eq!(updates.view.channel, updates::Channel::Stable);
    }
}
