use super::{ExactReference, installation::Installation};
use crate::{runtime_contract, storage::Storage};
use serde_json::{Value, json};

pub(crate) fn runtime_id(reference: &ExactReference) -> String {
    format!("registry.{}", u64::from(reference.mod_id))
}

pub(crate) fn runtime_root(reference: &ExactReference) -> String {
    format!(
        "mods/registry-{}/{}",
        u64::from(reference.mod_id),
        reference.release_id.0
    )
}

pub fn requested(store: &Storage, references: &[ExactReference]) -> Result<Value, String> {
    let installed = store
        .installed_registry_releases()
        .map_err(|error| error.to_string())?;
    let ordered = crate::ordering::resolve_registry(&installed, references)?.effective;
    let mut mods = Vec::new();
    let mut inventory = Vec::new();
    let mut total = 0u64;
    for reference in &ordered {
        store
            .require_registry_unblocked_hash(reference.sha256.as_str())
            .map_err(|error| error.to_string())?;
        let entry = installed
            .iter()
            .find(|entry| &entry.reference == reference)
            .ok_or("The exact registry release is not installed.")?;
        let prepared = store
            .prepared_artifact(reference.sha256.as_str())
            .map_err(|error| error.to_string())?
            .ok_or("The exact registry file inventory is missing. Install it again.")?;
        entry.installation.validate_files(&prepared.files)?;
        let (assembly, entry_type) = match &entry.installation {
            Installation::Lua { .. } => (None, None),
            Installation::Managed {
                source_root,
                entry_assembly,
                entry_type,
                ..
            } => (
                Some(if source_root.is_empty() {
                    entry_assembly.clone()
                } else {
                    format!("{source_root}/{entry_assembly}")
                }),
                Some(entry_type),
            ),
            Installation::Content { .. } => continue,
        };
        total = total
            .checked_add(
                prepared
                    .files
                    .iter()
                    .map(|file| file.size_bytes)
                    .sum::<u64>(),
            )
            .ok_or("Activation size overflow.")?;
        if total > runtime_contract::MAX_ACTIVATION_BYTES {
            return Err("The selected Code mods exceed the runtime's combined 256 MiB limit. Disable some mods; the existing deployment was retained.".into());
        }
        let requires: Vec<_> = entry
            .dependencies
            .iter()
            .filter_map(|dependency| {
                installed
                    .iter()
                    .find(|candidate| {
                        candidate.reference.mod_id == dependency.mod_id()
                            && ordered.contains(&candidate.reference)
                    })
                    .filter(|candidate| {
                        !matches!(candidate.installation, Installation::Content { .. })
                    })
                    .map(|candidate| runtime_id(&candidate.reference))
            })
            .collect();
        let id = runtime_id(reference);
        inventory.push(json!({"modId":id,"name":entry.display.name,"version":entry.version_label}));
        mods.push(json!({
            "modId":id,
            "source":{"kind":"registry","modId":reference.mod_id,"releaseId":reference.release_id,"sha256":reference.sha256},
            "root":runtime_root(reference),"entryAssembly":assembly,"entryType":entry_type,
            "requires":requires,
            "files":prepared.files.iter().map(|file| json!({"path":file.path,"sha256":file.sha256})).collect::<Vec<_>>()
        }));
    }
    let revision = store.load().map_err(|error| error.to_string())?.revision;
    runtime_contract::read(&serde_json::to_vec(&json!({
        "schemaVersion":4,"runtimeContractVersion":1,"integrationId":"starframe.bepinex",
        "deploymentRevision":revision.to_string(),"installedMods":inventory,"omittedDisabledMods":0,"mods":mods
    })).map_err(|error| error.to_string())?, "activation")
}

pub(crate) fn payload(
    store: &Storage,
    references: &[ExactReference],
    activation: &Value,
) -> Result<Vec<(String, crate::deployment::Source)>, String> {
    if &requested(store, references)? != activation {
        return Err("The selected registry setup changed before preparation.".into());
    }
    super::super::deployment::registry_sources(store, references)
}
