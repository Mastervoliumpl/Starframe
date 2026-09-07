use crate::{
    catalog::{Catalog, Layout},
    deployment::Source,
    packages, runtime_contract,
    storage::{LibraryEntry, ModReference, Origin, Records, Storage},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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
pub enum Action {
    List,
    RetryCleanup,
    SetEnabled {
        mod_id: String,
        hash: String,
        enabled: bool,
        expected_revision: String,
    },
    Uninstall {
        mod_id: String,
        hash: String,
        expected_revision: String,
        confirm_references: bool,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub catalog: Option<Catalog>,
    pub revision: String,
    pub library: Vec<LibraryEntry>,
    pub enabled: Vec<ModReference>,
    pub collections: Vec<crate::storage::Collection>,
    pub cleanup_errors: Vec<String>,
}

pub fn view(store: &Storage) -> Result<View> {
    let records = store.load().map_err(|e| e.to_string())?;
    let enabled = records
        .collections
        .iter()
        .find(|c| Some(&c.id) == records.active_collection.as_ref())
        .map_or(vec![], |c| c.entries.clone());
    Ok(View {
        catalog: store
            .catalog_cache()
            .map_err(|e| e.to_string())?
            .and_then(|c| c.catalog),
        revision: records.revision.to_string(),
        library: records.library,
        enabled,
        collections: records.collections,
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
        Action::SetEnabled {
            mod_id,
            hash,
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
                .find(|e| e.reference.mod_id == mod_id && e.reference.hash == hash)
                .ok_or("This exact package is not in the library.")?;
            let collection = records
                .collections
                .iter()
                .find(|c| Some(&c.id) == records.active_collection.as_ref());
            let mut entries = collection.map_or(vec![], |c| c.entries.clone());
            if enabled {
                let catalog = catalog(store)?;
                let mut needed = Vec::new();
                dependency_entries(
                    &catalog,
                    &records,
                    &entry.reference,
                    &mut needed,
                    &mut BTreeSet::new(),
                )?;
                for reference in needed {
                    entries.retain(|r| r.mod_id != reference.mod_id);
                    entries.push(reference);
                }
            } else {
                entries.retain(|r| !(r.mod_id == mod_id && r.hash == hash));
            }
            store
                .set_mod_membership(&entries, expected)
                .map_err(|e| e.to_string())?;
        }
        Action::Uninstall {
            mod_id,
            hash,
            expected_revision,
            confirm_references,
        } => {
            store
                .uninstall_mod(
                    &mod_id,
                    &hash,
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

fn catalog(store: &Storage) -> Result<Catalog> {
    store
        .catalog_cache()
        .map_err(|e| e.to_string())?
        .and_then(|c| c.catalog)
        .ok_or("Catalog metadata is unavailable. Existing game files were retained.".into())
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
    catalog: &Catalog,
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
    let release = release(catalog, reference)?;
    for id in &release.requires {
        let entry = records
            .library
            .iter()
            .find(|e| e.reference.release_id.as_ref() == Some(id))
            .ok_or_else(|| format!("Prepare required release {id} before enabling this mod."))?;
        dependency_entries(catalog, records, &entry.reference, result, visiting)?;
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
    let entries = records
        .collections
        .iter()
        .find(|c| Some(&c.id) == records.active_collection.as_ref())
        .map_or(&[][..], |c| c.entries.as_slice());
    if entries.is_empty() {
        let mut value = crate::launch::requested(&records)?;
        value["installedMods"] = inventory(&records, entries);
        return runtime_contract::read(
            &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
            "activation",
        );
    }
    let catalog = catalog(store)?;
    let mut ordered = Vec::new();
    for reference in entries {
        if !records.library.iter().any(|e| &e.reference == reference) {
            return Err(format!(
                "{} is unavailable in the library. Prepare the exact package or disable it.",
                reference.mod_id
            ));
        }
        dependency_entries(
            &catalog,
            &records,
            reference,
            &mut ordered,
            &mut BTreeSet::new(),
        )?;
    }
    if ordered.iter().any(|r| !entries.contains(r)) {
        return Err("An enabled mod requires a disabled dependency. Enable the dependency or disable the dependent mod.".into());
    }
    let mut mods = Vec::new();
    for reference in &ordered {
        let release = release(&catalog, reference)?;
        let prepared = store
            .prepared_artifact(&reference.hash)
            .map_err(|e| e.to_string())?
            .ok_or("Prepared package inventory is missing.")?;
        let Layout::StarframeManagedZip {
            root,
            entry_assembly,
            entry_type,
        } = &release.artifact.layout;
        let requires: Vec<_> = release
            .requires
            .iter()
            .map(|id| {
                catalog
                    .releases()
                    .find(|(_, r)| &r.id == id)
                    .map(|(m, _)| m.id.clone())
                    .ok_or("Required release metadata is missing.")
            })
            .collect::<std::result::Result<_, _>>()?;
        let entry_path = if root.is_empty() {
            entry_assembly.clone()
        } else {
            format!("{root}/{entry_assembly}")
        };
        mods.push(json!({"modId": reference.mod_id, "source": {"kind":"catalog", "releaseId": release.id}, "root": format!("mods/{}", reference.hash), "entryAssembly": entry_path, "entryType": entry_type, "requires":requires, "files":prepared.files.iter().map(|f| json!({"path":f.path,"sha256":f.sha256})).collect::<Vec<_>>() }));
    }
    let value = json!({"schemaVersion":2, "runtimeContractVersion":1, "integrationId":"starframe.bepinex", "deploymentRevision":records.revision.to_string(), "installedMods":inventory(&records, entries),"mods":mods});
    runtime_contract::read(
        &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
        "activation",
    )
}

fn inventory(records: &Records, enabled: &[ModReference]) -> Value {
    let mut seen = BTreeSet::new();
    let entries = enabled
        .iter()
        .filter_map(|r| records.library.iter().find(|e| &e.reference == r))
        .chain(records.library.iter());
    json!(
        entries
            .filter(|e| seen.insert(&e.reference.mod_id))
            .map(|e| json!({"modId":e.reference.mod_id,"name":e.name,"version":e.version}))
            .collect::<Vec<_>>()
    )
}

pub(crate) fn payload(store: &Storage, activation: &Value) -> Result<Vec<(String, Source)>> {
    if &requested(store)? != activation {
        return Err("The requested collection changed before preparation.".into());
    }
    let records = store.load().map_err(|e| e.to_string())?;
    let mut sources = Vec::new();
    for item in activation["mods"].as_array().ok_or("Invalid activation.")? {
        let reference = &records
            .library
            .iter()
            .find(|e| e.reference.release_id.as_deref() == item["source"]["releaseId"].as_str())
            .ok_or("An active release is missing from the library.")?
            .reference;
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
