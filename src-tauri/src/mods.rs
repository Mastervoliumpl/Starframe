use crate::{
    catalog::{Catalog, Layout},
    deployment::Source,
    packages, runtime_contract,
    storage::{LibraryEntry, ModReference, Origin, Records, Storage},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

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
        reference: ModReference,
        enabled: bool,
        expected_revision: String,
    },
    Uninstall {
        reference: ModReference,
        expected_revision: String,
        confirm_references: bool,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "ModView"))]
pub struct View {
    pub catalog: Option<Catalog>,
    pub advisories: Option<crate::catalog::advisories::Advisories>,
    pub findings: std::collections::BTreeMap<String, Vec<String>>,
    pub revision: String,
    pub library: Vec<LibraryEntry>,
    pub local_sources: Vec<crate::local_import::LocalSource>,
    pub local_watches: Vec<crate::local_import::LocalWatch>,
    pub enabled: Vec<ModReference>,
    pub order: Option<crate::ordering::Resolution>,
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

pub fn view(store: &Storage) -> Result<View> {
    let records = store.load().map_err(|e| e.to_string())?;
    let enabled = records
        .collections
        .iter()
        .find(|c| Some(&c.id) == records.active_collection.as_ref())
        .map_or(vec![], |c| c.entries.clone());
    let catalog = store
        .catalog_cache()
        .map_err(|e| e.to_string())?
        .and_then(|c| c.catalog);
    let locals = store.local_sources().map_err(|e| e.to_string())?;
    let advisories = store
        .catalog_security()
        .map_err(|e| e.to_string())?
        .map(|s| s.advisories().clone());
    let mut findings = std::collections::BTreeMap::new();
    if let Some(advisories) = &advisories
        && !advisories.advisories.is_empty()
    {
        let installed: BTreeSet<_> = records
            .library
            .iter()
            .map(|entry| &entry.reference.hash)
            .collect();
        let hashes: BTreeSet<_> = installed
            .iter()
            .copied()
            .chain(
                catalog
                    .iter()
                    .flat_map(|c| c.releases().map(|(_, r)| &r.artifact.sha256)),
            )
            .collect();
        let has_payloads = advisories
            .advisories
            .iter()
            .any(|a| a.affected.iter().any(|a| !a.payload_sha256.is_empty()));
        for hash in hashes {
            let prepared = if has_payloads && installed.contains(hash) {
                store.prepared_artifact(hash).map_err(|e| e.to_string())?
            } else {
                None
            };
            let matches =
                advisories.history_for(hash, prepared.as_ref().map_or(&[], |p| p.files.as_slice()));
            if !matches.is_empty() {
                findings.insert(
                    hash.clone(),
                    matches.into_iter().map(|a| a.id.clone()).collect(),
                );
            }
        }
    }
    let order = if let Some(error) = crate::sharing::active_error(store, &records)? {
        Err(error)
    } else if let Some(missing) = enabled
        .iter()
        .find(|r| !records.library.iter().any(|e| &e.reference == *r))
    {
        Err(format!(
            "{} is unavailable in the library. Prepare its exact package from Catalog to use this collection.",
            missing.mod_id
        ))
    } else if enabled.is_empty() {
        Ok(crate::ordering::Resolution {
            effective: vec![],
            adjustments: vec![],
        })
    } else {
        crate::ordering::resolve_with_locals(catalog.as_ref(), &locals, &enabled)
    };
    let (order, order_error) = match order {
        Ok(order) => (Some(order), None),
        Err(error) => (None, Some(error)),
    };
    let mut overlays = std::collections::BTreeMap::<String, Vec<String>>::new();
    if let Some(order) = &order {
        for reference in &order.effective {
            if matches!(
                metadata(catalog.as_ref(), &locals, reference)?.layout,
                Layout::StarframeLuaZip {}
            ) {
                let prepared = store
                    .prepared_artifact(&reference.hash)
                    .map_err(|e| e.to_string())?
                    .ok_or("Prepared Lua inventory is missing. Prepare the exact package again.")?;
                for file in prepared.files {
                    overlays
                        .entry(file.path.to_ascii_lowercase())
                        .or_default()
                        .push(reference.mod_id.clone());
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
    Ok(View {
        imports: store.collection_imports().map_err(|e| e.to_string())?,
        catalog,
        advisories,
        findings,
        order,
        order_error,
        collisions,
        revision: records.revision.to_string(),
        library: records.library,
        local_sources: locals,
        local_watches: store.local_watches().map_err(|e| e.to_string())?,
        enabled,
        collections: records.collections,
        active_collection: records.active_collection,
        cleanup_errors: store
            .pending_removals()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(hash, error)| format!("{hash}: {error}"))
            .collect(),
    })
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
                .map_err(|e| e.to_string())?;
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
                .find(|c| c.id == id)
                .ok_or("This collection no longer exists.")?;
            store
                .save_collection(&id, name.trim(), &collection.entries, collection.revision)
                .map_err(|e| e.to_string())?;
        }
        Action::DeleteCollection {
            id,
            expected_revision,
        } => {
            store
                .delete_collection(&id, revision(&expected_revision)?)
                .map_err(|e| e.to_string())?;
        }
        Action::SelectCollection {
            id,
            expected_revision,
        } => {
            let records = current_records(store, &expected_revision)?;
            if !records.collections.iter().any(|c| c.id == id) {
                return Err("This collection no longer exists.".into());
            }
            store
                .set_active_collection(Some(&id), records.revision)
                .map_err(|e| e.to_string())?;
        }
        Action::Reorder {
            mod_ids,
            expected_revision,
        } => {
            let expected = revision(&expected_revision)?;
            let records = store.load().map_err(|e| e.to_string())?;
            if records.revision != expected {
                return Err("The library changed. Retry with its current revision.".into());
            }
            let entries = records
                .collections
                .iter()
                .find(|c| Some(&c.id) == records.active_collection.as_ref())
                .map_or(&[][..], |c| c.entries.as_slice());
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
                        .find(|r| &r.mod_id == id)
                        .cloned()
                        .ok_or_else(|| {
                            "Reordering cannot change collection membership.".to_string()
                        })
                })
                .collect::<Result<Vec<_>>>()?;
            resolve_order(store, &reordered)?;
            store
                .set_mod_membership(&reordered, expected)
                .map_err(|e| e.to_string())?;
        }
        Action::SetEnabled {
            reference,
            enabled,
            expected_revision,
        } => {
            let expected = revision(&expected_revision)?;
            let records = store.load().map_err(|e| e.to_string())?;
            if records.revision != expected {
                return Err("The library changed. Retry with its current revision.".into());
            }
            let entry = records
                .library
                .iter()
                .find(|e| e.reference == reference)
                .ok_or("This exact package is not in the library.")?;
            let collection = records
                .collections
                .iter()
                .find(|c| Some(&c.id) == records.active_collection.as_ref());
            let mut entries = collection.map_or(vec![], |c| c.entries.clone());
            if enabled {
                let catalog = catalog(store)?;
                let security = store.catalog_security().map_err(|e| e.to_string())?;
                let locals = store.local_sources().map_err(|e| e.to_string())?;
                let mut needed = Vec::new();
                dependency_entries(
                    catalog.as_ref(),
                    &locals,
                    &records,
                    &entry.reference,
                    &mut needed,
                    &mut BTreeSet::new(),
                )?;
                for reference in needed {
                    let prepared = store
                        .prepared_artifact(&reference.hash)
                        .map_err(|e| e.to_string())?
                        .ok_or("Prepared package inventory is missing.")?;
                    if let Some(security) = &security {
                        security
                            .require_allowed(&prepared.hash, &prepared.files)
                            .map_err(|e| e.to_string())?;
                    }
                    packages::layout(
                        &prepared.files,
                        &metadata(catalog.as_ref(), &locals, &reference)?.layout,
                    )?;
                    if entries.contains(&reference) {
                        continue;
                    }
                    entries.retain(|r| r.mod_id != reference.mod_id);
                    entries.push(reference);
                }
                crate::ordering::resolve_with_locals(catalog.as_ref(), &locals, &entries)?;
            } else {
                entries.retain(|r| r != &reference);
            }
            store
                .set_mod_membership(&entries, expected)
                .map_err(|e| e.to_string())?;
        }
        Action::Uninstall {
            reference,
            expected_revision,
            confirm_references,
        } => {
            store
                .uninstall_mod(
                    &reference,
                    revision(&expected_revision)?,
                    confirm_references,
                )
                .map_err(|e| e.to_string())?;
            cleanup(store)?;
        }
    }
    view(store)
}

pub fn cleanup(store: &mut Storage) -> Result<()> {
    for (hash, _) in store.pending_removals().map_err(|e| e.to_string())? {
        let result = packages::remove_artifact(store, &hash);
        store
            .finish_removal(&hash, result.err().as_deref())
            .map_err(|e| e.to_string())?;
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
    let records = store.load().map_err(|e| e.to_string())?;
    if records.revision != revision(expected)? {
        return Err("The library changed. Retry with its current revision.".into());
    }
    Ok(records)
}

fn catalog(store: &Storage) -> Result<Option<Catalog>> {
    store
        .catalog_cache()
        .map_err(|e| e.to_string())
        .map(|cache| cache.and_then(|c| c.catalog))
}

pub(crate) fn resolve_order(
    store: &Storage,
    entries: &[ModReference],
) -> Result<crate::ordering::Resolution> {
    crate::ordering::resolve_with_locals(
        catalog(store)?.as_ref(),
        &store.local_sources().map_err(|e| e.to_string())?,
        entries,
    )
}

pub(crate) fn metadata(
    catalog: Option<&Catalog>,
    locals: &[crate::local_import::LocalSource],
    reference: &ModReference,
) -> Result<crate::local_import::Manifest> {
    if reference.origin == Origin::LocalImport {
        return locals
            .iter()
            .find(|source| &source.reference == reference)
            .map(|source| source.manifest.clone())
            .ok_or_else(|| {
                format!(
                    "Local metadata for {} is unavailable. Import the exact build again.",
                    reference.mod_id
                )
            });
    }
    let catalog =
        catalog.ok_or("Catalog metadata is unavailable. Existing game files were retained.")?;
    let release = release(catalog, reference)?;
    let owner = catalog
        .mods
        .iter()
        .find(|m| m.id == reference.mod_id)
        .ok_or("Mod metadata is missing.")?;
    let requires = release
        .requires
        .iter()
        .map(|id| {
            let (owner, release) = catalog
                .releases()
                .find(|(_, r)| &r.id == id)
                .ok_or("Required release metadata is missing.")?;
            Ok(ModReference {
                mod_id: owner.id.clone(),
                hash: release.artifact.sha256.clone(),
                origin: Origin::Catalog,
                release_id: Some(release.id.clone()),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(crate::local_import::Manifest {
        schema_version: 1,
        mod_id: owner.id.clone(),
        name: owner.name.clone(),
        author: owner.author.clone(),
        version: release.version.clone(),
        layout: release.artifact.layout.clone(),
        requires,
        load_before: release.load_before.clone(),
        load_after: release.load_after.clone(),
        prefer_before: release.prefer_before.clone(),
        prefer_after: release.prefer_after.clone(),
    })
}

fn release<'a>(
    catalog: &'a Catalog,
    reference: &ModReference,
) -> Result<&'a crate::catalog::Release> {
    let (owner, release) = catalog
        .releases()
        .find(|(_, r)| Some(&r.id) == reference.release_id.as_ref())
        .ok_or("Release metadata is missing. Existing files were retained.")?;
    if reference.origin != Origin::Catalog
        || owner.id != reference.mod_id
        || release.artifact.sha256 != reference.hash
    {
        return Err("Library identity differs from catalog approval.".into());
    }
    Ok(release)
}

fn dependency_entries(
    catalog: Option<&Catalog>,
    locals: &[crate::local_import::LocalSource],
    records: &Records,
    reference: &ModReference,
    result: &mut Vec<ModReference>,
    visiting: &mut BTreeSet<String>,
) -> Result<()> {
    if result.iter().any(|r| r == reference) {
        return Ok(());
    }
    if !visiting.insert(reference.mod_id.clone()) {
        return Err("Conflicting or cyclic mod dependencies.".into());
    }
    let release = metadata(catalog, locals, reference)?;
    for id in &release.requires {
        let entry = records
            .library
            .iter()
            .find(|e| &e.reference == id)
            .ok_or_else(|| {
                format!(
                    "Prepare the exact required build of {} before enabling this mod.",
                    id.mod_id
                )
            })?;
        dependency_entries(catalog, locals, records, &entry.reference, result, visiting)?;
    }
    visiting.remove(&reference.mod_id);
    if result
        .iter()
        .any(|r| r.mod_id == reference.mod_id && r != reference)
    {
        return Err("Dependencies require conflicting versions of one mod.".into());
    }
    result.push(reference.clone());
    Ok(())
}

pub fn requested(store: &Storage) -> Result<Value> {
    let records = store.load().map_err(|e| e.to_string())?;
    if let Some(error) = crate::sharing::active_error(store, &records)? {
        return Err(error);
    }
    let entries = records
        .collections
        .iter()
        .find(|c| Some(&c.id) == records.active_collection.as_ref())
        .map_or(&[][..], |c| c.entries.as_slice());
    let (inventory, omitted) = inventory(&records, entries);
    if entries.is_empty() {
        let mut value = crate::launch::requested(&records)?;
        value["schemaVersion"] = json!(3);
        value["installedMods"] = inventory;
        value["omittedDisabledMods"] = json!(omitted);
        return runtime_contract::read(
            &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
            "activation",
        );
    }
    let catalog = catalog(store)?;
    let security = store.catalog_security().map_err(|e| e.to_string())?;
    let locals = store.local_sources().map_err(|e| e.to_string())?;
    for reference in entries {
        if !records.library.iter().any(|e| &e.reference == reference) {
            return Err(format!(
                "{} is unavailable in the library. Prepare the exact package or disable it.",
                reference.mod_id
            ));
        }
    }
    let ordered =
        crate::ordering::resolve_with_locals(catalog.as_ref(), &locals, entries)?.effective;
    let mut mods = Vec::new();
    let mut total_bytes = 0;
    for reference in &ordered {
        let release = metadata(catalog.as_ref(), &locals, reference)?;
        let prepared = store
            .prepared_artifact(&reference.hash)
            .map_err(|e| e.to_string())?
            .ok_or("Prepared package inventory is missing.")?;
        if let Some(security) = &security {
            security
                .require_allowed(&prepared.hash, &prepared.files)
                .map_err(|e| e.to_string())?;
        }
        packages::layout(&prepared.files, &release.layout)?;
        total_bytes += prepared.files.iter().map(|f| f.size_bytes).sum::<u64>();
        if total_bytes > runtime_contract::MAX_ACTIVATION_BYTES {
            return Err("The selected mods exceed the runtime's combined 256 MiB limit. Disable some mods or choose a smaller collection; the current deployment was retained.".into());
        }
        let (entry_path, entry_type) = match &release.layout {
            Layout::StarframeManagedZip {
                root,
                entry_assembly,
                entry_type,
            } => (
                Some(if root.is_empty() {
                    entry_assembly.clone()
                } else {
                    format!("{root}/{entry_assembly}")
                }),
                Some(entry_type),
            ),
            Layout::StarframeLuaZip {} => (None, None),
        };
        let requires: Vec<_> = release.requires.iter().map(|r| r.mod_id.clone()).collect();
        let root = format!(
            "mods/{:x}",
            Sha256::digest(format!("{}\0{}", reference.mod_id, reference.hash).as_bytes())
        );
        let files = json!(
            prepared
                .files
                .iter()
                .map(|f| json!({"path":f.path,"sha256":f.sha256}))
                .collect::<Vec<_>>()
        );
        let source = match reference.origin {
            Origin::Catalog => json!({"kind":"catalog", "releaseId":reference.release_id}),
            Origin::LocalImport => {
                json!({"kind":"local", "contentId":runtime_contract::content_id(&files)?})
            }
        };
        mods.push(json!({"modId": reference.mod_id, "source": source, "root": root, "entryAssembly": entry_path, "entryType": entry_type, "requires":requires, "files":files }));
    }
    let value = json!({"schemaVersion":3, "runtimeContractVersion":1, "integrationId":"starframe.bepinex", "deploymentRevision":records.revision.to_string(), "installedMods":inventory,"omittedDisabledMods":omitted,"mods":mods});
    runtime_contract::read(
        &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
        "activation",
    )
}

fn inventory(records: &Records, enabled: &[ModReference]) -> (Value, usize) {
    let mut seen = BTreeSet::new();
    let entries = enabled
        .iter()
        .filter_map(|r| records.library.iter().find(|e| &e.reference == r))
        .chain(records.library.iter());
    let entries: Vec<_> = entries
        .filter(|e| seen.insert(&e.reference.mod_id))
        .collect();
    let omitted = entries.len().saturating_sub(runtime_contract::MAX_MODS);
    (
        json!(
            entries
                .into_iter()
                .take(runtime_contract::MAX_MODS)
                .map(|e| json!({"modId":e.reference.mod_id,"name":e.name,"version":e.version}))
                .collect::<Vec<_>>()
        ),
        omitted,
    )
}

pub(crate) fn payload(store: &Storage, activation: &Value) -> Result<Vec<(String, Source)>> {
    if &requested(store)? != activation {
        return Err("The requested collection changed before preparation.".into());
    }
    let records = store.load().map_err(|e| e.to_string())?;
    let mut sources = Vec::new();
    for item in activation["mods"].as_array().ok_or("Invalid activation.")? {
        let reference = records
            .collections
            .iter()
            .find(|c| Some(&c.id) == records.active_collection.as_ref())
            .and_then(|c| {
                c.entries
                    .iter()
                    .find(|r| Some(r.mod_id.as_str()) == item["modId"].as_str())
            })
            .ok_or("An active reference is missing from the collection.")?;
        let prepared = packages::verify_artifact(store, reference)?;
        let base = store
            .artifact_directory(reference)
            .map_err(|e| e.to_string())?;
        for file in prepared.files {
            sources.push((
                format!("Starframe/{}/{}", item["root"].as_str().unwrap(), file.path),
                Source::File {
                    path: base.join(&file.path),
                    hash: file.sha256,
                    size: file.size_bytes,
                },
            ));
        }
    }
    Ok(sources)
}
