use super::{Result, Source};
use crate::{packages, registry::ExactReference, storage::Storage};
use std::collections::BTreeSet;

pub(crate) fn payload(
    store: &Storage,
    references: &[ExactReference],
) -> Result<Vec<(String, Source)>> {
    let installed = store
        .installed_registry_releases()
        .map_err(|error| error.to_string())?;
    let mut mods = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut sources = Vec::new();
    for reference in references {
        if !mods.insert(u64::from(reference.mod_id)) {
            return Err("A mod has more than one selected registry release.".into());
        }
        let entry = installed
            .iter()
            .find(|entry| &entry.reference == reference)
            .ok_or("The exact registry release is not installed.")?;
        let prepared = packages::verify_registry_artifact(store, entry)?;
        let root = store
            .package_root()
            .join("artifacts")
            .join(reference.sha256.as_str());
        for file in prepared.files {
            let target = match entry.installation.content_path(&file.path)? {
                // The shared deployment owner is rooted at <game>/engine.
                Some(target) => target
                    .strip_prefix("engine/")
                    .ok_or("The declared destination is outside the game engine.")?
                    .to_owned(),
                None => format!(
                    "Starframe/{}/{}",
                    crate::registry::activation::runtime_root(reference),
                    file.path
                ),
            };
            if !paths.insert(target.to_ascii_lowercase()) {
                return Err("Selected registry content has conflicting game paths.".into());
            }
            sources.push((
                target,
                Source::File {
                    path: root.join(file.path),
                    hash: file.sha256,
                    size: file.size_bytes,
                },
            ));
        }
    }
    Ok(sources)
}

#[cfg(test)]
mod tests;
