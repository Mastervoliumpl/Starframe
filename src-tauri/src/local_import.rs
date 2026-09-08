use crate::{
    catalog::Layout,
    storage::{LibraryEntry, ModReference, Origin},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MANIFEST: &str = "starframe.local.json";
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "LocalManifest"))]
pub struct Manifest {
    pub schema_version: u32,
    pub mod_id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub layout: Layout,
    #[serde(default)]
    pub requires: Vec<ModReference>,
    #[serde(default)]
    pub load_before: Vec<String>,
    #[serde(default)]
    pub load_after: Vec<String>,
    #[serde(default)]
    pub prefer_before: Vec<String>,
    #[serde(default)]
    pub prefer_after: Vec<String>,
}

impl Manifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Unsupported local manifest schema. Use schemaVersion 1.".into());
        }
        crate::catalog::id(&self.mod_id)?;
        for (value, limit) in [(&self.name, 200), (&self.author, 200), (&self.version, 128)] {
            if value.trim().is_empty() || value.len() > limit || value.chars().any(char::is_control)
            {
                return Err(
                    "Local mod name, author and version must contain bounded, readable text."
                        .into(),
                );
            }
        }
        crate::catalog::validate_layout(&self.layout)?;
        let mut ids = BTreeSet::new();
        if self.requires.len() > 64 {
            return Err("A local mod supports at most 64 dependencies.".into());
        }
        for reference in &self.requires {
            crate::storage::validate_reference(reference).map_err(|e| e.to_string())?;
            if reference.mod_id == self.mod_id || !ids.insert(&reference.mod_id) {
                return Err("Dependencies must refer to distinct other mods.".into());
            }
        }
        for targets in [
            &self.load_before,
            &self.load_after,
            &self.prefer_before,
            &self.prefer_after,
        ] {
            let mut seen = BTreeSet::new();
            if targets.len() > 64 {
                return Err("Too many local ordering constraints.".into());
            }
            for id in targets {
                crate::catalog::id(id)?;
                if id == &self.mod_id || !seen.insert(id) {
                    return Err("Ordering constraints must refer to distinct other mods.".into());
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LocalSource {
    pub reference: ModReference,
    pub path: String,
    pub manifest: Manifest,
}

impl LocalSource {
    pub fn validate(&self) -> Result<(), String> {
        self.manifest.validate()?;
        crate::storage::validate_reference(&self.reference).map_err(|e| e.to_string())?;
        if self.reference.origin != Origin::LocalImport
            || self.reference.mod_id != self.manifest.mod_id
            || !std::path::Path::new(&self.path).is_absolute()
            || self.path.len() > 32768
            || self.path.chars().any(char::is_control)
        {
            return Err("Invalid saved local source.".into());
        }
        Ok(())
    }

    pub fn entry(&self) -> LibraryEntry {
        LibraryEntry {
            reference: self.reference.clone(),
            name: self.manifest.name.clone(),
            author: self.manifest.author.clone(),
            version: self.manifest.version.clone(),
        }
    }
}
