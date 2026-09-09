use crate::{
    catalog::{Artifact, Catalog, Layout},
    filesystem::{pin, read_file, regular_metadata},
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
use tokio::{io::AsyncWriteExt, sync::watch as cancellation};
use uuid::Uuid;

mod archive;
mod artifacts;
mod local;
#[cfg(test)]
mod tests;
mod transfer;
pub mod watch;
use archive::*;
pub(crate) use archive::{layout, supported_files};
use artifacts::*;
pub(crate) use artifacts::{Directory, remove_artifact, verify_artifact};

type Result<T> = std::result::Result<T, String>;
const MAX_ENTRIES: usize = 4096;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const CANCELLED: &str = "Package preparation cancelled. No game files changed.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "PackageStatus"))]
pub enum Status {
    Preparing,
    Cancelling,
    Cancelled,
    Completed,
    Failed,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "PackageAction"))]
pub enum Action {
    #[serde(rename_all = "camelCase")]
    ImportLocal {
        request_id: String,
        path: String,
    },
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
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "PackageOperation"))]
pub struct Operation {
    pub id: String,
    pub request_id: String,
    pub release_id: String,
    pub hash: String,
    pub status: Status,
    pub message: String,
    #[cfg_attr(test, ts(type = "number"))]
    pub received_bytes: u64,
    #[cfg_attr(test, ts(type = "number"))]
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
    signal: cancellation::Sender<bool>,
}
impl Default for Cancel {
    fn default() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            signal: cancellation::channel(false).0,
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
    entry: Option<LibraryEntry>,
    source: Option<String>,
    cancel: Cancel,
    progress: Arc<AtomicU64>,
    result: mpsc::Receiver<Result<PreparedImport>>,
    ready: Option<Result<PreparedImport>>,
}

#[derive(Clone)]
struct PreparedImport {
    prepared: Prepared,
    local: Option<crate::local_import::LocalSource>,
}

/// The existing storage worker owns this queue and calls `poll` to commit results.
/// Dropping the queue cancels workers; startup records unfinished work as failed.
pub struct Packages {
    client: reqwest::Client,
    active: HashMap<String, Active>,
}

impl Packages {
    pub fn busy(&self) -> bool {
        !self.active.is_empty()
    }
    pub(crate) fn can_start(&self, hash: &str) -> bool {
        self.active.len() < 3 && !self.busy_hash(hash)
    }
    pub fn busy_hash(&self, hash: &str) -> bool {
        self.active
            .values()
            .any(|active| active.source.is_some() || active.operation.hash == hash)
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
        if let Some(security) = storage.catalog_security().map_err(|e| e.to_string())? {
            let prepared = storage
                .prepared_artifact(&artifact.sha256)
                .map_err(|e| e.to_string())?;
            security
                .require_allowed(
                    &artifact.sha256,
                    prepared.as_ref().map_or(&[], |p| p.files.as_slice()),
                )
                .map_err(|e| e.to_string())?;
        }
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
            let _ = sender.send(outcome.map(|prepared| PreparedImport {
                prepared,
                local: None,
            }));
        });
        self.active.insert(
            operation.id.clone(),
            Active {
                operation: operation.clone(),
                entry: Some(entry),
                source: None,
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
        let prepared = storage
            .prepared_artifact(&reference.hash)
            .map_err(|e| e.to_string())?
            .ok_or(
                "This local content has no verified file manifest. Import the exact build again.",
            )?;
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
            let _ = sender.send(outcome.map(|prepared| PreparedImport {
                prepared,
                local: None,
            }));
        });
        self.active.insert(
            operation.id.clone(),
            Active {
                operation: operation.clone(),
                entry: Some(entry),
                source: None,
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
                    Err(_) => Err(
                        "Package worker stopped unexpectedly. Retry package preparation.".into(),
                    ),
                });
            }
            // Completion is committed here, so cancellation can still win after extraction.
            let outcome = active.ready.as_ref().unwrap().clone().and_then(|result| {
                active.cancel.check()?;
                if result.local.is_some() {
                    return Ok(result);
                }
                let entry = active
                    .entry
                    .as_ref()
                    .ok_or("Package metadata is missing.")?;
                if entry.reference.origin == Origin::LocalImport {
                    if !storage
                        .load()
                        .map_err(|e| e.to_string())?
                        .library
                        .iter()
                        .any(|e| e.reference == entry.reference)
                    {
                        return Err("The local reference changed during verification.".into());
                    }
                    return Ok(result);
                }
                let catalog = storage
                    .catalog_cache()
                    .map_err(|e| e.to_string())?
                    .and_then(|cache| cache.catalog)
                    .ok_or("Approved catalog is unavailable. Retry after refresh.")?;
                if catalog.downloadable(&active.operation.release_id)?.sha256
                    != result.prepared.hash
                {
                    return Err("Release identity changed during package preparation.".into());
                }
                Ok(result)
            });
            match outcome {
                Ok(result) => {
                    let prepared = result.prepared;
                    if result.local.is_some() {
                        active.operation.total_bytes = prepared
                            .files
                            .iter()
                            .map(|f| f.size_bytes)
                            .sum::<u64>()
                            .max(1);
                    }
                    let entry = result
                        .local
                        .as_ref()
                        .map(|local| local.entry())
                        .or_else(|| active.entry.clone())
                        .ok_or("Package metadata is missing.")?;
                    active.operation.hash = prepared.hash.clone();
                    active.operation.status = Status::Completed;
                    active.operation.received_bytes = active.operation.total_bytes;
                    active.operation.message = "Verified package saved in the library.".into();
                    if let Err(error) = if result.local.is_some() {
                        storage.complete_import(
                            &active.operation,
                            &entry,
                            &prepared,
                            result.local.as_ref(),
                        )
                    } else if entry.reference.origin == Origin::LocalImport {
                        storage.save_package(&active.operation)
                    } else {
                        storage.complete_package(&active.operation, &entry, &prepared)
                    } {
                        active.operation.status = Status::Failed;
                        active.operation.message = format!(
                            "Could not save the prepared package: {error}. Retry preparation; verified content was retained."
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
