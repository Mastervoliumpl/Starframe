use super::{
    Cancel, Kind, Operation, Packages, RegistryActive, RegistryEvent, RegistryRequest, Result,
    Status, read_file,
};
use crate::{
    filesystem::pin,
    registry::{Client, Error as RegistryError, Session, VerifiedArchive, trust::DownloadIdentity},
    space,
    storage::Storage,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};
use time::OffsetDateTime;
use tokio::sync::watch;
use uuid::Uuid;

fn archive_dir(root: &Path) -> Result<(PathBuf, File)> {
    let _root = pin(root)?;
    let dir = root.join("registry-archives");
    match fs::create_dir(&dir) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.to_string()),
    }
    let handle = pin(&dir)?;
    Ok((dir, handle))
}

fn archive_path(dir: &Path, identity: &DownloadIdentity) -> PathBuf {
    dir.join(format!("{}.zip", identity.reference.sha256.as_str()))
}

fn stage_path(dir: &Path, operation_id: Uuid) -> PathBuf {
    dir.join(format!("{operation_id}.part"))
}

fn discard_stage(root: &Path, operation_id: Uuid) -> Result<()> {
    let (dir, _pin) = archive_dir(root)?;
    let path = stage_path(&dir, operation_id);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
        Ok(_) => (),
    }
    crate::filesystem::regular_metadata(&path, false)?;
    fs::remove_file(path).map_err(|e| e.to_string())
}

fn recover_stages(root: &Path, active: &[Uuid]) -> Result<()> {
    let (dir, _pin) = archive_dir(root)?;
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let Some(id) = name
            .to_str()
            .and_then(|name| name.strip_suffix(".part"))
            .and_then(|id| Uuid::parse_str(id).ok())
        else {
            continue;
        };
        if !active.contains(&id) {
            crate::filesystem::regular_metadata(&entry.path(), false)?;
            fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn verify(path: &Path, identity: &DownloadIdentity, cancel: Option<&Cancel>) -> Result<()> {
    let mut file = read_file(path)?;
    if file.metadata().map_err(|e| e.to_string())?.len() != identity.bytes {
        return Err("Registry archive size changed. Retry the exact download.".into());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if let Some(cancel) = cancel {
            cancel.check()?;
        }
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if format!("{:x}", digest.finalize()) != identity.reference.sha256.as_str() {
        return Err("Registry archive hash changed. Retry the exact download.".into());
    }
    Ok(())
}

#[cfg(test)]
fn cached(root: &Path, identity: &DownloadIdentity) -> Result<Option<PathBuf>> {
    let (dir, _pin) = archive_dir(root)?;
    let path = archive_path(&dir, identity);
    if !path.exists() {
        return Ok(None);
    }
    verify(&path, identity, None)?;
    Ok(Some(path))
}

pub(super) fn begin(root: &Path, operation_id: Uuid) -> Result<(PathBuf, File, File)> {
    let (dir, pin) = archive_dir(root)?;
    let path = stage_path(&dir, operation_id);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    Ok((path, file, pin))
}

pub(super) fn promote(
    root: &Path,
    identity: &DownloadIdentity,
    verified: &VerifiedArchive,
    operation_id: Uuid,
    cancel: &Cancel,
    auth_abort: &watch::Receiver<bool>,
) -> Result<PathBuf> {
    if !verified.matches_identity(identity) {
        return Err("Registry transfer identity changed.".into());
    }
    let (dir, _pin) = archive_dir(root)?;
    let stage = stage_path(&dir, operation_id);
    verify(&stage, identity, Some(cancel))?;
    cancel.check()?;
    if *auth_abort.borrow() {
        return Err("Sign-in changed during the download. The archive was not saved.".into());
    }
    let target = archive_path(&dir, identity);
    if target.exists() {
        verify(&target, identity, Some(cancel))?;
        cancel.check()?;
        if *auth_abort.borrow() {
            return Err("Sign-in changed during the download. The archive was not saved.".into());
        }
        fs::remove_file(stage).map_err(|e| e.to_string())?;
    } else {
        fs::rename(stage, &target).map_err(|e| e.to_string())?;
    }
    Ok(target)
}

fn network_error(error: RegistryError) -> String {
    use reqwest::StatusCode;
    let status = match error {
        RegistryError::Server(status, _) | RegistryError::Http(status) => Some(status),
        RegistryError::Cancelled => return "Registry download cancelled.".into(),
        RegistryError::Integrity => {
            return "Downloaded bytes failed the approved SHA-256 check. Retry this exact release."
                .into();
        }
        RegistryError::Protocol => {
            return "The registry returned invalid download data. Retry later.".into();
        }
        RegistryError::InvalidToken => return "Sign in again to download this release.".into(),
        _ => return "The registry connection failed. Retry this exact release later.".into(),
    };
    match status {
        Some(StatusCode::UNAUTHORIZED) => "Sign in again to download this release.".into(),
        Some(StatusCode::FORBIDDEN) => {
            "This release or account is not permitted to download. Refresh its status.".into()
        }
        Some(StatusCode::NOT_FOUND) => {
            "This registry release or grant was not found. Refresh its status.".into()
        }
        Some(StatusCode::CONFLICT) => {
            "The release or security revision changed. Refresh before retrying.".into()
        }
        Some(StatusCode::GONE) => {
            "This release or grant is no longer available. Refresh its status.".into()
        }
        Some(StatusCode::TOO_MANY_REQUESTS) => {
            "The registry asked Starframe to wait before retrying.".into()
        }
        Some(StatusCode::SERVICE_UNAVAILABLE) => {
            "The registry is unavailable. Retry this exact release later.".into()
        }
        Some(status) => format!("Registry download failed with HTTP {status}. Retry later."),
        None => unreachable!(),
    }
}

impl Packages {
    pub fn resume_registry_receipts(
        &mut self,
        storage: &Storage,
        client: Client,
        session: Session,
        bearer: String,
        auth_cancel: watch::Receiver<bool>,
    ) -> Result<usize> {
        if bearer.is_empty() || *auth_cancel.borrow() {
            return Err("Sign in again to confirm pending download receipts.".into());
        }
        let mut started = 0;
        for claim in storage
            .pending_registry_receipts(session.account_id)
            .map_err(|e| e.to_string())?
        {
            if self.registry_receipts.contains_key(&claim.download_id)
                || self.registry_active.values().any(|active| {
                    active
                        .claim
                        .as_ref()
                        .is_some_and(|current| current.download_id == claim.download_id)
                })
            {
                continue;
            }
            let (sender, result) = mpsc::channel();
            let current_claim = claim.clone();
            let current_client = client.clone();
            let current_session = session.clone();
            let current_bearer = bearer.clone();
            let cancel = auth_cancel.clone();
            let retry_cancel = Cancel::default();
            let worker_cancel = retry_cancel.clone();
            let worker = tauri::async_runtime::spawn(async move {
                let outcome = tokio::select! {
                    result = current_client.retry_receipt(
                        &current_claim, &current_session, &current_bearer, cancel
                    ) => result.map(|_| ()).map_err(network_error),
                    _ = worker_cancel.cancelled() => Err("Starframe closed before the receipt was confirmed.".into()),
                };
                let _ = sender.send(outcome);
            });
            self.registry_receipts.insert(
                claim.download_id,
                super::RegistryReceiptRetry {
                    claim,
                    result,
                    worker,
                    cancel: retry_cancel,
                },
            );
            started += 1;
        }
        Ok(started)
    }

    pub fn start_registry(
        &mut self,
        storage: &mut Storage,
        request_id: &str,
        request: RegistryRequest,
    ) -> Result<Operation> {
        self.start_registry_inner(storage, request_id, request, None)
    }

    pub fn start_registry_install(
        &mut self,
        storage: &mut Storage,
        request_id: &str,
        request: RegistryRequest,
        display: crate::storage::RegistryDisplay,
    ) -> Result<Operation> {
        if !display.valid() {
            return Err("The registry display metadata is invalid.".into());
        }
        self.start_registry_inner(storage, request_id, request, Some(display))
    }

    fn start_registry_inner(
        &mut self,
        storage: &mut Storage,
        request_id: &str,
        request: RegistryRequest,
        install: Option<crate::storage::RegistryDisplay>,
    ) -> Result<Operation> {
        let RegistryRequest {
            mod_id,
            release_id,
            root_public,
            client,
            session,
            bearer,
            auth_cancel,
        } = request;
        Uuid::parse_str(request_id).map_err(|_| "Invalid registry request ID.")?;
        if storage
            .package_request(request_id)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("This registry request ID was already used. Start a new request.".into());
        }
        if bearer.is_empty() || *auth_cancel.borrow() {
            return Err("Sign in again to download this release.".into());
        }
        let identity = storage
            .ready_registry_download(&root_public, mod_id, release_id, OffsetDateTime::now_utc())
            .map_err(|e| e.to_string())?;
        if install.is_some() && identity.installation.is_none() {
            return Err("This release has no approved installation declaration. Ask the author for a new declared release.".into());
        }
        if storage
            .pending_removals()
            .map_err(|error| error.to_string())?
            .iter()
            .any(|(hash, _)| hash == identity.reference.sha256.as_str())
        {
            return Err("Finish pending artifact cleanup before installing this release.".into());
        }
        if !self.can_start(identity.reference.sha256.as_str()) {
            return Err(
                "Three downloads are already active, or this archive is being prepared.".into(),
            );
        }
        let root = storage.package_root().to_owned();
        let reserved = self
            .active
            .values()
            .map(|active| active.operation.total_bytes + super::MAX_EXPANDED_BYTES)
            .chain(self.registry_active.values().map(|active| {
                active.operation.total_bytes
                    + if active.install.is_some() {
                        super::MAX_EXPANDED_BYTES
                    } else {
                        0
                    }
            }))
            .sum::<u64>();
        if install.is_some() {
            space::require(&root, reserved + identity.bytes + super::MAX_EXPANDED_BYTES)?;
        }
        let active_ids = self
            .registry_active
            .keys()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect::<Vec<_>>();
        recover_stages(&root, &active_ids)?;
        let operation_id = Uuid::new_v4();
        let mut operation = Operation {
            id: operation_id.to_string(),
            request_id: request_id.into(),
            release_id: release_id.0.to_string(),
            hash: identity.reference.sha256.as_str().into(),
            kind: if install.is_some() {
                Kind::RegistryInstall
            } else {
                Kind::RegistryArchive
            },
            receipt_id: None,
            status: Status::Preparing,
            message: "Downloading the exact signed registry archive.".into(),
            received_bytes: 0,
            total_bytes: identity.bytes,
        };
        let (archive_directory, directory_pin) = archive_dir(&root)?;
        let cache_path = archive_path(&archive_directory, &identity);
        if cache_path.exists() {
            operation.message = "Checking the exact cached archive.".into();
            storage
                .save_package(&operation)
                .map_err(|e| e.to_string())?;
            let cancel = Cancel::default();
            let worker_cancel = cancel.clone();
            let worker_auth = auth_cancel.clone();
            let worker_identity = identity.clone();
            let (sender, result) = mpsc::channel();
            let worker_sender = sender.clone();
            let worker = tauri::async_runtime::spawn_blocking(move || {
                let _directory_pin = directory_pin;
                let outcome = (|| {
                    worker_cancel.check()?;
                    if *worker_auth.borrow() {
                        return Err("Sign-in changed before the cached archive was checked.".into());
                    }
                    verify(&cache_path, &worker_identity, Some(&worker_cancel))?;
                    worker_cancel.check()?;
                    if *worker_auth.borrow() {
                        return Err("Sign-in changed before the cached archive was checked.".into());
                    }
                    Ok(())
                })();
                let _ = worker_sender.send(RegistryEvent::Cached(outcome));
            });
            self.registry_active.insert(
                operation.id.clone(),
                RegistryActive {
                    operation: operation.clone(),
                    identity,
                    root: root_public,
                    client,
                    bearer,
                    cancel,
                    abort: auth_cancel,
                    progress: Arc::new(AtomicU64::new(0)),
                    sender,
                    result,
                    worker,
                    claim: None,
                    install,
                },
            );
            return Ok(operation);
        }
        space::require(&root, reserved + identity.bytes)?;
        let (stage, file, directory_pin) = begin(&root, operation_id)?;
        if let Err(error) = storage.save_package(&operation) {
            let _ = fs::remove_file(stage);
            return Err(error.to_string());
        }
        let cancel = Cancel::default();
        let worker_cancel = cancel.clone();
        let progress = Arc::new(AtomicU64::new(0));
        let worker_progress = progress.clone();
        let (sender, result) = mpsc::channel();
        let worker_sender = sender.clone();
        let worker_client = client.clone();
        let worker_identity = identity.clone();
        let worker_session = session.clone();
        let worker_bearer = bearer.clone();
        let (abort_sender, abort) = watch::channel(false);
        let worker_abort = abort.clone();
        tauri::async_runtime::spawn(async move {
            let mut auth_cancel = auth_cancel;
            tokio::select! {
                _ = worker_cancel.cancelled() => (),
                _ = auth_cancel.changed() => (),
            }
            abort_sender.send_replace(true);
        });
        let worker = tauri::async_runtime::spawn(async move {
            let _directory_pin = directory_pin;
            let mut file = tokio::fs::File::from_std(file);
            let outcome = async {
                let (_, verified) = worker_client
                    .download_with_renewal(
                        &worker_identity,
                        &worker_session,
                        &worker_bearer,
                        &mut file,
                        &worker_progress,
                        worker_abort.clone(),
                    )
                    .await
                    .map_err(network_error)?;
                let current = worker_client
                    .confirm_download_session(&worker_session, &worker_bearer, worker_abort)
                    .await
                    .map_err(network_error);
                Ok((verified, current))
            }
            .await;
            let _ = worker_sender.send(RegistryEvent::Transferred(Box::new(outcome)));
        });
        self.registry_active.insert(
            operation.id.clone(),
            RegistryActive {
                operation: operation.clone(),
                identity,
                root: root_public,
                client,
                bearer,
                cancel,
                abort,
                progress,
                sender,
                result,
                worker,
                claim: None,
                install,
            },
        );
        Ok(operation)
    }

    pub(super) fn poll_registry(&mut self, storage: &mut Storage) -> Result<bool> {
        let mut changed = false;
        for id in self.registry_active.keys().cloned().collect::<Vec<_>>() {
            let mut active = self.registry_active.remove(&id).unwrap();
            active.operation.received_bytes = active.progress.load(Ordering::Relaxed);
            let received = active.result.try_recv();
            let received = if matches!(received, Err(mpsc::TryRecvError::Empty))
                && active.worker.inner().is_finished()
            {
                active.result.try_recv()
            } else {
                received
            };
            let event = match received {
                Ok(event) => event,
                Err(mpsc::TryRecvError::Empty) if !active.worker.inner().is_finished() => {
                    self.registry_active.insert(id, active);
                    continue;
                }
                Err(_)
                    if active.claim.is_some() && active.operation.status == Status::Completed =>
                {
                    RegistryEvent::Receipt(Err(
                        "The receipt worker stopped. The verified archive and claim were retained."
                            .into(),
                    ))
                }
                Err(_) if active.claim.is_some() => RegistryEvent::Promoted(Err(
                    "Archive verification stopped before it was saved. Retry the exact release."
                        .into(),
                )),
                Err(_) => RegistryEvent::Transferred(Box::new(Err(
                    "Registry worker stopped before the archive was saved. Retry.".into(),
                ))),
            };
            changed = true;
            let installation_event = matches!(&event, RegistryEvent::Installed(_));
            match event {
                RegistryEvent::Cached(outcome) => match outcome {
                    Ok(()) => {
                        active.operation.status = Status::Completed;
                        active.operation.received_bytes = active.operation.total_bytes;
                        active.operation.message =
                            "Exact verified archive is cached. No new download receipt was sent."
                                .into();
                    }
                    Err(message) => {
                        active.operation.status = if message == super::CANCELLED {
                            Status::Cancelled
                        } else {
                            Status::Failed
                        };
                        active.operation.message = message;
                    }
                },
                RegistryEvent::Transferred(outcome) if outcome.is_ok() => {
                    let (verified, current) = outcome.unwrap();
                    let (claim, updated) = storage
                        .record_registry_receipt_operation(&verified, &active.operation)
                        .map_err(|e| e.to_string())?;
                    active.claim = Some(claim.clone());
                    active.operation = updated;
                    let outcome = (|| {
                        active.cancel.check()?;
                        if *active.abort.borrow() {
                            return Err(
                                "Sign-in changed during the download. The archive was not saved."
                                    .into(),
                            );
                        }
                        let current = current?;
                        storage
                            .confirm_registry_download(
                                &active.root,
                                &active.identity,
                                OffsetDateTime::now_utc(),
                            )
                            .map_err(|e| e.to_string())?;
                        Ok(current)
                    })();
                    match outcome {
                        Ok(current) => {
                            let root = storage.package_root().to_owned();
                            let identity = active.identity.clone();
                            let operation_id = Uuid::parse_str(&active.operation.id).unwrap();
                            let cancel = active.cancel.clone();
                            let auth_abort = active.abort.clone();
                            let sender = active.sender.clone();
                            active.operation.message =
                                "Checking the complete archive before saving it.".into();
                            storage
                                .save_package(&active.operation)
                                .map_err(|e| e.to_string())?;
                            active.worker = tauri::async_runtime::spawn_blocking(move || {
                                let result = cancel.check().and_then(|_| {
                                    promote(
                                        &root,
                                        &identity,
                                        &verified,
                                        operation_id,
                                        &cancel,
                                        &auth_abort,
                                    )?;
                                    Ok(current)
                                });
                                let _ = sender.send(RegistryEvent::Promoted(result));
                            });
                            self.registry_active.insert(id, active);
                            continue;
                        }
                        Err(message) => {
                            active.operation.status = if message == super::CANCELLED {
                                Status::Cancelled
                            } else {
                                Status::Failed
                            };
                            active.operation.message = message;
                            if let Err(error) = discard_stage(
                                storage.package_root(),
                                Uuid::parse_str(&active.operation.id).unwrap(),
                            ) {
                                active.operation.message.push_str(&format!(
                                    " Staging cleanup needs attention: {error}"
                                ));
                            }
                        }
                    }
                }
                RegistryEvent::Transferred(outcome) => {
                    if let Err(message) = *outcome {
                        active.operation.status = if message == "Registry download cancelled." {
                            Status::Cancelled
                        } else {
                            Status::Failed
                        };
                        active.operation.message = message;
                        if let Err(error) = discard_stage(
                            storage.package_root(),
                            Uuid::parse_str(&active.operation.id).unwrap(),
                        ) {
                            active
                                .operation
                                .message
                                .push_str(&format!(" Staging cleanup needs attention: {error}"));
                        }
                    }
                }
                RegistryEvent::Promoted(result) => match result {
                    Ok(current) => {
                        active.operation.status = Status::Completed;
                        active.operation.message =
                            "Archive verified and saved. Download receipt pending; retry when signed in. Installation remains pending."
                                .into();
                        active.operation.received_bytes = active.operation.total_bytes;
                        storage
                            .save_package(&active.operation)
                            .map_err(|e| e.to_string())?;
                        let claim = active
                            .claim
                            .as_ref()
                            .ok_or("Receipt claim is missing.")?
                            .clone();
                        let client = active.client.clone();
                        let bearer = active.bearer.clone();
                        let abort = active.abort.clone();
                        let sender = active.sender.clone();
                        active.worker = tauri::async_runtime::spawn(async move {
                            let result = client
                                .retry_receipt(&claim, &current, &bearer, abort)
                                .await
                                .map(|_| ())
                                .map_err(network_error);
                            let _ = sender.send(RegistryEvent::Receipt(result));
                        });
                        self.registry_active.insert(id, active);
                        continue;
                    }
                    Err(message) => {
                        active.operation.status = if message == super::CANCELLED {
                            Status::Cancelled
                        } else {
                            Status::Failed
                        };
                        active.operation.message = message;
                        if let Err(error) = discard_stage(
                            storage.package_root(),
                            Uuid::parse_str(&active.operation.id).unwrap(),
                        ) {
                            active
                                .operation
                                .message
                                .push_str(&format!(" Staging cleanup needs attention: {error}"));
                        }
                    }
                },
                RegistryEvent::Receipt(result) => {
                    active.operation.status = Status::Completed;
                    active.operation.received_bytes = active.operation.total_bytes;
                    match result {
                        Ok(()) => {
                            let claim = active.claim.as_ref().ok_or("Receipt claim is missing.")?;
                            active.operation = storage
                                .finish_registry_receipt(claim, Some(&active.operation))
                                .map_err(|e| e.to_string())?
                                .ok_or("Receipt operation is missing.")?;
                        }
                        Err(error) => {
                            active.operation.message = format!(
                                "Verified registry archive saved. Download receipt is pending: {error}"
                            );
                        }
                    }
                }
                RegistryEvent::Installed(result) => {
                    let outcome = result.and_then(|prepared| {
                        active.cancel.check()?;
                        if *active.abort.borrow() { return Err("Sign-in changed before the registry installation was committed.".into()); }
                        let display = active.install.as_ref().ok_or("Registry installation intent is missing.")?;
                        let mut complete = active.operation.clone();
                        complete.status = Status::Completed;
                        complete.received_bytes = complete.total_bytes;
                        complete.message = "Exact registry release installed. Enable it in My Mods or a collection.".into();
                        storage.complete_registry_install(&active.root, &active.identity, display, &prepared, &complete, OffsetDateTime::now_utc()).map_err(|error| error.to_string())?;
                        active.operation = complete;
                        Ok(())
                    });
                    if let Err(message) = outcome {
                        active.operation.status = if message == super::CANCELLED {
                            Status::Cancelled
                        } else {
                            Status::Failed
                        };
                        active.operation.message = message;
                    }
                }
            }
            if !installation_event
                && active.operation.status == Status::Completed
                && active.install.is_some()
            {
                match begin_installation(storage, &mut active) {
                    Ok(()) => {
                        self.registry_active.insert(id, active);
                        continue;
                    }
                    Err(message) => {
                        active.operation.status = if message == super::CANCELLED {
                            Status::Cancelled
                        } else {
                            Status::Failed
                        };
                        active.operation.message = message;
                    }
                }
            }
            storage
                .save_package(&active.operation)
                .map_err(|e| e.to_string())?;
        }
        for id in self.registry_receipts.keys().copied().collect::<Vec<_>>() {
            let retry = self.registry_receipts.remove(&id).unwrap();
            let received = retry.result.try_recv();
            let received = if matches!(received, Err(mpsc::TryRecvError::Empty))
                && retry.worker.inner().is_finished()
            {
                retry.result.try_recv()
            } else {
                received
            };
            match received {
                Ok(Ok(())) => {
                    let operation = storage
                        .package_receipt_operation(retry.claim.download_id)
                        .map_err(|e| e.to_string())?;
                    storage
                        .finish_registry_receipt(&retry.claim, operation.as_ref())
                        .map_err(|e| e.to_string())?;
                    changed = true;
                }
                Ok(Err(error)) => {
                    if let Some(mut operation) = storage
                        .package_receipt_operation(retry.claim.download_id)
                        .map_err(|e| e.to_string())?
                        && operation.status == Status::Completed
                    {
                        operation.message = format!("Download receipt is pending: {error}");
                        storage
                            .save_package(&operation)
                            .map_err(|e| e.to_string())?;
                        changed = true;
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => (),
                Err(mpsc::TryRecvError::Empty) if retry.worker.inner().is_finished() => (),
                Err(mpsc::TryRecvError::Empty) => {
                    self.registry_receipts.insert(id, retry);
                }
            }
        }
        Ok(changed)
    }
}

fn begin_installation(storage: &mut Storage, active: &mut RegistryActive) -> Result<()> {
    active.cancel.check()?;
    if *active.abort.borrow() {
        return Err("Sign-in changed before the registry archive was prepared.".into());
    }
    let release = storage
        .ready_registry_release(
            &active.root,
            active.identity.reference.mod_id,
            active.identity.reference.release_id,
            OffsetDateTime::now_utc(),
        )
        .map_err(|error| error.to_string())?;
    if release.download_identity().as_ref() != Some(&active.identity) {
        return Err("The signed registry installation changed. Refresh the exact release.".into());
    }
    let root = storage.package_root().to_owned();
    let reserved = active.identity.bytes + super::MAX_EXPANDED_BYTES;
    space::require(&root, reserved)?;
    let operation_id = Uuid::parse_str(&active.operation.id)
        .map_err(|_| "Invalid registry installation operation.")?;
    let cancelled = active.cancel.flag.clone();
    let sender = active.sender.clone();
    active.operation.status = Status::Preparing;
    active.operation.message = "Preparing the approved installation declaration.".into();
    storage
        .save_package(&active.operation)
        .map_err(|error| error.to_string())?;
    active.worker = tauri::async_runtime::spawn_blocking(move || {
        let outcome = super::prepare_registry_archive(&root, operation_id, &release, cancelled);
        let _ = sender.send(RegistryEvent::Installed(outcome));
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Client, ModId, ReleaseId, Session};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ring::signature::{Ed25519KeyPair, KeyPair};
    use std::{
        io::Write,
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    };
    use time::format_description::well_known::Rfc3339;

    fn signed_time(value: OffsetDateTime) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            value.year(),
            u8::from(value.month()),
            value.day(),
            value.hour(),
            value.minute(),
            value.second(),
            value.millisecond()
        )
    }

    fn sign(
        mut payload: serde_json::Value,
        key: &Ed25519KeyPair,
        key_id: &serde_json::Value,
        now: OffsetDateTime,
        hours: i64,
    ) -> Vec<u8> {
        payload["issuedAt"] = signed_time(now - time::Duration::minutes(1)).into();
        payload["expiresAt"] = signed_time(now + time::Duration::hours(hours)).into();
        let signature = STANDARD.encode(key.sign(&serde_json::to_vec(&payload).unwrap()).as_ref());
        serde_json::to_vec(&serde_json::json!({"signed":payload,"signatures":[{
            "keyId":key_id,"algorithm":"ed25519","signature":signature
        }]}))
        .unwrap()
    }

    fn signed_store(
        store: &mut Storage,
        archive: &[u8],
    ) -> ([u8; 32], ModId, ReleaseId, DownloadIdentity) {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        let now = OffsetDateTime::now_utc();
        let root = Ed25519KeyPair::from_seed_unchecked(&[7u8; 32]).unwrap();
        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let root_public: [u8; 32] = root.public_key().as_ref().try_into().unwrap();
        let key_id = &fixture["envelope"]["signed"]["keys"][0]["keyId"];
        let root_id = &fixture["envelope"]["signatures"][0]["keyId"];
        let keys = sign(
            fixture["envelope"]["signed"].clone(),
            &root,
            root_id,
            now,
            24,
        );
        let security = sign(
            fixture["securityEnvelope"]["signed"].clone(),
            &online,
            key_id,
            now,
            12,
        );
        let mut release = fixture["releaseEnvelope"]["signed"].clone();
        release["release"]["artifact"]["sha256"] = format!("{:x}", Sha256::digest(archive)).into();
        release["release"]["artifact"]["bytes"] = (archive.len() as u64).into();
        let manifest = sign(release, &online, key_id, now, 12);
        let mod_id = ModId::try_from(1).unwrap();
        let release_id =
            ReleaseId(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        store
            .accept_registry_keys(&keys, &root_public, now)
            .unwrap();
        store
            .accept_registry_security(&security, &root_public, now)
            .unwrap();
        store
            .accept_registry_release(&manifest, &root_public, mod_id, release_id, now)
            .unwrap();
        let identity = store
            .ready_registry_download(&root_public, mod_id, release_id, now)
            .unwrap();
        (root_public, mod_id, release_id, identity)
    }

    fn declared_store(
        store: &mut Storage,
        archive: &[u8],
    ) -> ([u8; 32], ModId, ReleaseId, DownloadIdentity) {
        let (root, mod_id, release_id, _) = signed_store(store, archive);
        let mut envelope: serde_json::Value = serde_json::from_slice(
            &store
                .registry_document(&format!("release:{}", release_id.0))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let plan: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-installation-v1.json"
        ))
        .unwrap();
        envelope["signed"]["schemaVersion"] = 2.into();
        envelope["signed"]["revision"] = 2.into();
        envelope["signed"]["release"]["metadata"]["installation"] =
            plan["cases"]["valid"][2]["plan"].clone();
        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let now = OffsetDateTime::now_utc();
        let manifest = sign(
            envelope["signed"].clone(),
            &online,
            &envelope["signatures"][0]["keyId"],
            now,
            12,
        );
        store
            .accept_registry_release(&manifest, &root, mod_id, release_id, now)
            .unwrap();
        let identity = store
            .ready_registry_download(&root, mod_id, release_id, now)
            .unwrap();
        (root, mod_id, release_id, identity)
    }

    #[tokio::test]
    async fn shared_queue_installs_declared_map_atomically_and_reuses_cache_after_restart() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).unwrap();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (path, bytes) in [
            ("Maps/Example/Example.sanmap", b"inert map".as_slice()),
            ("Maps/Example/Textures/height.png", b"inert asset"),
        ] {
            zip.start_file(path, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        let archive = zip.finish().unwrap().into_inner();
        let (root_public, mod_id, release_id, identity) = declared_store(&mut store, &archive);
        let session_fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/registry-v1.json")).unwrap();
        let mut session = session_fixtures
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["name"] == "session")
            .unwrap()["body"]["data"]
            .clone();
        session["expiresAt"] = (OffsetDateTime::now_utc() + time::Duration::hours(1))
            .format(&Rfc3339)
            .unwrap()
            .into();
        let session: Session = serde_json::from_value(session).unwrap();
        let (origin, thread) = serve_download(&identity, &archive, &session, Some(true));
        let client = Client::new(
            crate::registry::Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true)
                .unwrap(),
        )
        .unwrap();
        let (_auth_sender, auth_cancel) = watch::channel(false);
        let display = crate::storage::RegistryDisplay {
            name: "Fixture map".into(),
            author: "Fixture author".into(),
        };
        let mut queue = Packages::open(&mut store).unwrap();
        let started = crate::backend::registry_install_action(
            &mut store,
            &mut queue,
            &Uuid::new_v4().to_string(),
            RegistryRequest {
                mod_id,
                release_id,
                root_public,
                client: client.clone(),
                session: session.clone(),
                bearer: "fixture-token".into(),
                auth_cancel: auth_cancel.clone(),
            },
            display.clone(),
        )
        .unwrap();
        let operation_id = started[0].id.clone();
        for _ in 0..150 {
            crate::backend::poll_packages(&mut store, &mut queue).unwrap();
            if !queue.busy() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!queue.busy());
        let complete = store
            .package_operations()
            .unwrap()
            .into_iter()
            .find(|operation| operation.id == operation_id)
            .unwrap();
        assert_eq!(complete.status, Status::Completed, "{}", complete.message);
        assert_eq!(complete.kind, Kind::RegistryInstall);
        assert!(complete.message.contains("installed"));
        assert!(
            store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .is_empty()
        );
        let entries = store.installed_registry_releases().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].reference, *identity.reference());
        assert_eq!(entries[0].display, display);
        assert!(
            store
                .prepared_artifact(identity.reference.sha256.as_str())
                .unwrap()
                .is_some()
        );
        assert_eq!(thread.join().unwrap().len(), 4);
        assert!(super::super::remove_artifact(&store, identity.reference.sha256.as_str()).is_err());
        drop(queue);
        drop(store);
        let mut store = Storage::open(root.path()).unwrap();
        assert_eq!(store.installed_registry_releases().unwrap(), entries);
        let mut queue = Packages::open(&mut store).unwrap();
        queue
            .start_registry_install(
                &mut store,
                &Uuid::new_v4().to_string(),
                RegistryRequest {
                    mod_id,
                    release_id,
                    root_public,
                    client,
                    session,
                    bearer: "fixture-token".into(),
                    auth_cancel,
                },
                display,
            )
            .unwrap();
        for _ in 0..150 {
            queue.poll(&mut store).unwrap();
            if !queue.busy() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!queue.busy());
        assert_eq!(
            store.package_operations().unwrap()[0].status,
            Status::Completed
        );
        assert_eq!(store.installed_registry_releases().unwrap(), entries);
    }

    fn request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0u8; 4096];
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                    })
                    .unwrap_or(0);
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&bytes).to_string()
    }

    fn serve_download(
        identity: &DownloadIdentity,
        archive: &[u8],
        session: &Session,
        receipt_ok: Option<bool>,
    ) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let reference = identity.reference.clone();
        let release_id = identity.reference.release_id;
        let hash = identity.reference.sha256.as_str().to_owned();
        let archive = archive.to_vec();
        let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(5))
            .format(&Rfc3339)
            .unwrap();
        let session = session.clone();
        let thread = thread::spawn(move || {
            let mut paths = Vec::new();
            for step in 0..if receipt_ok.is_some() { 4 } else { 3 } {
                let (mut stream, _) = listener.accept().unwrap();
                let incoming = request(&mut stream);
                paths.push(incoming.lines().next().unwrap().to_owned());
                let (status, body, headers) = match step {
                    0 => (
                        "201 Created",
                        serde_json::json!({"apiVersion":1,"data":{
                            "downloadId":"33333333-3333-4333-8333-333333333333",
                            "reference":reference,"bytes":archive.len(),
                            "metadataRevision":1,"securityRevision":1,
                            "contentPath":"/v1/downloads/33333333-3333-4333-8333-333333333333/content",
                            "expiresAt":expires
                        }})
                        .to_string()
                        .into_bytes(),
                        String::new(),
                    ),
                    1 => (
                        "200 OK",
                        archive.clone(),
                        format!("ETag: \"sha256-{hash}\"\r\nAccept-Ranges: bytes\r\n"),
                    ),
                    2 => (
                        "200 OK",
                        serde_json::json!({"apiVersion":1,"data":session})
                            .to_string()
                            .into_bytes(),
                        String::new(),
                    ),
                    _ if receipt_ok == Some(true) => (
                        "200 OK",
                        serde_json::json!({"apiVersion":1,"data":{
                            "releaseId":release_id.0,"counted":true
                        }})
                        .to_string()
                        .into_bytes(),
                        String::new(),
                    ),
                    _ => (
                        "503 Service Unavailable",
                        serde_json::json!({"apiVersion":1,"error":{
                            "code":"service_unavailable","message":"Fixture outage",
                            "requestId":"55555555-5555-4555-8555-555555555555",
                            "retryAfterSeconds":null,"problems":[]
                        }})
                        .to_string()
                        .into_bytes(),
                        String::new(),
                    ),
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
            paths
        });
        (origin, thread)
    }

    #[test]
    fn restart_cleanup_removes_only_inactive_registry_stages() {
        let root = tempfile::tempdir().unwrap();
        let _store = Storage::open(root.path()).unwrap();
        let abandoned = Uuid::new_v4();
        let active = Uuid::new_v4();
        let (abandoned_path, mut file, pin) = begin(root.path(), abandoned).unwrap();
        file.write_all(b"partial").unwrap();
        drop(file);
        drop(pin);
        let (active_path, file, pin) = begin(root.path(), active).unwrap();
        drop(file);
        drop(pin);
        let note = root.path().join("registry-archives").join("notes.txt");
        fs::write(&note, b"unowned").unwrap();
        recover_stages(root.path(), &[active]).unwrap();
        assert!(!abandoned_path.exists());
        assert!(active_path.exists());
        assert_eq!(fs::read(note).unwrap(), b"unowned");
    }

    #[tokio::test]
    async fn queue_saves_exact_archive_and_receipt_without_installing() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).unwrap();
        let archive = b"PK\x03\x04one verified registry archive";
        let (root_public, mod_id, release_id, identity) = signed_store(&mut store, archive);
        let session_fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/registry-v1.json")).unwrap();
        let mut session = session_fixture
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["name"] == "session")
            .unwrap()["body"]["data"]
            .clone();
        session["expiresAt"] = (OffsetDateTime::now_utc() + time::Duration::hours(1))
            .format(&Rfc3339)
            .unwrap()
            .into();
        let session: Session = serde_json::from_value(session).unwrap();
        let account = session.account_id;
        let (origin, thread) = serve_download(&identity, archive, &session, Some(true));
        let client = Client::new(
            crate::registry::Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true)
                .unwrap(),
        )
        .unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        let (_auth_sender, auth_cancel) = watch::channel(false);
        let request_id = Uuid::new_v4().to_string();
        let started = crate::backend::registry_package_action(
            &mut store,
            &mut queue,
            &request_id,
            RegistryRequest {
                mod_id,
                release_id,
                root_public,
                client: client.clone(),
                session: session.clone(),
                bearer: "fixture-token".into(),
                auth_cancel: auth_cancel.clone(),
            },
        )
        .unwrap()
        .into_iter()
        .find(|operation| operation.request_id == request_id)
        .unwrap();
        let mut final_operation = started;
        for _ in 0..100 {
            queue.poll(&mut store).unwrap();
            final_operation = queue
                .operations(&store)
                .unwrap()
                .into_iter()
                .find(|operation| operation.id == final_operation.id)
                .unwrap();
            if final_operation.message.contains("receipt confirmed") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(final_operation.status, Status::Completed);
        assert!(
            final_operation
                .message
                .contains("Installation remains pending")
        );
        assert_eq!(
            fs::read(cached(root.path(), &identity).unwrap().unwrap()).unwrap(),
            archive
        );
        assert!(store.pending_registry_receipts(account).unwrap().is_empty());
        assert!(store.registry_references().unwrap().is_empty());
        assert_eq!(thread.join().unwrap().len(), 4);
        drop(queue);
        drop(store);
        let mut store = Storage::open(root.path()).unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        let cached_path = cached(root.path(), &identity).unwrap().unwrap();
        let mut cached_operation = queue
            .start_registry(
                &mut store,
                &Uuid::new_v4().to_string(),
                RegistryRequest {
                    mod_id,
                    release_id,
                    root_public,
                    client: client.clone(),
                    session: session.clone(),
                    bearer: "fixture-token".into(),
                    auth_cancel: auth_cancel.clone(),
                },
            )
            .unwrap();
        assert_eq!(cached_operation.status, Status::Preparing);
        for _ in 0..100 {
            queue.poll(&mut store).unwrap();
            cached_operation = queue
                .operations(&store)
                .unwrap()
                .into_iter()
                .find(|operation| operation.id == cached_operation.id)
                .unwrap();
            if cached_operation.status == Status::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(cached_operation.status, Status::Completed);
        assert!(cached_operation.message.contains("No new download receipt"));
        fs::write(cached_path, b"changed cached bytes").unwrap();
        let mut corrupt_operation = queue
            .start_registry(
                &mut store,
                &Uuid::new_v4().to_string(),
                RegistryRequest {
                    mod_id,
                    release_id,
                    root_public,
                    client,
                    session,
                    bearer: "fixture-token".into(),
                    auth_cancel,
                },
            )
            .unwrap();
        for _ in 0..100 {
            queue.poll(&mut store).unwrap();
            corrupt_operation = queue
                .operations(&store)
                .unwrap()
                .into_iter()
                .find(|operation| operation.id == corrupt_operation.id)
                .unwrap();
            if corrupt_operation.status == Status::Failed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(corrupt_operation.status, Status::Failed);
        assert!(corrupt_operation.message.contains("size changed"));
        assert!(store.pending_registry_receipts(account).unwrap().is_empty());
    }

    #[tokio::test]
    async fn queue_retries_account_receipt_after_restart_without_redownloading() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).unwrap();
        let archive = b"PK\x03\x04receipt retry archive";
        let (root_public, mod_id, release_id, identity) = signed_store(&mut store, archive);
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/registry-v1.json")).unwrap();
        let mut session = fixtures
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["name"] == "session")
            .unwrap()["body"]["data"]
            .clone();
        session["expiresAt"] = (OffsetDateTime::now_utc() + time::Duration::hours(1))
            .format(&Rfc3339)
            .unwrap()
            .into();
        let session: Session = serde_json::from_value(session).unwrap();
        let (origin, thread) = serve_download(&identity, archive, &session, Some(false));
        let client = Client::new(
            crate::registry::Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true)
                .unwrap(),
        )
        .unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        let (_sender, cancel) = watch::channel(false);
        let started = queue
            .start_registry(
                &mut store,
                &Uuid::new_v4().to_string(),
                RegistryRequest {
                    mod_id,
                    release_id,
                    root_public,
                    client,
                    session: session.clone(),
                    bearer: "fixture-token".into(),
                    auth_cancel: cancel.clone(),
                },
            )
            .unwrap();
        let mut operation = started;
        for _ in 0..100 {
            queue.poll(&mut store).unwrap();
            operation = queue
                .operations(&store)
                .unwrap()
                .into_iter()
                .find(|item| item.id == operation.id)
                .unwrap();
            if queue.registry_active.is_empty() && operation.status == Status::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(operation.message.contains("receipt is pending"));
        assert!(operation.receipt_id.is_some());
        assert_eq!(
            store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(thread.join().unwrap().len(), 4);
        drop(queue);
        drop(store);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let retry = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let incoming = request(&mut stream);
            let body = serde_json::json!({"apiVersion":1,"data":{
                "releaseId":release_id.0,"counted":false
            }})
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            incoming
        });
        let client = Client::new(
            crate::registry::Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true)
                .unwrap(),
        )
        .unwrap();
        let mut store = Storage::open(root.path()).unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        assert_eq!(
            queue
                .resume_registry_receipts(
                    &store,
                    client,
                    session.clone(),
                    "fixture-token".into(),
                    cancel,
                )
                .unwrap(),
            1
        );
        for _ in 0..100 {
            queue.poll(&mut store).unwrap();
            if store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .is_empty()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .is_empty()
        );
        let confirmed = store
            .package_request(&operation.request_id)
            .unwrap()
            .unwrap();
        assert_eq!(confirmed.status, Status::Completed);
        assert_eq!(confirmed.receipt_id, None);
        assert!(confirmed.message.contains("receipt confirmed"));
        assert!(retry.join().unwrap().starts_with(&format!(
            "POST /v1/registry/releases/{}/receipts HTTP/1.1",
            release_id.0
        )));
        assert_eq!(
            fs::read(cached(root.path(), &identity).unwrap().unwrap()).unwrap(),
            archive
        );
    }

    #[tokio::test]
    async fn queue_cancel_removes_partial_stage_without_receipt() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).unwrap();
        let archive = b"PK\x03\x04a cancellable registry archive";
        let (root_public, mod_id, release_id, identity) = signed_store(&mut store, archive);
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/registry-v1.json")).unwrap();
        let mut session = fixtures
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["name"] == "session")
            .unwrap()["body"]["data"]
            .clone();
        session["expiresAt"] = (OffsetDateTime::now_utc() + time::Duration::hours(1))
            .format(&Rfc3339)
            .unwrap()
            .into();
        let session: Session = serde_json::from_value(session).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let reference = identity.reference.clone();
        let hash = identity.reference.sha256.as_str().to_owned();
        let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(5))
            .format(&Rfc3339)
            .unwrap();
        let (release_client, release_server) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let grant_request = request(&mut stream);
            let body = serde_json::json!({"apiVersion":1,"data":{
                "downloadId":"33333333-3333-4333-8333-333333333333",
                "reference":reference,"bytes":archive.len(),
                "metadataRevision":1,"securityRevision":1,
                "contentPath":"/v1/downloads/33333333-3333-4333-8333-333333333333/content",
                "expiresAt":expires
            }})
            .to_string();
            write!(
                stream,
                "HTTP/1.1 201 Created\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            drop(stream);
            let (mut stream, _) = listener.accept().unwrap();
            let content_request = request(&mut stream);
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"sha256-{hash}\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n", archive.len()).unwrap();
            stream.write_all(&archive[..8]).unwrap();
            release_server.recv_timeout(Duration::from_secs(5)).unwrap();
            (grant_request, content_request)
        });
        let client = Client::new(
            crate::registry::Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true)
                .unwrap(),
        )
        .unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        let (_sender, auth_cancel) = watch::channel(false);
        let operation = queue
            .start_registry(
                &mut store,
                &Uuid::new_v4().to_string(),
                RegistryRequest {
                    mod_id,
                    release_id,
                    root_public,
                    client,
                    session: session.clone(),
                    bearer: "fixture-token".into(),
                    auth_cancel,
                },
            )
            .unwrap();
        for _ in 0..100 {
            if queue
                .operations(&store)
                .unwrap()
                .iter()
                .any(|item| item.id == operation.id && item.received_bytes >= 8)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(
            queue
                .operations(&store)
                .unwrap()
                .iter()
                .any(|item| item.id == operation.id && item.received_bytes >= 8)
        );
        queue.cancel(&mut store, &operation.id).unwrap();
        for _ in 0..100 {
            queue.poll(&mut store).unwrap();
            if queue.registry_active.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let finished = queue
            .operations(&store)
            .unwrap()
            .into_iter()
            .find(|item| item.id == operation.id)
            .unwrap();
        assert_eq!(finished.status, Status::Cancelled);
        assert!(
            store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            !stage_path(
                &root.path().join("registry-archives"),
                Uuid::parse_str(&operation.id).unwrap()
            )
            .exists()
        );
        release_client.send(()).unwrap();
        let (grant, content) = server.join().unwrap();
        assert!(grant.starts_with(&format!(
            "POST /v1/registry/releases/{}/downloads HTTP/1.1",
            release_id.0
        )));
        assert!(content.starts_with("GET /v1/downloads/"));
    }

    #[tokio::test]
    async fn newer_signed_block_prevents_queue_promotion_after_transfer() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).unwrap();
        let archive = b"PK\x03\x04blocked after transfer";
        let (root_public, mod_id, release_id, identity) = signed_store(&mut store, archive);
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/registry-v1.json")).unwrap();
        let mut session = fixtures
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["name"] == "session")
            .unwrap()["body"]["data"]
            .clone();
        session["expiresAt"] = (OffsetDateTime::now_utc() + time::Duration::hours(1))
            .format(&Rfc3339)
            .unwrap()
            .into();
        let session: Session = serde_json::from_value(session).unwrap();
        let (origin, server) = serve_download(&identity, archive, &session, None);
        let client = Client::new(
            crate::registry::Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true)
                .unwrap(),
        )
        .unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        let (_sender, cancel) = watch::channel(false);
        let operation = queue
            .start_registry(
                &mut store,
                &Uuid::new_v4().to_string(),
                RegistryRequest {
                    mod_id,
                    release_id,
                    root_public,
                    client,
                    session: session.clone(),
                    bearer: "fixture-token".into(),
                    auth_cancel: cancel,
                },
            )
            .unwrap();
        assert_eq!(server.join().unwrap().len(), 3);
        for _ in 0..100 {
            if queue
                .registry_active
                .get(&operation.id)
                .unwrap()
                .worker
                .inner()
                .is_finished()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let mut security = fixture["securityEnvelope"]["signed"].clone();
        security["revision"] = 2.into();
        security["decisions"] = serde_json::json!([{
            "sha256":identity.reference.sha256.as_str(),
            "revision":2,"status":"blocked","reason":"Fixture block"
        }]);
        let signed = sign(
            security,
            &online,
            &fixture["envelope"]["signed"]["keys"][0]["keyId"],
            OffsetDateTime::now_utc(),
            12,
        );
        store
            .accept_registry_security(&signed, &root_public, OffsetDateTime::now_utc())
            .unwrap();
        queue.poll(&mut store).unwrap();
        let finished = queue
            .operations(&store)
            .unwrap()
            .into_iter()
            .find(|item| item.id == operation.id)
            .unwrap();
        assert_eq!(finished.status, Status::Failed);
        assert!(cached(root.path(), &identity).unwrap().is_none());
        assert!(
            store
                .registry_hash_blocked(&identity.reference.sha256)
                .unwrap()
        );
        assert_eq!(
            store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .len(),
            1
        );
        assert!(
            !stage_path(
                &root.path().join("registry-archives"),
                Uuid::parse_str(&operation.id).unwrap()
            )
            .exists()
        );
        assert!(store.registry_references().unwrap().is_empty());
    }

    #[tokio::test]
    async fn account_change_after_transfer_keeps_claim_without_cache_promotion() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).unwrap();
        let archive = b"PK\x03\x04account-bound transfer";
        let (root_public, mod_id, release_id, identity) = signed_store(&mut store, archive);
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/registry-v1.json")).unwrap();
        let mut session = fixtures
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["name"] == "session")
            .unwrap()["body"]["data"]
            .clone();
        session["expiresAt"] = (OffsetDateTime::now_utc() + time::Duration::hours(1))
            .format(&Rfc3339)
            .unwrap()
            .into();
        let session: Session = serde_json::from_value(session).unwrap();
        let other = Session {
            account_id: Uuid::new_v4(),
            ..session.clone()
        };
        let (origin, server) = serve_download(&identity, archive, &other, None);
        let client = Client::new(
            crate::registry::Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true)
                .unwrap(),
        )
        .unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        let (_sender, cancel) = watch::channel(false);
        let operation = queue
            .start_registry(
                &mut store,
                &Uuid::new_v4().to_string(),
                RegistryRequest {
                    mod_id,
                    release_id,
                    root_public,
                    client,
                    session: session.clone(),
                    bearer: "fixture-token".into(),
                    auth_cancel: cancel,
                },
            )
            .unwrap();
        assert_eq!(server.join().unwrap().len(), 3);
        for _ in 0..100 {
            queue.poll(&mut store).unwrap();
            if queue.registry_active.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let finished = queue
            .operations(&store)
            .unwrap()
            .into_iter()
            .find(|item| item.id == operation.id)
            .unwrap();
        assert_eq!(finished.status, Status::Failed);
        assert!(finished.message.contains("Sign in again"));
        assert!(cached(root.path(), &identity).unwrap().is_none());
        assert_eq!(
            store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .len(),
            1
        );
        assert!(
            store
                .pending_registry_receipts(other.account_id)
                .unwrap()
                .is_empty()
        );
    }
}
