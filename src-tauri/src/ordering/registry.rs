use super::{cycle, topological};
use crate::{
    registry::{ExactReference, ModId},
    storage::RegistryLibraryEntry,
};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryResolution {
    pub effective: Vec<ExactReference>,
    pub adjustments: Vec<RegistryAdjustment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryAdjustment {
    pub before: ModId,
    pub after: ModId,
    pub message: String,
}

pub fn resolve_registry(
    installed: &[RegistryLibraryEntry],
    requested: &[ExactReference],
) -> Result<RegistryResolution, String> {
    let (selected, edges) = selection_edges(installed, requested)?;
    let mut adjustments = Vec::new();
    for (from, targets) in edges.iter().enumerate() {
        for &to in targets {
            if from > to {
                adjustments.push(RegistryAdjustment {
                    before: requested[from].mod_id,
                    after: requested[to].mod_id,
                    message: format!(
                        "{} must load before {}.",
                        selected[from].display.name, selected[to].display.name
                    ),
                });
            }
        }
    }
    let effective = topological(&edges)
        .into_iter()
        .map(|index| requested[index].clone())
        .collect();
    Ok(RegistryResolution {
        effective,
        adjustments,
    })
}

pub(super) fn selection_edges<'a>(
    installed: &'a [RegistryLibraryEntry],
    requested: &[ExactReference],
) -> Result<(Vec<&'a RegistryLibraryEntry>, Vec<BTreeSet<usize>>), String> {
    if requested.len() > crate::runtime_contract::MAX_MODS {
        return Err("Starframe supports at most 256 active mods.".into());
    }
    let mut positions = HashMap::new();
    let mut selected = Vec::new();
    for (index, reference) in requested.iter().enumerate() {
        if positions.insert(reference.mod_id, index).is_some() {
            return Err(format!(
                "Only one release of mod {} can be enabled.",
                u64::from(reference.mod_id)
            ));
        }
        let entry = installed.iter().find(|entry| &entry.reference == reference)
            .ok_or_else(|| format!("Release {} for mod {} is not installed. Install the pinned release to use this collection.", reference.release_id.0, u64::from(reference.mod_id)))?;
        if !crate::registry::valid_dependencies(reference.mod_id, &entry.dependencies) {
            return Err(
                "Installed registry dependency metadata is invalid. Files were retained.".into(),
            );
        }
        selected.push(entry);
    }
    let mut edges = vec![BTreeSet::new(); requested.len()];
    for (index, entry) in selected.iter().enumerate() {
        for required in &entry.dependencies {
            let dependency = positions.get(&required.mod_id()).copied().ok_or_else(|| {
                format!(
                    "{} requires mod {}. Enable a matching installed release or disable {}.",
                    entry.display.name,
                    u64::from(required.mod_id()),
                    entry.display.name
                )
            })?;
            let candidate = selected[dependency];
            if !required.matches(
                candidate.reference.mod_id,
                candidate.reference.release_id,
                &candidate.version_label,
            ) {
                return Err(format!(
                    "{} requires a different release of {}. Select a matching exact release or disable {}.",
                    entry.display.name, candidate.display.name, entry.display.name
                ));
            }
            edges[dependency].insert(index);
        }
    }
    if let Some(cycle) = cycle(&edges) {
        let names: Vec<_> = cycle
            .iter()
            .map(|&index| selected[index].display.name.as_str())
            .collect();
        return Err(format!(
            "Load-order cycle: {}. Disable an involved mod or select compatible releases.",
            names.join(" -> ")
        ));
    }
    Ok((selected, edges))
}

#[cfg(test)]
mod tests;
