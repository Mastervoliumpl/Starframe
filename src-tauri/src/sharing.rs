use crate::{
    packages::{Packages, Status as PackageStatus},
    references::Reference,
    storage::{Records, Storage},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

type Result<T> = std::result::Result<T, String>;
pub const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Portable {
    pub format: String,
    pub schema_version: u32,
    pub name: String,
    pub entries: Vec<Reference>,
}

impl Portable {
    pub fn read(text: &str) -> Result<Self> {
        if text.len() > MAX_BYTES {
            return Err("Collection files must be at most 1 MiB.".into());
        }
        let document: Self =
            serde_json::from_value(crate::runtime_contract::unique_json(text.as_bytes())?)
                .map_err(|e| format!("Invalid collection file: {e}"))?;
        document.validate()?;
        Ok(document)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.format != "starframe-collection" || self.schema_version != 2 {
            return Err("Unsupported collection format or version. Ask the sender for a version 2 Starframe collection.".into());
        }
        if self.name.trim().is_empty()
            || self.name.chars().count() > 200
            || self.name.chars().any(char::is_control)
        {
            return Err(
                "Collection names need 1–200 characters without control characters.".into(),
            );
        }
        if self.entries.len() > crate::runtime_contract::MAX_MODS {
            return Err("This runtime supports at most 256 active mods.".into());
        }
        let mut ids = HashSet::new();
        for reference in &self.entries {
            reference.validate()?;
            if !ids.insert(reference.runtime_id()) {
                return Err(
                    "A collection cannot contain two references with the same runtime identity."
                        .into(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "ImportStatus"))]
pub enum Status {
    Pending,
    Preparing,
    Ready,
    Unresolved,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "ImportEntry"))]
pub struct Entry {
    pub reference: Reference,
    pub status: Status,
    pub message: String,
    pub operation_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "CollectionImport"))]
pub struct Import {
    pub collection_id: String,
    pub entries: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "SharingAction"))]
pub enum Action {
    Review {
        text: String,
    },
    Accept {
        text: String,
        request_id: String,
        expected_revision: String,
    },
    Retry {
        id: String,
    },
    Export {
        id: String,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "SharingReply"))]
pub struct Reply {
    pub text: Option<String>,
    pub name: String,
    pub entries: Vec<Entry>,
    pub collection_id: Option<String>,
    pub order_error: Option<String>,
}

pub enum OnlineAction {
    Plan {
        id: String,
    },
    Fail {
        id: String,
        reference: crate::registry::ExactReference,
        message: String,
    },
    Install {
        id: String,
        reference: crate::registry::ExactReference,
        request: Box<crate::packages::RegistryRequest>,
        approval: Box<crate::packages::RegistryApproval>,
    },
}

pub enum OnlineReply {
    Plan(Vec<crate::registry::ExactReference>),
    Install(Option<crate::packages::Operation>),
    Failed,
}

pub fn online_action(
    store: &mut Storage,
    packages: &mut Packages,
    action: OnlineAction,
) -> Result<OnlineReply> {
    let id = match &action {
        OnlineAction::Plan { id }
        | OnlineAction::Fail { id, .. }
        | OnlineAction::Install { id, .. } => id,
    };
    let mut import = store
        .collection_imports()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|import| &import.collection_id == id)
        .ok_or("This imported collection no longer exists.")?;
    match action {
        OnlineAction::Plan { .. } => {
            let installed = crate::mods::library(store)?;
            let mut missing = Vec::new();
            for entry in &mut import.entries {
                let Reference::Registry(reference) = &entry.reference else {
                    continue;
                };
                if entry.status != Status::Unresolved
                    || installed
                        .iter()
                        .any(|installed| installed.reference == entry.reference)
                {
                    continue;
                }
                if let Err(message) =
                    store.require_registry_unblocked_hash(reference.sha256.as_str())
                {
                    entry.message = message.to_string();
                    continue;
                }
                entry.status = Status::Preparing;
                entry.operation_id = None;
                entry.message = "Checking fresh signed approval for this exact release.".into();
                missing.push(reference.clone());
            }
            if !missing.is_empty() {
                store
                    .save_import(&import)
                    .map_err(|error| error.to_string())?;
            }
            Ok(OnlineReply::Plan(missing))
        }
        OnlineAction::Fail {
            reference, message, ..
        } => {
            if let Some(entry) = import.entries.iter_mut().find(|entry| {
                entry.reference == Reference::Registry(reference.clone())
                    && entry.status == Status::Preparing
                    && entry.operation_id.is_none()
            }) {
                entry.status = Status::Unresolved;
                entry.message = message;
                store
                    .save_import(&import)
                    .map_err(|error| error.to_string())?;
            }
            Ok(OnlineReply::Failed)
        }
        OnlineAction::Install {
            reference,
            request,
            approval,
            ..
        } => {
            let entry = import
                .entries
                .iter_mut()
                .find(|entry| {
                    entry.reference == Reference::Registry(reference.clone())
                        && entry.status == Status::Preparing
                        && entry.operation_id.is_none()
                })
                .ok_or("This exact shared download is no longer pending.")?;
            if *request.auth_cancel.borrow() {
                return Err("Sign-in changed. Retry import after signing in.".into());
            }
            store
                .require_registry_unblocked_hash(reference.sha256.as_str())
                .map_err(|error| error.to_string())?;
            if !packages.can_start(reference.sha256.as_str()) {
                return Ok(OnlineReply::Install(None));
            }
            let operation = if store
                .installed_registry_releases()
                .map_err(|error| error.to_string())?
                .iter()
                .any(|entry| entry.reference == reference)
            {
                packages.verify_registry_installed(store, &reference)?
            } else {
                crate::backend::approved_registry_install(
                    store,
                    packages,
                    &Uuid::new_v4().to_string(),
                    &reference,
                    *request,
                    *approval,
                )?
            };
            entry.operation_id = Some(operation.id.clone());
            entry.message =
                "Preparing the exact signed release. See Downloads for progress or cancellation."
                    .into();
            if let Err(error) = store.save_import(&import) {
                packages.cancel(store, &operation.id)?;
                return Err(error.to_string());
            }
            Ok(OnlineReply::Install(Some(operation)))
        }
    }
}

fn assess(store: &Storage, reference: &Reference) -> Result<String> {
    store
        .require_registry_unblocked_hash(reference.hash())
        .map_err(|error| error.to_string())?;
    if crate::mods::library(store)?
        .iter()
        .any(|entry| &entry.reference == reference)
    {
        return Ok(
            "Matching exact content is available; verify its files for offline reuse.".into(),
        );
    }
    Err(match reference {
        Reference::Local(_)=>"Local-only content is missing. Obtain and import this exact build from its author; no registry download is approved.",
        Reference::Registry(_)=>"The exact registry release is missing. A signed-in registry download is required; no replacement was selected.",
    }.into())
}
fn review(store: &Storage, document: &Portable) -> Result<Reply> {
    let entries = document
        .entries
        .iter()
        .map(|reference| {
            let (status, message) = match assess(store, reference) {
                Ok(message) => (Status::Pending, message),
                Err(message) => (Status::Unresolved, message),
            };
            Entry {
                reference: reference.clone(),
                status,
                message,
                operation_id: None,
            }
        })
        .collect();
    let order_error = if document.entries.is_empty() {
        None
    } else {
        crate::mods::resolve_order(store, &document.entries).err()
    };
    Ok(Reply {
        text: None,
        name: document.name.clone(),
        entries,
        collection_id: None,
        order_error,
    })
}

pub fn action(store: &mut Storage, action: Action) -> Result<Reply> {
    match action {
        Action::Export { id } => {
            let records = store.load().map_err(|e| e.to_string())?;
            let collection = records
                .collections
                .iter()
                .find(|c| c.id == id)
                .ok_or("This collection no longer exists.")?;
            let document = Portable {
                format: "starframe-collection".into(),
                schema_version: 2,
                name: collection.name.clone(),
                entries: collection.entries.clone(),
            };
            document.validate()?;
            Ok(Reply {
                text: Some(serde_json::to_string_pretty(&document).map_err(|e| e.to_string())?),
                name: document.name,
                entries: vec![],
                collection_id: Some(id),
                order_error: None,
            })
        }
        Action::Review { text } => review(store, &Portable::read(&text)?),
        Action::Accept {
            text,
            request_id,
            expected_revision,
        } => {
            Uuid::parse_str(&request_id).map_err(|_| "Invalid collection import request ID.")?;
            let document = Portable::read(&text)?;
            let mut reply = review(store, &document)?;
            if let Some(existing) = store
                .load()
                .map_err(|e| e.to_string())?
                .collections
                .into_iter()
                .find(|c| c.id == request_id)
            {
                if existing.name != document.name || existing.entries != document.entries {
                    return Err("This request ID belongs to another collection.".into());
                }
            } else {
                store
                    .create_import(
                        &document,
                        &Import {
                            collection_id: request_id.clone(),
                            entries: reply.entries.clone(),
                        },
                        expected_revision
                            .parse()
                            .map_err(|_| "Invalid saved-data revision.")?,
                    )
                    .map_err(|e| e.to_string())?;
            }
            reply.collection_id = Some(request_id);
            Ok(reply)
        }
        Action::Retry { id } => {
            let records = store.load().map_err(|e| e.to_string())?;
            let collection = records
                .collections
                .iter()
                .find(|c| c.id == id)
                .ok_or("This collection no longer exists.")?;
            let mut reply = review(
                store,
                &Portable {
                    format: "starframe-collection".into(),
                    schema_version: 2,
                    name: collection.name.clone(),
                    entries: collection.entries.clone(),
                },
            )?;
            let imports = store.collection_imports().map_err(|e| e.to_string())?;
            let old = imports
                .iter()
                .find(|i| i.collection_id == id)
                .ok_or("This collection was not imported.")?;
            if old
                .entries
                .iter()
                .any(|e| matches!(e.status, Status::Pending | Status::Preparing))
            {
                return Err(
                    "This import is still running. Wait or cancel its current download.".into(),
                );
            }
            store
                .save_import(&Import {
                    collection_id: id.clone(),
                    entries: reply.entries.clone(),
                })
                .map_err(|e| e.to_string())?;
            reply.collection_id = Some(id);
            Ok(reply)
        }
    }
}

pub fn recover(store: &mut Storage) -> Result<()> {
    for mut import in store.collection_imports().map_err(|e| e.to_string())? {
        let mut changed = false;
        for entry in &mut import.entries {
            if matches!(entry.status, Status::Pending | Status::Preparing) {
                entry.status = Status::Unresolved;
                entry.message = "Import stopped when Starframe closed. Retry import to verify or prepare the remaining content.".into();
                changed = true;
            }
        }
        if changed {
            store.save_import(&import).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn poll(store: &mut Storage, packages: &mut Packages) -> Result<bool> {
    let operations = packages.operations(store)?;
    let mut changed = false;
    for mut import in store
        .collection_imports()
        .map_err(|error| error.to_string())?
    {
        let mut dirty = false;
        for entry in &mut import.entries {
            if entry.status == Status::Preparing {
                if entry.operation_id.is_none() {
                    continue;
                }
                let operation = operations
                    .iter()
                    .find(|operation| Some(&operation.id) == entry.operation_id.as_ref());
                if operation.is_some_and(|operation| {
                    matches!(
                        operation.status,
                        PackageStatus::Preparing | PackageStatus::Cancelling
                    )
                }) {
                    continue;
                }
                match operation {
                    Some(operation) if operation.status == PackageStatus::Completed => {
                        match assess(store, &entry.reference) {
                            Ok(_) => {
                                entry.status = Status::Ready;
                                entry.message = "Exact content verified and available.".into();
                            }
                            Err(message) => {
                                entry.status = Status::Unresolved;
                                entry.message = message;
                            }
                        }
                    }
                    Some(operation) => {
                        entry.status = Status::Unresolved;
                        entry.message = operation.message.clone();
                    }
                    None => {
                        entry.status = Status::Unresolved;
                        entry.message = "Verification operation is missing. Retry import.".into();
                    }
                }
                dirty = true;
            }
            if entry.status != Status::Pending || !packages.can_start(entry.reference.hash()) {
                continue;
            }
            let result = assess(store, &entry.reference).and_then(|_| match &entry.reference {
                Reference::Local(_) => {
                    packages.verify_local(store, &entry.reference.local_reference().unwrap())
                }
                Reference::Registry(reference) => {
                    packages.verify_registry_installed(store, reference)
                }
            });
            match result {
                Ok(operation) => {
                    entry.status = Status::Preparing;
                    entry.message =
                        "Verifying exact content. See Downloads for progress or cancellation."
                            .into();
                    entry.operation_id = Some(operation.id);
                }
                Err(message) => {
                    entry.status = Status::Unresolved;
                    entry.message = message;
                }
            }
            dirty = true;
        }
        if dirty {
            store
                .save_import(&import)
                .map_err(|error| error.to_string())?;
            changed = true;
        }
    }
    Ok(changed)
}
pub(crate) fn active_error(store: &Storage, records: &Records) -> Result<Option<String>> {
    let Some(id) = &records.active_collection else {
        return Ok(None);
    };
    let imports = store.collection_imports().map_err(|e| e.to_string())?;
    let Some(import) = imports.iter().find(|i| &i.collection_id == id) else {
        return Ok(None);
    };
    let collection = records
        .collections
        .iter()
        .find(|c| &c.id == id)
        .ok_or("Active collection is missing.")?;
    Ok(collection.entries.iter().find_map(|reference| {
        import
            .entries
            .iter()
            .find(|e| &e.reference == reference && e.status != Status::Ready)
            .map(|e| format!("{}: {}", reference.runtime_id(), e.message))
    }))
}
