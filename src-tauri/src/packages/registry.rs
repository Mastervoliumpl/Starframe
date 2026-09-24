use super::{
    Cancel, Operation, Packages, RegistryActive, RegistryEvent, RegistryRequest, Result, Status,
    read_file,
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

fn verify(path: &Path, identity: &DownloadIdentity) -> Result<()> {
    let mut file = read_file(path)?;
    if file.metadata().map_err(|e| e.to_string())?.len() != identity.bytes {
        return Err("Registry archive size changed. Retry the exact download.".into());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
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

pub(super) fn cached(root: &Path, identity: &DownloadIdentity) -> Result<Option<PathBuf>> {
    let (dir, _pin) = archive_dir(root)?;
    let path = archive_path(&dir, identity);
    if !path.exists() {
        return Ok(None);
    }
    verify(&path, identity)?;
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
) -> Result<PathBuf> {
    if !verified.matches_identity(identity) {
        return Err("Registry transfer identity changed.".into());
    }
    let (dir, _pin) = archive_dir(root)?;
    let stage = stage_path(&dir, operation_id);
    verify(&stage, identity)?;
    let target = archive_path(&dir, identity);
    if target.exists() {
        verify(&target, identity)?;
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
            if self.registry_receipts.contains_key(&claim.download_id) {
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
        if !self.can_start(identity.reference.sha256.as_str()) {
            return Err(
                "Three downloads are already active, or this archive is being prepared.".into(),
            );
        }
        let root = storage.package_root().to_owned();
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
            status: Status::Preparing,
            message: "Downloading the exact signed registry archive.".into(),
            received_bytes: 0,
            total_bytes: identity.bytes,
        };
        if cached(&root, &identity)?.is_some() {
            operation.status = Status::Completed;
            operation.received_bytes = identity.bytes;
            operation.message =
                "Exact verified archive is cached. No new download receipt was sent.".into();
            storage
                .save_package(&operation)
                .map_err(|e| e.to_string())?;
            return Ok(operation);
        }
        let reserved = self
            .active
            .values()
            .map(|active| active.operation.total_bytes + super::MAX_EXPANDED_BYTES)
            .chain(
                self.registry_active
                    .values()
                    .map(|active| active.operation.total_bytes),
            )
            .sum::<u64>();
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
                Err(_) if active.claim.is_some() => RegistryEvent::Receipt(Err(
                    "The receipt worker stopped. The verified archive and claim were retained."
                        .into(),
                )),
                Err(_) => RegistryEvent::Transferred(Box::new(Err(
                    "Registry worker stopped before the archive was saved. Retry.".into(),
                ))),
            };
            changed = true;
            match event {
                RegistryEvent::Transferred(outcome) if outcome.is_ok() => {
                    let (verified, current) = outcome.unwrap();
                    let claim = storage
                        .record_registry_receipt(&verified)
                        .map_err(|e| e.to_string())?;
                    active.claim = Some(claim.clone());
                    let outcome = (|| {
                        active.cancel.check()?;
                        if *active.abort.borrow() {
                            return Err("Sign-in changed during the download. The receipt claim remains with its original account; the archive was not saved.".into());
                        }
                        let current = current?;
                        storage
                            .confirm_registry_download(
                                &active.root,
                                &active.identity,
                                OffsetDateTime::now_utc(),
                            )
                            .map_err(|e| e.to_string())?;
                        promote(
                            storage.package_root(),
                            &active.identity,
                            &verified,
                            Uuid::parse_str(&active.operation.id).unwrap(),
                        )?;
                        Ok(current)
                    })();
                    match outcome {
                        Ok(current) => {
                            active.operation.status = Status::Completed;
                            active.operation.message =
                                "Archive verified and saved. Download receipt pending; retry when signed in. Installation remains pending."
                                    .into();
                            active.operation.received_bytes = active.operation.total_bytes;
                            storage
                                .save_package(&active.operation)
                                .map_err(|e| e.to_string())?;
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
                RegistryEvent::Receipt(result) => {
                    if result.is_ok()
                        && let Some(claim) = &active.claim
                    {
                        storage
                            .clear_registry_receipt(claim)
                            .map_err(|e| e.to_string())?;
                    }
                    active.operation.status = Status::Completed;
                    active.operation.received_bytes = active.operation.total_bytes;
                    active.operation.message = match result {
                        Ok(()) => "Verified registry archive saved. Download receipt confirmed. Installation remains pending.".into(),
                        Err(error) => format!("Verified registry archive saved. Download receipt is pending: {error}"),
                    };
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
                    storage
                        .clear_registry_receipt(&retry.claim)
                        .map_err(|e| e.to_string())?;
                    changed = true;
                }
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => (),
                Err(mpsc::TryRecvError::Empty) if retry.worker.inner().is_finished() => (),
                Err(mpsc::TryRecvError::Empty) => {
                    self.registry_receipts.insert(id, retry);
                }
            }
        }
        Ok(changed)
    }
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
        receipt_ok: bool,
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
            for step in 0..4 {
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
                    _ if receipt_ok => (
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
        let (origin, thread) = serve_download(&identity, archive, &session, true);
        let client = Client::new(
            crate::registry::Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true)
                .unwrap(),
        )
        .unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        let (_auth_sender, auth_cancel) = watch::channel(false);
        let started = queue
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
        let cached_operation = queue
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
        assert_eq!(cached_operation.status, Status::Completed);
        assert!(cached_operation.message.contains("No new download receipt"));
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
        let (origin, thread) = serve_download(&identity, archive, &session, false);
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
}
