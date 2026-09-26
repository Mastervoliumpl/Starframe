use crate::{
    deployment::Source,
    references::Reference,
    registry::installation::Installation,
    runtime_contract,
    storage::{Origin, Storage},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn requested(store: &Storage, references: &[Reference]) -> Result<Value, String> {
    let installed = store
        .installed_registry_releases()
        .map_err(|error| error.to_string())?;
    let locals = store.local_sources().map_err(|error| error.to_string())?;
    let order = crate::ordering::resolve_mixed(&installed, &locals, references)?.effective;
    let records = store.load().map_err(|error| error.to_string())?;
    let registry_ids: BTreeSet<_> = installed
        .iter()
        .map(|entry| crate::registry::activation::runtime_id(&entry.reference))
        .collect();
    if records.library.iter().any(|entry| {
        entry.reference.origin == Origin::LocalImport
            && registry_ids.contains(&entry.reference.mod_id)
    }) {
        return Err("A local mod and an installed registry mod use the same runtime settings key. Give the local mod a distinct declared ID before combining this library.".into());
    }
    let registry = order
        .iter()
        .filter_map(|reference| match reference {
            Reference::Registry(reference) => Some(reference.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let local = order
        .iter()
        .filter_map(Reference::local_reference)
        .collect::<Vec<_>>();
    let registry_activation = crate::registry::activation::requested(store, &registry)?;
    let local_activation = crate::mods::requested_local(store, &local)?;
    let mut active: BTreeMap<_, _> = registry_activation["mods"]
        .as_array()
        .unwrap()
        .iter()
        .chain(local_activation["mods"].as_array().unwrap())
        .map(|item| (item["modId"].as_str().unwrap().to_owned(), item.clone()))
        .collect();
    let mut mods = Vec::new();
    let mut inventory = Vec::new();
    let mut seen = BTreeSet::new();
    let mut bytes = 0u64;
    for reference in &order {
        let id = reference.runtime_id();
        if let Some(item) = active.remove(&id) {
            let prepared = store
                .prepared_artifact(reference.hash())
                .map_err(|error| error.to_string())?
                .ok_or("Prepared inventory is missing.")?;
            bytes = bytes
                .checked_add(
                    prepared
                        .files
                        .iter()
                        .map(|file| file.size_bytes)
                        .sum::<u64>(),
                )
                .ok_or("Activation size overflow.")?;
            if bytes > runtime_contract::MAX_ACTIVATION_BYTES {
                return Err("The selected Code mods exceed the runtime's combined 256 MiB limit. Disable some mods; the existing deployment was retained.".into());
            }
            let source_inventory = if matches!(reference, Reference::Registry(_)) {
                &registry_activation
            } else {
                &local_activation
            };
            inventory.push(
                source_inventory["installedMods"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|entry| entry["modId"] == id)
                    .unwrap()
                    .clone(),
            );
            seen.insert(id);
            mods.push(item);
        }
    }
    for entry in &records.library {
        if entry.reference.origin == Origin::LocalImport
            && seen.insert(entry.reference.mod_id.clone())
        {
            inventory.push(
                json!({"modId":entry.reference.mod_id,"name":entry.name,"version":entry.version}),
            );
        }
    }
    for entry in &installed {
        if !matches!(entry.installation, Installation::Content { .. }) {
            let id = crate::registry::activation::runtime_id(&entry.reference);
            if seen.insert(id.clone()) {
                inventory.push(
                    json!({"modId":id,"name":entry.display.name,"version":entry.version_label}),
                );
            }
        }
    }
    let omitted = inventory.len().saturating_sub(runtime_contract::MAX_MODS);
    inventory.truncate(runtime_contract::MAX_MODS);
    runtime_contract::read(&serde_json::to_vec(&json!({
        "schemaVersion":4,"runtimeContractVersion":1,"integrationId":"starframe.bepinex",
        "deploymentRevision":records.revision.to_string(),"installedMods":inventory,"omittedDisabledMods":omitted,"mods":mods
    })).map_err(|error| error.to_string())?, "activation")
}

pub(crate) fn payload(
    store: &Storage,
    references: &[Reference],
    activation: &Value,
) -> Result<Vec<(String, Source)>, String> {
    if &requested(store, references)? != activation {
        return Err("The selected setup changed before preparation.".into());
    }
    let registry = references
        .iter()
        .filter_map(|reference| match reference {
            Reference::Registry(reference) => Some(reference.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let local = references
        .iter()
        .filter_map(Reference::local_reference)
        .collect::<Vec<_>>();
    let mut local_activation = activation.clone();
    local_activation["mods"]
        .as_array_mut()
        .unwrap()
        .retain(|item| item["source"]["kind"] == "local");
    let mut sources = crate::mods::payload_entries(store, &local_activation, &local)?;
    sources.extend(crate::deployment::registry_sources(store, &registry)?);
    Ok(sources)
}
