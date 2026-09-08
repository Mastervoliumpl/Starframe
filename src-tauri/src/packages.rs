use crate::{
    catalog::{Artifact, Catalog, Layout},
    deployment::{pin, regular_metadata},
    storage::{LibraryEntry, ModReference, Origin, Storage},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::watch};
use uuid::Uuid;

#[cfg(test)]
mod tests;
mod transfer;

type Result<T> = std::result::Result<T, String>;
const MAX_ENTRIES: usize = 4096;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const CANCELLED: &str = "Package preparation cancelled. No game files changed.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Preparing,
    Cancelling,
    Cancelled,
    Completed,
    Failed,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    #[serde(rename_all = "camelCase")]
    Prepare {
        request_id: String,
        release_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Cancel {
        operation_id: String,
    },
    List,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operation {
    pub id: String,
    pub request_id: String,
    pub release_id: String,
    pub hash: String,
    pub status: Status,
    pub message: String,
    pub received_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedFile {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Prepared {
    pub hash: String,
    pub files: Vec<PreparedFile>,
}

#[derive(Clone)]
struct Cancel {
    flag: Arc<AtomicBool>,
    signal: watch::Sender<bool>,
}
impl Default for Cancel {
    fn default() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            signal: watch::channel(false).0,
        }
    }
}
impl Cancel {
    fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.signal.send_replace(true);
    }
    fn check(&self) -> Result<()> {
        if self.flag.load(Ordering::SeqCst) {
            Err(CANCELLED.into())
        } else {
            Ok(())
        }
    }
    async fn cancelled(&self) {
        let mut receiver = self.signal.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}

struct Active {
    operation: Operation,
    entry: LibraryEntry,
    cancel: Cancel,
    progress: Arc<AtomicU64>,
    result: mpsc::Receiver<Result<Prepared>>,
    ready: Option<Result<Prepared>>,
}

/// The existing storage worker owns this queue and calls `poll` to commit results.
/// Dropping the queue cancels workers; startup records unfinished work as failed.
pub struct Packages {
    client: reqwest::Client,
    active: HashMap<String, Active>,
}

impl Packages {
    pub(crate) fn can_start(&self, hash: &str) -> bool {
        self.active.len() < 3 && !self.busy_hash(hash)
    }
    pub fn busy_hash(&self, hash: &str) -> bool {
        self.active
            .values()
            .any(|active| active.operation.hash == hash)
    }
    pub fn open(storage: &mut Storage) -> Result<Self> {
        storage.recover_packages().map_err(|e| e.to_string())?;
        crate::sharing::recover(storage)?;
        Ok(Self {
            client: transfer::client()?,
            active: HashMap::new(),
        })
    }

    pub fn start(
        &mut self,
        storage: &mut Storage,
        request_id: &str,
        release_id: &str,
    ) -> Result<Operation> {
        Uuid::parse_str(request_id).map_err(|_| "Invalid package request ID.")?;
        if let Some(operation) = storage
            .package_request(request_id)
            .map_err(|e| e.to_string())?
        {
            if operation.release_id != release_id {
                return Err("This request ID belongs to a different release.".into());
            }
            return Ok(operation);
        }
        let catalog = storage
            .catalog_cache()
            .map_err(|e| e.to_string())?
            .and_then(|cache| cache.catalog)
            .ok_or("No approved catalog is available. Wait for catalog refresh.")?;
        let (entry, artifact) = resolve(&catalog, release_id)?;
        if storage
            .pending_removals()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|(hash, _)| hash == &artifact.sha256)
        {
            return Err("This artifact still has pending uninstall cleanup. Restart to retry cleanup before downloading it again.".into());
        }
        if let Some(active) = self
            .active
            .values()
            .find(|a| a.operation.hash == artifact.sha256)
        {
            return Err(format!(
                "This artifact is already being prepared by operation {}.",
                active.operation.id
            ));
        }
        if self.active.len() >= 3 {
            return Err(
                "Three packages are being prepared. Wait for one to finish or cancel it.".into(),
            );
        }
        let operation = Operation {
            id: Uuid::new_v4().to_string(),
            request_id: request_id.into(),
            release_id: release_id.into(),
            hash: artifact.sha256.clone(),
            status: Status::Preparing,
            message: "Downloading and verifying the approved package.".into(),
            received_bytes: 0,
            total_bytes: artifact.size_bytes,
        };
        let root = storage.package_root().to_owned();
        let previous = storage
            .prepared_artifact(&artifact.sha256)
            .map_err(|e| e.to_string())?;
        if previous.is_none() {
            let reserved: u64 = self
                .active
                .values()
                .map(|a| a.operation.total_bytes + MAX_EXPANDED_BYTES)
                .sum();
            crate::space::require(&root, reserved + artifact.size_bytes + MAX_EXPANDED_BYTES)?;
        }
        storage
            .save_package(&operation)
            .map_err(|e| e.to_string())?;
        let cancel = Cancel::default();
        let progress = Arc::new(AtomicU64::new(0));
        let (sender, result) = mpsc::sync_channel(1);
        let worker_cancel = cancel.clone();
        let worker_progress = progress.clone();
        let id = operation.id.clone();
        let client = self.client.clone();
        tauri::async_runtime::spawn(async move {
            let outcome = prepare(
                root,
                id,
                artifact,
                previous,
                client,
                worker_cancel,
                worker_progress,
            )
            .await;
            let _ = sender.send(outcome);
        });
        self.active.insert(
            operation.id.clone(),
            Active {
                operation: operation.clone(),
                entry,
                cancel,
                progress,
                result,
                ready: None,
            },
        );
        Ok(operation)
    }

    pub fn cancel(&mut self, storage: &mut Storage, operation_id: &str) -> Result<()> {
        let active = self
            .active
            .get_mut(operation_id)
            .ok_or("This package operation is not running.")?;
        active.cancel.cancel();
        active.operation.status = Status::Cancelling;
        active.operation.message = "Stopping package preparation at the next file boundary.".into();
        storage
            .save_package(&active.operation)
            .map_err(|e| e.to_string())
    }

    pub(crate) fn verify_local(
        &mut self,
        storage: &mut Storage,
        reference: &ModReference,
    ) -> Result<Operation> {
        if reference.origin != Origin::LocalImport || !self.can_start(&reference.hash) {
            return Err("Local verification is unavailable. Retry shortly.".into());
        }
        let entry = storage
            .load()
            .map_err(|e| e.to_string())?
            .library
            .into_iter()
            .find(|e| &e.reference == reference)
            .ok_or("The exact local content is missing.")?;
        let prepared = storage.prepared_artifact(&reference.hash).map_err(|e| e.to_string())?.ok_or("This local content has no verified file manifest. Import the exact build again when local imports are supported.")?;
        let total_bytes = prepared
            .files
            .iter()
            .map(|f| f.size_bytes)
            .sum::<u64>()
            .max(1);
        let operation = Operation {
            id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            release_id: "local-verification".into(),
            hash: reference.hash.clone(),
            status: Status::Preparing,
            message: "Verifying matching local content. No download is needed.".into(),
            received_bytes: 0,
            total_bytes,
        };
        storage
            .save_package(&operation)
            .map_err(|e| e.to_string())?;
        let root = storage.package_root().to_owned();
        let path = storage
            .artifact_directory(reference)
            .map_err(|e| e.to_string())?;
        let cancel = Cancel::default();
        let worker_cancel = cancel.clone();
        let (sender, result) = mpsc::sync_channel(1);
        tauri::async_runtime::spawn_blocking(move || {
            let outcome = (|| {
                verify_existing(
                    &mut Directory::open(&root)?,
                    &path,
                    &prepared,
                    &worker_cancel,
                )?;
                Ok(prepared)
            })();
            let _ = sender.send(outcome);
        });
        self.active.insert(
            operation.id.clone(),
            Active {
                operation: operation.clone(),
                entry,
                cancel,
                progress: Arc::new(AtomicU64::new(0)),
                result,
                ready: None,
            },
        );
        Ok(operation)
    }

    pub fn poll(&mut self, storage: &mut Storage) -> Result<bool> {
        let mut changed = false;
        for id in self.active.keys().cloned().collect::<Vec<_>>() {
            let active = self.active.get_mut(&id).unwrap();
            active.operation.received_bytes = active.progress.load(Ordering::Relaxed);
            if active.ready.is_none() {
                active.ready = Some(match active.result.try_recv() {
                    Ok(outcome) => outcome,
                    Err(mpsc::TryRecvError::Empty) => continue,
                    Err(_) => Err("Package worker stopped unexpectedly. Retry the release.".into()),
                });
            }
            // Completion is committed here, so cancellation can still win after extraction.
            let outcome = active.ready.as_ref().unwrap().clone().and_then(|prepared| {
                active.cancel.check()?;
                if active.entry.reference.origin == Origin::LocalImport {
                    if !storage
                        .load()
                        .map_err(|e| e.to_string())?
                        .library
                        .iter()
                        .any(|e| e.reference == active.entry.reference)
                    {
                        return Err("The local reference changed during verification.".into());
                    }
                    return Ok(prepared);
                }
                let catalog = storage
                    .catalog_cache()
                    .map_err(|e| e.to_string())?
                    .and_then(|cache| cache.catalog)
                    .ok_or("Approved catalog is unavailable. Retry after refresh.")?;
                if catalog.downloadable(&active.operation.release_id)?.sha256 != prepared.hash {
                    return Err("Release identity changed during package preparation.".into());
                }
                Ok(prepared)
            });
            match outcome {
                Ok(prepared) => {
                    active.operation.status = Status::Completed;
                    active.operation.received_bytes = active.operation.total_bytes;
                    active.operation.message = "Verified package saved in the library.".into();
                    if let Err(error) = if active.entry.reference.origin == Origin::LocalImport {
                        storage.save_package(&active.operation)
                    } else {
                        storage.complete_package(&active.operation, &active.entry, &prepared)
                    } {
                        active.operation.status = Status::Failed;
                        active.operation.message = format!(
                            "Could not save the prepared package: {error}. Retry the release; verified content was retained."
                        );
                        storage
                            .save_package(&active.operation)
                            .map_err(|e| e.to_string())?;
                    }
                }
                Err(message) => {
                    active.operation.status = if message == CANCELLED {
                        Status::Cancelled
                    } else {
                        Status::Failed
                    };
                    active.operation.message = message.chars().take(1000).collect();
                    storage
                        .save_package(&active.operation)
                        .map_err(|e| e.to_string())?;
                }
            }
            self.active.remove(&id);
            changed = true;
        }
        Ok(changed)
    }

    pub fn operations(&self, storage: &Storage) -> Result<Vec<Operation>> {
        let mut operations = storage.package_operations().map_err(|e| e.to_string())?;
        for operation in &mut operations {
            if let Some(active) = self.active.get(&operation.id) {
                operation.received_bytes = active.progress.load(Ordering::Relaxed);
            }
        }
        for active in self.active.values() {
            if !operations.iter().any(|op| op.id == active.operation.id) {
                let mut operation = active.operation.clone();
                operation.received_bytes = active.progress.load(Ordering::Relaxed);
                operations.push(operation);
            }
        }
        Ok(operations)
    }
}
impl Drop for Packages {
    fn drop(&mut self) {
        for active in self.active.values() {
            active.cancel.cancel();
        }
    }
}

fn resolve(catalog: &Catalog, release_id: &str) -> Result<(LibraryEntry, Artifact)> {
    catalog.validate()?;
    let artifact = catalog.downloadable(release_id)?.clone();
    let (owner, release) = catalog
        .releases()
        .find(|(_, r)| r.id == release_id)
        .ok_or("Release is not approved.")?;
    Ok((
        LibraryEntry {
            reference: ModReference {
                mod_id: owner.id.clone(),
                hash: artifact.sha256.clone(),
                origin: Origin::Catalog,
                release_id: Some(release.id.clone()),
            },
            name: owner.name.clone(),
            author: owner.author.clone(),
            version: release.version.clone(),
        },
        artifact,
    ))
}

pub(crate) struct Directory {
    root: PathBuf,
    pins: BTreeMap<PathBuf, File>,
}

pub(crate) fn verify_artifact(store: &Storage, reference: &ModReference) -> Result<Prepared> {
    let prepared = store
        .prepared_artifact(&reference.hash)
        .map_err(|e| e.to_string())?
        .ok_or("The package has no verified file manifest. Prepare it again.")?;
    let mut directory = Directory::open(store.package_root())?;
    verify_existing(
        &mut directory,
        &store
            .artifact_directory(reference)
            .map_err(|e| e.to_string())?,
        &prepared,
        &Cancel::default(),
    )?;
    Ok(prepared)
}

pub(crate) fn remove_artifact(store: &Storage, hash: &str) -> Result<()> {
    if store
        .load()
        .map_err(|e| e.to_string())?
        .library
        .iter()
        .any(|entry| entry.reference.hash == hash)
    {
        return Err("This artifact is still in the library. Cleanup retained it.".into());
    }
    let reference = ModReference {
        mod_id: "artifact".into(),
        hash: hash.into(),
        origin: Origin::LocalImport,
        release_id: None,
    };
    let root = store
        .artifact_directory(&reference)
        .map_err(|e| e.to_string())?;
    match fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
        Ok(_) => (),
    }
    let prepared = store
        .prepared_artifact(hash)
        .map_err(|e| e.to_string())?
        .ok_or("Uninstall inventory is missing. Files were retained.")?;
    let mut directory = Directory::open(store.package_root())?;
    directory.directory(&format!("artifacts/{hash}"))?;
    let mut pending = vec![root.clone()];
    let mut folders = Vec::new();
    let mut files = Vec::new();
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
            if files.len() + folders.len() + pending.len() > MAX_ENTRIES * 32 {
                return Err("Uninstall cleanup exceeded its file limit.".into());
            }
            let path = entry.map_err(|e| e.to_string())?.path();
            let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            regular_metadata(&path, metadata.is_dir())?;
            let relative = path
                .strip_prefix(&root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            crate::runtime_contract::relative_path(&relative)?;
            if metadata.is_dir() {
                directory.directory(&format!("artifacts/{hash}/{relative}"))?;
                pending.push(path);
            } else {
                let expected = prepared
                    .files
                    .iter()
                    .find(|f| f.path == relative)
                    .ok_or("Uninstall found an unexpected file. It was retained.")?;
                if digest(
                    &mut read_file(&path)?,
                    expected.size_bytes,
                    &Cancel::default(),
                )? != (expected.size_bytes, expected.sha256.clone())
                {
                    return Err(
                        "Uninstall found changed content. It was retained for repair.".into(),
                    );
                }
                files.push(path);
            }
        }
        folders.push(folder);
    }
    for path in files {
        fs::remove_file(path).map_err(|e| {
            format!("Uninstall cleanup failed: {e}. Retry after the file is unlocked.")
        })?;
    }
    for folder in folders.into_iter().rev() {
        directory.pins.remove(&folder);
        fs::remove_dir(folder).map_err(|e| format!("Uninstall cleanup failed: {e}"))?;
    }
    Ok(())
}
impl Directory {
    pub(crate) fn open(root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err("Package storage requires an absolute app-data path.".into());
        }
        let mut pins = BTreeMap::new();
        for part in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
            pins.insert(part.to_owned(), pin(part)?);
        }
        Ok(Self {
            root: root.into(),
            pins,
        })
    }
    pub(crate) fn directory(&mut self, relative: &str) -> Result<PathBuf> {
        crate::runtime_contract::relative_path(relative)?;
        let mut path = self.root.clone();
        for part in relative.split('/') {
            path.push(part);
            if !self.pins.contains_key(&path) {
                match fs::create_dir(&path) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(format!("Cannot create package staging: {e}")),
                }
                self.pins.insert(path.clone(), pin(&path)?);
            }
        }
        Ok(path)
    }

    fn remove_stage(&mut self, id: &str) -> Result<()> {
        Uuid::parse_str(id).map_err(|_| "Invalid staging operation ID.")?;
        let stage = self.root.join("package-staging").join(id);
        if !stage.starts_with(&self.root) {
            return Err("Staging path escaped app data.".into());
        }
        let mut pending = vec![stage.clone()];
        let mut directories = Vec::new();
        let mut files = Vec::new();
        while let Some(folder) = pending.pop() {
            self.pins.insert(folder.clone(), pin(&folder)?);
            for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if directories.len() + pending.len() + files.len() > MAX_ENTRIES * 32 {
                    return Err("Staging cleanup exceeded its file limit.".into());
                }
                let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                regular_metadata(&path, metadata.is_dir())?;
                if metadata.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path);
                }
            }
            directories.push(folder);
        }
        for file in files {
            fs::remove_file(file).map_err(|e| e.to_string())?;
        }
        for folder in directories.into_iter().rev() {
            self.pins.remove(&folder);
            fs::remove_dir(folder).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

async fn prepare(
    root: PathBuf,
    id: String,
    artifact: Artifact,
    previous: Option<Prepared>,
    client: reqwest::Client,
    cancel: Cancel,
    progress: Arc<AtomicU64>,
) -> Result<Prepared> {
    let worker_artifact = artifact.clone();
    let worker_cancel = cancel.clone();
    let stage_id = id.clone();
    let setup = tauri::async_runtime::spawn_blocking(move || {
        let mut directory = Directory::open(&root)?;
        directory.directory("artifacts")?;
        let final_path = root.join("artifacts").join(&worker_artifact.sha256);
        if let Some(previous) = previous {
            verify_existing(&mut directory, &final_path, &previous, &worker_cancel)?;
            layout(&previous.files, &worker_artifact.layout)?;
            return Ok((directory, None, Some(previous)));
        }
        let stage = directory.directory(&format!("package-staging/{stage_id}"))?;
        Ok::<_, String>((directory, Some(stage), None))
    })
    .await
    .map_err(|e| format!("Package staging worker failed: {e}"))??;
    let (mut directory, stage, previous) = setup;
    if let Some(previous) = previous {
        return Ok(previous);
    }
    let stage = stage.ok_or("Package staging was not created.")?;
    let archive = stage.join("download.zip");
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&archive)
        .await
        .map_err(|e| format!("Cannot create package download: {e}"))?;
    let downloaded = tokio::select! {
        _ = cancel.cancelled() => Err(CANCELLED.into()),
        _ = tokio::time::sleep(Duration::from_secs(300)) => Err("Package transfer exceeded five minutes. Retry on a faster connection.".into()),
        result = transfer::download(&client, &artifact, &mut file, &progress) => result,
    };
    let downloaded = match downloaded {
        Ok(()) => file
            .sync_all()
            .await
            .map_err(|e| format!("Cannot flush the package download: {e}")),
        Err(error) => Err(error),
    };
    drop(file);
    if let Err(error) = downloaded {
        return tauri::async_runtime::spawn_blocking(move || match directory.remove_stage(&id) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(format!(
                "{error} Staging {id} was retained because cleanup failed: {cleanup}"
            )),
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let outcome = (|| {
        cancel.check()?;
        let content = directory.directory(&format!("package-staging/{id}/content"))?;
        let prepared = extract(&archive, &content, &artifact, &cancel)?;
        cancel.check()?;
        let final_path = directory.root.join("artifacts").join(&artifact.sha256);
        match fs::symlink_metadata(&final_path) {
            Ok(_) => {
                // A crash may leave promoted bytes without a database commit. Never overwrite them.
                verify_existing(&mut directory, &final_path, &prepared, &cancel)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                directory.pins.remove(&content);
                fs::rename(&content, &final_path).map_err(|e| format!("Cannot promote the verified package: {e}. Retry after checking app-data permissions."))?;
            }
            Err(e) => return Err(e.to_string()),
        }
        Ok(prepared)
        })();
        match directory.remove_stage(&id) {
            Ok(()) => outcome,
            Err(cleanup) => Err(format!("{} Staging {id} was retained because cleanup failed: {cleanup}", outcome.err().unwrap_or_else(|| "Package files verified; library commit was withheld.".into()))),
        }
    }).await.map_err(|e| format!("Package extraction worker failed: {e}"))?
}

fn layout(files: &[PreparedFile], layout: &Layout) -> Result<()> {
    if matches!(layout, Layout::StarframeLuaZip {}) {
        if files.is_empty() || files.iter().any(|f| !lua_path(&f.path)) {
            return Err("Lua overlays require only .lua files under LJ/lua; AI and map content are unsupported.".into());
        }
        return Ok(());
    }
    let Layout::StarframeManagedZip {
        root,
        entry_assembly,
        ..
    } = layout
    else {
        unreachable!()
    };
    let entry = if root.is_empty() {
        entry_assembly.clone()
    } else {
        format!("{root}/{entry_assembly}")
    };
    if !files
        .iter()
        .any(|file| file.path == entry && file.size_bytes > 0)
    {
        return Err(format!(
            "Unsupported package layout: approved entry {entry} is missing or empty. Ask the curator to correct the release."
        ));
    }
    Ok(())
}

pub(crate) fn lua_path(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.starts_with("lj/lua/")
        && path.ends_with(".lua")
        && !path.split('/').any(|part| part == "ai")
        && crate::runtime_contract::relative_path(&path).is_ok()
}

fn extract(
    archive: &Path,
    content: &Path,
    artifact: &Artifact,
    cancel: &Cancel,
) -> Result<Prepared> {
    let mut input = read_file(archive)?;
    let (size, hash) = digest(&mut input, artifact.size_bytes, cancel)?;
    if size != artifact.size_bytes || hash != artifact.sha256 {
        return Err("Package bytes do not match the approved SHA-256 and size. Retry or contact the curator.".into());
    }
    input.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let count = preflight_zip(&mut input)?;
    let mut archive =
        zip::ZipArchive::new(input).map_err(|e| format!("Unsupported or damaged ZIP: {e}"))?;
    if archive.len() != count || archive.has_overlapping_files().map_err(|e| e.to_string())? {
        return Err("ZIP has duplicate names or overlapping entry data.".into());
    }
    let mut names: BTreeMap<String, (String, bool)> = BTreeMap::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        cancel.check()?;
        let file = archive
            .by_index(index)
            .map_err(|e| format!("Cannot read ZIP entry: {e}"))?;
        let name = file.name().strip_suffix('/').unwrap_or(file.name());
        let normalized = crate::runtime_contract::relative_path(name)?;
        let is_directory = file.is_dir();
        if file.encrypted()
            || !matches!(
                file.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
        {
            return Err("Only unencrypted Stored or Deflate ZIP entries are supported.".into());
        }
        if let Some(mode) = file.unix_mode() {
            let kind = mode & 0o170000;
            if !matches!(kind, 0 | 0o100000 | 0o040000)
                || (kind == 0o040000 && !is_directory)
                || (kind == 0o100000 && is_directory)
            {
                return Err("ZIP links and special files are not supported.".into());
            }
        }
        total = total
            .checked_add(file.size())
            .ok_or("ZIP expanded size overflow.")?;
        if file.size() > MAX_FILE_BYTES
            || total > MAX_EXPANDED_BYTES
            || (is_directory && file.size() != 0)
        {
            return Err("ZIP exceeds the 512 MiB per-file or 2 GiB expanded-size limit.".into());
        }
        if names
            .insert(normalized, (name.into(), is_directory))
            .is_some()
        {
            return Err("ZIP contains duplicate or case-colliding paths.".into());
        }
    }
    let mut directories = names.clone();
    for (original, _) in names.values() {
        let mut prefix = String::new();
        let parts: Vec<_> = original.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let lower = prefix.to_ascii_lowercase();
            if let Some((existing, directory)) = directories.get(&lower)
                && (!directory || existing != &prefix)
            {
                return Err("ZIP has a file/directory or directory-case collision.".into());
            }
            directories.insert(lower, (prefix.clone(), true));
        }
    }
    crate::space::require(content, total)?;
    let mut directory = Directory::open(content)?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        cancel.check()?;
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        if entry.is_dir() {
            directory.directory(name.trim_end_matches('/'))?;
            continue;
        }
        if let Some((parent, _)) = name.rsplit_once('/') {
            directory.directory(parent)?;
        }
        let target = content.join(&name);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| format!("Cannot create extracted file: {e}"))?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            cancel.check()?;
            let count = entry
                .read(&mut buffer)
                .map_err(|e| format!("ZIP integrity check failed: {e}"))?;
            if count == 0 {
                break;
            }
            size += count as u64;
            if size > entry.size() || size > MAX_FILE_BYTES {
                return Err("ZIP entry expands beyond its declared size.".into());
            }
            output
                .write_all(&buffer[..count])
                .map_err(|e| format!("Cannot write extracted package: {e}"))?;
            hash.update(&buffer[..count]);
        }
        if size != entry.size() {
            return Err("ZIP entry is incomplete.".into());
        }
        output.sync_all().map_err(|e| e.to_string())?;
        files.push(PreparedFile {
            path: name,
            sha256: format!("{:x}", hash.finalize()),
            size_bytes: size,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    layout(&files, &artifact.layout)?;
    Ok(Prepared {
        hash: artifact.sha256.clone(),
        files,
    })
}

fn digest(input: &mut File, limit: u64, cancel: &Cancel) -> Result<(u64, String)> {
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        cancel.check()?;
        let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > limit {
            return Err("Package file exceeds its expected size.".into());
        }
        hash.update(&buffer[..count]);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

pub(crate) fn read_file(path: &Path) -> Result<File> {
    regular_metadata(path, false)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Deny writes/deletes while verifying; open a reparse point itself, never its target.
        options.share_mode(1).custom_flags(0x00200000);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Package path is not an ordinary file.".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Package files cannot be reparse points.".into());
        }
    }
    Ok(file)
}

// Bound central-directory allocation before the ZIP library parses it. ZIP64 and
// split archives are unnecessary within this package format's 2 GiB limit.
fn preflight_zip(input: &mut File) -> Result<usize> {
    let length = input.metadata().map_err(|e| e.to_string())?.len();
    let tail_length = length.min(65_557) as usize;
    input
        .seek(SeekFrom::End(-(tail_length as i64)))
        .map_err(|e| e.to_string())?;
    let mut tail = vec![0; tail_length];
    input.read_exact(&mut tail).map_err(|e| e.to_string())?;
    let end = tail
        .windows(4)
        .rposition(|p| p == b"PK\x05\x06")
        .ok_or("ZIP end record is missing.")?;
    if end + 22 > tail.len() {
        return Err("ZIP end record is truncated.".into());
    }
    let u16_at = |bytes: &[u8], at| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let u32_at =
        |bytes: &[u8], at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as u64;
    let footer = &tail[end..];
    let count = u16_at(footer, 10);
    let size = u32_at(footer, 12);
    let start = u32_at(footer, 16);
    if count == 0
        || count > MAX_ENTRIES
        || u16_at(footer, 8) != count
        || u16_at(footer, 4) != 0
        || u16_at(footer, 6) != 0
        || size > 4 * 1024 * 1024
        || start + size != length - tail_length as u64 + end as u64
        || end + 22 + u16_at(footer, 20) != tail.len()
    {
        return Err("ZIP exceeds entry/metadata limits or uses an unsupported ZIP64, split or trailing-data layout.".into());
    }
    input
        .seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut headers = vec![0; size as usize];
    input.read_exact(&mut headers).map_err(|e| e.to_string())?;
    let mut cursor = 0;
    for _ in 0..count {
        if cursor + 46 > headers.len() || &headers[cursor..cursor + 4] != b"PK\x01\x02" {
            return Err("Invalid ZIP central directory.".into());
        }
        let header = &headers[cursor..];
        let name_size = u16_at(header, 28);
        let extra_size = u16_at(header, 30);
        let end = cursor + 46 + name_size + extra_size + u16_at(header, 32);
        if end > headers.len() || u16_at(header, 34) != 0 || u32_at(header, 38) & 0x400 != 0 {
            return Err("ZIP has an invalid entry or a reparse-point attribute.".into());
        }
        let mut extra = &headers[cursor + 46 + name_size..cursor + 46 + name_size + extra_size];
        while !extra.is_empty() {
            if extra.len() < 4 {
                return Err("Invalid ZIP extra field.".into());
            }
            let kind = u16_at(extra, 0);
            let size = u16_at(extra, 2);
            if size + 4 > extra.len() || matches!(kind, 0x0001 | 0x000d | 0x756e) {
                return Err("ZIP64 and Unix link metadata are not supported.".into());
            }
            extra = &extra[4 + size..];
        }
        cursor = end;
    }
    if cursor != headers.len() {
        return Err("ZIP central-directory count does not match its data.".into());
    }
    input.rewind().map_err(|e| e.to_string())?;
    Ok(count)
}

fn verify_existing(
    directory: &mut Directory,
    path: &Path,
    prepared: &Prepared,
    cancel: &Cancel,
) -> Result<()> {
    regular_metadata(path, true)?;
    directory.directory(&format!("artifacts/{}", prepared.hash))?;
    let mut pending = vec![path.to_owned()];
    let mut found = BTreeMap::new();
    let mut entries = 0;
    while let Some(folder) = pending.pop() {
        let _pin = pin(&folder)?;
        for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
            cancel.check()?;
            entries += 1;
            if entries > MAX_ENTRIES * 32 {
                return Err("Stored package has too many entries.".into());
            }
            let target = entry.map_err(|e| e.to_string())?.path();
            let metadata = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
            regular_metadata(&target, metadata.is_dir())?;
            let relative = target
                .strip_prefix(path)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            crate::runtime_contract::relative_path(&relative)?;
            if metadata.is_dir() {
                directory.directory(&format!("artifacts/{}/{}", prepared.hash, relative))?;
                pending.push(target);
                continue;
            }
            let expected = prepared
                .files
                .iter()
                .find(|file| file.path == relative)
                .ok_or("Stored package has unexpected files. Retain it for repair.")?;
            let (size, hash) = digest(&mut read_file(&target)?, expected.size_bytes, cancel)?;
            if size != expected.size_bytes || hash != expected.sha256 {
                return Err(
                    "Stored package was changed. Retain it for repair; no files were overwritten."
                        .into(),
                );
            }
            found.insert(relative, (size, hash));
        }
    }
    if found.len() != prepared.files.len() {
        return Err("Stored package is incomplete. Retain it for repair.".into());
    }
    Ok(())
}
