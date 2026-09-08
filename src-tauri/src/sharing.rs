use crate::{
    catalog::Catalog,
    packages::{Packages, Status as PackageStatus},
    storage::{ModReference, Origin, Records, Storage},
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
    pub entries: Vec<ModReference>,
}

impl Portable {
    pub fn read(text: &str) -> Result<Self> {
        if text.len() > MAX_BYTES {
            return Err("Collection files must be at most 1 MiB.".into());
        }
        let document: Self =
            serde_json::from_str(text).map_err(|e| format!("Invalid collection file: {e}"))?;
        document.validate()?;
        Ok(document)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.format != "starframe-collection" || self.schema_version != 1 {
            return Err("Unsupported collection format or version. Ask the sender for a version 1 Starframe collection.".into());
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
            crate::storage::validate_reference(reference).map_err(|e| e.to_string())?;
            for id in std::iter::once(&reference.mod_id).chain(reference.release_id.iter()) {
                if id.len() > 128
                    || !id.bytes().enumerate().all(|(i, b)| {
                        b.is_ascii_lowercase()
                            || b.is_ascii_digit()
                            || (i > 0 && b"._-".contains(&b))
                    })
                {
                    return Err("Mod and release IDs must use lowercase letters, digits, dots, underscores or hyphens.".into());
                }
            }
            if !ids.insert(&reference.mod_id) {
                return Err("A collection cannot contain two references to the same mod.".into());
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
    pub reference: ModReference,
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

fn assess(
    reference: &ModReference,
    records: &Records,
    catalog: Option<&Catalog>,
    requested: &[ModReference],
) -> Result<String> {
    let exact = records.library.iter().any(|e| &e.reference == reference);
    if reference.origin == Origin::LocalImport {
        return if exact {
            Ok("Matching local content is present; verify its files before reuse.".into())
        } else {
            Err("Local-only content is missing. Obtain this exact build from its author; no download source is approved.".into())
        };
    }
    let catalog = catalog.ok_or("Approved catalog is unavailable. Retry after refresh.")?;
    let (owner, release) = catalog.releases().find(|(_, r)| Some(&r.id) == reference.release_id.as_ref())
        .ok_or("The exact release is not in the approved catalog. Ask the sender to repair this reference.")?;
    if owner.id != reference.mod_id || release.artifact.sha256 != reference.hash {
        return Err(
            "The shared identity differs from catalog approval. No substitute was selected.".into(),
        );
    }
    catalog.downloadable(&release.id)?;
    for dependency in &release.requires {
        if !requested.iter().any(|r| {
            r.origin == Origin::Catalog
                && r.release_id.as_ref() == Some(dependency)
                && catalog.releases().any(|(m, release)| {
                    m.id == r.mod_id
                        && release.id == *dependency
                        && release.artifact.sha256 == r.hash
                })
        }) {
            return Err(format!(
                "Required release {dependency} is missing from this collection. Ask the sender to include its exact reference."
            ));
        }
    }
    Ok(if exact {
        "Already downloaded; verify local files before reuse."
    } else {
        "Download and verify this exact approved release."
    }
    .into())
}

fn review(store: &Storage, document: &Portable) -> Result<Reply> {
    let records = store.load().map_err(|e| e.to_string())?;
    let catalog = store
        .catalog_cache()
        .map_err(|e| e.to_string())?
        .and_then(|c| c.catalog);
    let entries = document
        .entries
        .iter()
        .map(|reference| {
            let (status, message) =
                match assess(reference, &records, catalog.as_ref(), &document.entries) {
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
        catalog.as_ref().map_or_else(
            || Some("Catalog metadata is unavailable.".into()),
            |c| crate::ordering::resolve(c, &document.entries).err(),
        )
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
                schema_version: 1,
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
                    schema_version: 1,
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
    let records = store.load().map_err(|e| e.to_string())?;
    let catalog = store
        .catalog_cache()
        .map_err(|e| e.to_string())?
        .and_then(|c| c.catalog);
    let operations = packages.operations(store)?;
    let mut changed = false;
    for mut import in store.collection_imports().map_err(|e| e.to_string())? {
        let mut dirty = false;
        let requested = &records
            .collections
            .iter()
            .find(|c| c.id == import.collection_id)
            .ok_or("Imported collection is missing.")?
            .entries;
        for entry in &mut import.entries {
            if entry.status == Status::Preparing {
                let operation = operations
                    .iter()
                    .find(|op| Some(&op.id) == entry.operation_id.as_ref());
                match operation {
                    Some(op)
                        if matches!(
                            op.status,
                            PackageStatus::Preparing | PackageStatus::Cancelling
                        ) =>
                    {
                        continue;
                    }
                    Some(op)
                        if op.status == PackageStatus::Completed
                            && records
                                .library
                                .iter()
                                .any(|e| e.reference == entry.reference) =>
                    {
                        entry.status = Status::Ready;
                        entry.message = "Exact package verified and available.".into();
                    }
                    other => {
                        entry.status = Status::Unresolved;
                        entry.message = other.map_or("Package operation is missing. Retry import.".into(), |op| if op.status == PackageStatus::Completed { "The saved approval identity does not match. No substitute was selected.".into() } else { op.message.clone() });
                    }
                }
                dirty = true;
            }
            if entry.status != Status::Pending {
                continue;
            }
            match assess(&entry.reference, &records, catalog.as_ref(), requested) {
                Err(message) => {
                    entry.status = Status::Unresolved;
                    entry.message = message;
                    dirty = true;
                }
                Ok(_) if !packages.can_start(&entry.reference.hash) => (),
                Ok(_) => {
                    let result = if entry.reference.origin == Origin::LocalImport {
                        packages.verify_local(store, &entry.reference)
                    } else {
                        packages.start(
                            store,
                            &Uuid::new_v4().to_string(),
                            entry.reference.release_id.as_deref().unwrap(),
                        )
                    };
                    match result {
                        Ok(op) => {
                            entry.status = Status::Preparing;
                            entry.message = "Preparing the exact package. See Downloads for progress or cancellation.".into();
                            entry.operation_id = Some(op.id);
                        }
                        Err(message) => {
                            entry.status = Status::Unresolved;
                            entry.message = message;
                        }
                    }
                    dirty = true;
                }
            }
        }
        if dirty {
            store.save_import(&import).map_err(|e| e.to_string())?;
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
            .map(|e| format!("{}: {}", reference.mod_id, e.message))
    }))
}
