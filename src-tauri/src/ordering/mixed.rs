use super::{cycle, registry::selection_edges, topological};
use crate::{local_import::LocalSource, references::Reference, storage::RegistryLibraryEntry};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixedResolution {
    pub effective: Vec<Reference>,
    pub adjustments: Vec<MixedAdjustment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixedAdjustment {
    pub before: Reference,
    pub after: Reference,
    pub message: String,
}

pub fn resolve_mixed(
    installed: &[RegistryLibraryEntry],
    locals: &[LocalSource],
    requested: &[Reference],
) -> Result<MixedResolution, String> {
    if requested.len() > crate::runtime_contract::MAX_MODS {
        return Err("Starframe supports at most 256 active mods.".into());
    }
    let mut identities = BTreeSet::new();
    let mut local_positions = HashMap::new();
    let mut registry_positions = Vec::new();
    let mut registry_references = Vec::new();
    let mut names = Vec::new();
    let mut selected_locals = Vec::new();
    for (index, reference) in requested.iter().enumerate() {
        reference.validate()?;
        if !identities.insert(reference.runtime_id()) {
            return Err("Selected references have duplicate or conflicting runtime identities. Keep only one release per mod; local and registry mods must have distinct runtime keys.".into());
        }
        match reference {
            Reference::Registry(reference) => {
                registry_positions.push(index);
                registry_references.push(reference.clone());
                let entry = installed.iter().find(|entry| &entry.reference == reference)
                    .ok_or("The exact registry release is not installed. Install the pinned release to use this collection.")?;
                names.push(entry.display.name.clone());
            }
            Reference::Local(_) => {
                let local = reference.local_reference().unwrap();
                local_positions.insert(local.mod_id.clone(), index);
                let source = locals.iter().find(|source| source.reference == local)
                    .ok_or("The exact local build is unavailable. Import matching content to use this collection.")?;
                source.validate()?;
                names.push(source.manifest.name.clone());
                selected_locals.push((index, source));
            }
        }
    }
    let (_, registry_edges) = selection_edges(installed, &registry_references)?;
    let mut edges = vec![BTreeSet::new(); requested.len()];
    for (from, targets) in registry_edges.iter().enumerate() {
        for &to in targets {
            edges[registry_positions[from]].insert(registry_positions[to]);
        }
    }
    for &(index, source) in &selected_locals {
        for required in &source.manifest.requires {
            let exact = Reference::try_from(required)?;
            let dependency = requested
                .iter()
                .position(|reference| reference == &exact)
                .ok_or_else(|| {
                    format!(
                        "{} requires the exact local build {} ({}). Enable it or disable {}.",
                        source.manifest.name, required.mod_id, required.hash, source.manifest.name
                    )
                })?;
            edges[dependency].insert(index);
        }
        for (targets, before) in [
            (&source.manifest.load_before, true),
            (&source.manifest.load_after, false),
        ] {
            for target in targets {
                if let Some(&other) = local_positions.get(target) {
                    let (from, to) = if before {
                        (index, other)
                    } else {
                        (other, index)
                    };
                    edges[from].insert(to);
                }
            }
        }
    }
    if let Some(cycle) = cycle(&edges) {
        return Err(format!(
            "Load-order cycle: {}. Disable an involved mod or select compatible releases.",
            cycle
                .iter()
                .map(|&index| names[index].as_str())
                .collect::<Vec<_>>()
                .join(" -> ")
        ));
    }
    let mut adjustments = Vec::new();
    let adjustment = |from: usize, to: usize, message: String| MixedAdjustment {
        before: requested[from].clone(),
        after: requested[to].clone(),
        message,
    };
    for (from, targets) in edges.iter().enumerate() {
        for &to in targets {
            if from > to {
                adjustments.push(adjustment(
                    from,
                    to,
                    format!("{} must load before {}.", names[from], names[to]),
                ));
            }
        }
    }
    let indices = topological(&edges);
    let effective_positions: HashMap<_, _> = indices
        .iter()
        .enumerate()
        .map(|(position, &index)| (index, position))
        .collect();
    for (index, source) in selected_locals {
        for (targets, before) in [
            (&source.manifest.prefer_before, true),
            (&source.manifest.prefer_after, false),
        ] {
            for target in targets {
                if let Some(&other) = local_positions.get(target) {
                    let (from, to) = if before {
                        (index, other)
                    } else {
                        (other, index)
                    };
                    if effective_positions[&from] > effective_positions[&to] {
                        let message = format!(
                            "{} prefers to load before {}. Your priority and required constraints take precedence.",
                            names[from], names[to]
                        );
                        if !adjustments.iter().any(|item| item.message == message) {
                            adjustments.push(adjustment(from, to, message));
                        }
                    }
                }
            }
        }
    }
    Ok(MixedResolution {
        effective: indices
            .into_iter()
            .map(|index| requested[index].clone())
            .collect(),
        adjustments,
    })
}

#[cfg(test)]
mod tests;
