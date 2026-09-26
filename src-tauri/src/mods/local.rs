use crate::{
    deployment::Source,
    local_import::Layout,
    packages, runtime_contract,
    storage::{ModReference, Records, Storage},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
type Result<T> = std::result::Result<T, String>;
pub(crate) fn metadata(
    locals: &[crate::local_import::LocalSource],
    reference: &ModReference,
) -> Result<crate::local_import::Manifest> {
    crate::references::Reference::try_from(reference)?;
    locals
        .iter()
        .find(|source| &source.reference == reference)
        .map(|source| source.manifest.clone())
        .ok_or_else(|| {
            format!(
                "Local metadata for {} is unavailable. Import the exact build again.",
                reference.mod_id
            )
        })
}

pub(crate) fn requested_local(store: &Storage, entries: &[ModReference]) -> Result<Value> {
    for reference in entries {
        crate::references::Reference::try_from(reference)?;
    }
    let records = store.load().map_err(|e| e.to_string())?;
    requested_entries(store, &records, entries)
}

fn requested_entries(
    store: &Storage,
    records: &Records,
    entries: &[ModReference],
) -> Result<Value> {
    let (inventory, omitted) = inventory(records, entries);
    if entries.is_empty() {
        let mut value = json!({"schemaVersion":3,"runtimeContractVersion":1,"integrationId":"starframe.bepinex","deploymentRevision":records.revision.to_string(),"mods":[]});
        value["installedMods"] = inventory;
        value["omittedDisabledMods"] = json!(omitted);
        return runtime_contract::read(
            &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
            "activation",
        );
    }
    let locals = store.local_sources().map_err(|e| e.to_string())?;
    for reference in entries {
        if !records.library.iter().any(|e| &e.reference == reference) {
            return Err(format!(
                "{} is unavailable in the library. Prepare the exact package or disable it.",
                reference.mod_id
            ));
        }
    }
    let ordered = crate::ordering::resolve_locals(&locals, entries)?.effective;
    let mut mods = Vec::new();
    let mut total_bytes = 0;
    for reference in &ordered {
        store
            .require_registry_unblocked_hash(&reference.hash)
            .map_err(|e| e.to_string())?;
        let release = metadata(&locals, reference)?;
        let prepared = store
            .prepared_artifact(&reference.hash)
            .map_err(|e| e.to_string())?
            .ok_or("Prepared package inventory is missing.")?;
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
        let source = json!({"kind":"local", "contentId":runtime_contract::content_id(&files)?});
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

pub(crate) fn payload_entries(
    store: &Storage,
    activation: &Value,
    entries: &[ModReference],
) -> Result<Vec<(String, Source)>> {
    let mut sources = Vec::new();
    for item in activation["mods"].as_array().ok_or("Invalid activation.")? {
        let reference = entries
            .iter()
            .find(|reference| Some(reference.mod_id.as_str()) == item["modId"].as_str())
            .ok_or("An active reference is missing from the selection.")?;
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
