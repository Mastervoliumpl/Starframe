use crate::{
    packages,
    references::Reference,
    registry::installation::Installation,
    storage::{Origin, Records, Storage},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
mod local;
pub(crate) use local::{metadata, payload_entries, requested_local};
#[cfg(test)]
mod tests;
type Result<T> = std::result::Result<T, String>;

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "ModAction"))]
pub enum Action {
    List,
    RetryCleanup,
    CreateCollection {
        name: String,
        expected_revision: String,
    },
    RenameCollection {
        id: String,
        name: String,
        expected_revision: String,
    },
    DeleteCollection {
        id: String,
        expected_revision: String,
    },
    SelectCollection {
        id: String,
        expected_revision: String,
    },
    Reorder {
        mod_ids: Vec<String>,
        expected_revision: String,
    },
    SetEnabled {
        reference: Reference,
        enabled: bool,
        expected_revision: String,
    },
    Uninstall {
        reference: Reference,
        expected_revision: String,
        confirm_references: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "InstalledEntry"))]
pub struct LibraryEntry {
    pub reference: Reference,
    pub name: String,
    pub author: String,
    pub version: String,
    pub kind: String,
    pub tested_game_build: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "ModView"))]
pub struct View {
    pub blocked: BTreeMap<String, String>,
    pub revision: String,
    pub library: Vec<LibraryEntry>,
    pub local_sources: Vec<crate::local_import::LocalSource>,
    pub local_watches: Vec<crate::local_import::LocalWatch>,
    pub enabled: Vec<Reference>,
    pub order: Option<crate::ordering::MixedResolution>,
    pub order_error: Option<String>,
    pub collisions: Vec<Collision>,
    pub collections: Vec<crate::storage::Collection>,
    pub active_collection: Option<String>,
    pub cleanup_errors: Vec<String>,
    pub imports: Vec<crate::sharing::Import>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Collision {
    pub path: String,
    pub mods: Vec<String>,
    pub winner: String,
}

pub fn library(store: &Storage) -> Result<Vec<LibraryEntry>> {
    let records = store.load().map_err(|error| error.to_string())?;
    let mut entries = records
        .library
        .iter()
        .filter(|entry| entry.reference.origin == Origin::LocalImport)
        .map(|entry| {
            Ok(LibraryEntry {
                reference: Reference::try_from(&entry.reference)?,
                name: entry.name.clone(),
                author: entry.author.clone(),
                version: entry.version.clone(),
                kind: "code".into(),
                tested_game_build: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    entries.extend(
        store
            .installed_registry_releases()
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|entry| LibraryEntry {
                reference: Reference::Registry(entry.reference),
                name: entry.display.name,
                author: entry.display.author,
                version: entry.version_label,
                kind: match entry.installation {
                    Installation::Content {
                        kind: crate::registry::installation::Kind::Map,
                        ..
                    } => "map",
                    Installation::Content { .. } => "ai",
                    _ => "code",
                }
                .into(),
                tested_game_build: Some(entry.tested_game_build),
            }),
    );
    Ok(entries)
}

pub(crate) fn resolve_order(
    store: &Storage,
    entries: &[Reference],
) -> Result<crate::ordering::MixedResolution> {
    crate::ordering::resolve_mixed(
        &store
            .installed_registry_releases()
            .map_err(|error| error.to_string())?,
        &store.local_sources().map_err(|error| error.to_string())?,
        entries,
    )
}

pub fn view(store: &Storage) -> Result<View> {
    let records = store.load().map_err(|error| error.to_string())?;
    let enabled = active(&records).to_vec();
    let order = match crate::sharing::active_error(store, &records)? {
        Some(error) => Err(error),
        None => resolve_order(store, &enabled),
    };
    let (order, mut order_error) = match order {
        Ok(order) => (Some(order), None),
        Err(error) => (None, Some(error)),
    };
    let mut overlays = BTreeMap::<String, Vec<String>>::new();
    if let Some(order) = &order {
        let activation = crate::selection::requested(store, &order.effective);
        match activation {
            Err(error) => order_error = Some(error),
            Ok(activation) => {
                for item in activation["mods"].as_array().unwrap() {
                    if item["entryAssembly"].is_null() {
                        for file in item["files"].as_array().unwrap() {
                            overlays
                                .entry(file["path"].as_str().unwrap().to_ascii_lowercase())
                                .or_default()
                                .push(item["modId"].as_str().unwrap().to_owned());
                        }
                    }
                }
            }
        }
    }
    let collisions = overlays
        .into_iter()
        .filter_map(|(path, mods)| {
            (mods.len() > 1).then(|| Collision {
                path,
                winner: mods.last().unwrap().clone(),
                mods,
            })
        })
        .collect();
    let blocked = store
        .registry_decisions()
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|decision| {
            matches!(
                decision.status,
                crate::registry::trust::DecisionStatus::Blocked
            )
        })
        .map(|decision| (decision.sha256.as_str().to_owned(), decision.reason))
        .collect();
    Ok(View {
        blocked,
        revision: records.revision.to_string(),
        library: library(store)?,
        local_sources: store.local_sources().map_err(|error| error.to_string())?,
        local_watches: store.local_watches().map_err(|error| error.to_string())?,
        enabled,
        order,
        order_error,
        collisions,
        collections: records.collections,
        active_collection: records.active_collection,
        cleanup_errors: store
            .pending_removals()
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|(hash, error)| format!("{hash}: {error}"))
            .collect(),
        imports: store
            .collection_imports()
            .map_err(|error| error.to_string())?,
    })
}

fn active(records: &Records) -> &[Reference] {
    records
        .collections
        .iter()
        .find(|collection| Some(&collection.id) == records.active_collection.as_ref())
        .map_or(&[], |collection| collection.entries.as_slice())
}

pub fn action(store: &mut Storage, action: Action) -> Result<View> {
    match action {
        Action::List => (),
        Action::RetryCleanup => cleanup(store)?,
        Action::CreateCollection {
            name,
            expected_revision,
        } => {
            current_records(store, &expected_revision)?;
            store
                .save_collection(&uuid::Uuid::new_v4().to_string(), name.trim(), &[], 0)
                .map_err(|error| error.to_string())?;
        }
        Action::RenameCollection {
            id,
            name,
            expected_revision,
        } => {
            let records = current_records(store, &expected_revision)?;
            let collection = records
                .collections
                .iter()
                .find(|collection| collection.id == id)
                .ok_or("This collection no longer exists.")?;
            store
                .save_collection(&id, name.trim(), &collection.entries, collection.revision)
                .map_err(|error| error.to_string())?;
        }
        Action::DeleteCollection {
            id,
            expected_revision,
        } => {
            store
                .delete_collection(&id, revision(&expected_revision)?)
                .map_err(|error| error.to_string())?;
        }
        Action::SelectCollection {
            id,
            expected_revision,
        } => {
            let records = current_records(store, &expected_revision)?;
            if !records
                .collections
                .iter()
                .any(|collection| collection.id == id)
            {
                return Err("This collection no longer exists.".into());
            }
            store
                .set_active_collection(Some(&id), records.revision)
                .map_err(|error| error.to_string())?;
        }
        Action::Reorder {
            mod_ids,
            expected_revision,
        } => {
            let records = current_records(store, &expected_revision)?;
            let entries = active(&records);
            if mod_ids.len() != entries.len()
                || mod_ids.iter().collect::<BTreeSet<_>>().len() != entries.len()
            {
                return Err("Reordering must include each enabled mod exactly once.".into());
            }
            let reordered = mod_ids
                .iter()
                .map(|id| {
                    entries
                        .iter()
                        .find(|reference| &reference.runtime_id() == id)
                        .cloned()
                        .ok_or_else(|| "Reordering cannot change collection membership.".into())
                })
                .collect::<Result<Vec<_>>>()?;
            resolve_order(store, &reordered)?;
            store
                .set_mod_membership(&reordered, records.revision)
                .map_err(|error| error.to_string())?;
        }
        Action::SetEnabled {
            reference,
            enabled,
            expected_revision,
        } => {
            reference.validate()?;
            let records = current_records(store, &expected_revision)?;
            if !library(store)?
                .iter()
                .any(|entry| entry.reference == reference)
            {
                return Err("This exact package is not in the library.".into());
            }
            let mut entries = active(&records).to_vec();
            if enabled {
                let needed = local_requirements(store, &reference, &mut BTreeSet::new())?;
                for dependency in needed {
                    if dependency != reference
                        && entries.iter().any(|existing| {
                            existing.runtime_id() == dependency.runtime_id()
                                && existing != &dependency
                        })
                    {
                        return Err("A local dependency conflicts with a pinned collection entry. Choose the required exact build explicitly.".into());
                    }
                    if let Some(position) = entries
                        .iter()
                        .position(|existing| existing.runtime_id() == dependency.runtime_id())
                    {
                        entries[position] = dependency;
                    } else {
                        entries.push(dependency);
                    }
                }
                crate::selection::requested(store, &entries)?;
                crate::selection::verify(store, &entries)?;
            } else {
                entries.retain(|existing| existing != &reference);
            }
            store
                .set_mod_membership(&entries, records.revision)
                .map_err(|error| error.to_string())?;
        }
        Action::Uninstall {
            reference,
            expected_revision,
            confirm_references,
        } => {
            store
                .uninstall_reference(
                    &reference,
                    revision(&expected_revision)?,
                    confirm_references,
                )
                .map_err(|error| error.to_string())?;
            cleanup(store)?;
        }
    }
    view(store)
}

fn local_requirements(
    store: &Storage,
    reference: &Reference,
    visiting: &mut BTreeSet<String>,
) -> Result<Vec<Reference>> {
    if !visiting.insert(reference.runtime_id()) {
        return Err("Conflicting or cyclic local dependencies.".into());
    }
    let mut needed = Vec::new();
    if let Some(local) = reference.local_reference() {
        let sources = store.local_sources().map_err(|error| error.to_string())?;
        let source = sources
            .iter()
            .find(|source| source.reference == local)
            .ok_or("Exact local metadata is missing.")?;
        for required in &source.manifest.requires {
            let required = Reference::try_from(required)?;
            needed.extend(local_requirements(store, &required, visiting)?);
        }
    }
    visiting.remove(&reference.runtime_id());
    if needed
        .iter()
        .any(|existing| existing.runtime_id() == reference.runtime_id() && existing != reference)
    {
        return Err("Local dependencies require conflicting builds.".into());
    }
    if !needed.contains(reference) {
        needed.push(reference.clone());
    }
    Ok(needed)
}

pub fn requested(store: &Storage) -> Result<Value> {
    let records = store.load().map_err(|error| error.to_string())?;
    if let Some(error) = crate::sharing::active_error(store, &records)? {
        return Err(error);
    }
    crate::selection::requested(store, active(&records))
}

pub(crate) fn payload(
    store: &Storage,
    activation: &Value,
) -> Result<Vec<(String, crate::deployment::Source)>> {
    let records = store.load().map_err(|error| error.to_string())?;
    crate::selection::payload(store, active(&records), activation)
}

pub fn cleanup(store: &mut Storage) -> Result<()> {
    for (hash, _) in store
        .pending_removals()
        .map_err(|error| error.to_string())?
    {
        let result = packages::remove_artifact(store, &hash);
        store
            .finish_removal(&hash, result.err().as_deref())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn revision(value: &str) -> Result<i64> {
    let parsed = value
        .parse::<i64>()
        .map_err(|_| "Invalid library revision.")?;
    if parsed < 0 || parsed.to_string() != value {
        return Err("Invalid library revision.".into());
    }
    Ok(parsed)
}
fn current_records(store: &Storage, expected: &str) -> Result<Records> {
    let records = store.load().map_err(|error| error.to_string())?;
    if records.revision != revision(expected)? {
        return Err("The library changed. Retry with its current revision.".into());
    }
    Ok(records)
}
