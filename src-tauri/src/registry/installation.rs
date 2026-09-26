use serde::{Deserialize, Serialize};

pub const CONTRACT_REVISION: &str = "1d616d43d90227ee9967091be039e6c3e695091e";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Code,
    Map,
    Ai,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Loader {
    Lua,
    Bepinex5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    SanctuaryMaps,
    SanctuaryAiMods,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged, rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum Installation {
    Lua {
        schema_version: u8,
        kind: Kind,
        loader: Loader,
        entry_path: String,
    },
    Managed {
        schema_version: u8,
        kind: Kind,
        loader: Loader,
        source_root: String,
        entry_assembly: String,
        entry_type: String,
    },
    Content {
        schema_version: u8,
        kind: Kind,
        source_root: String,
        destination: Destination,
        folder: String,
    },
}

fn path(value: &str, empty: bool) -> bool {
    (empty && value.is_empty())
        || (value.len() <= 240 && crate::runtime_contract::relative_path(value).is_ok())
}

impl Installation {
    pub fn valid(&self) -> bool {
        match self {
            Self::Lua {
                schema_version,
                kind,
                loader,
                entry_path,
            } => {
                *schema_version == 1
                    && *kind == Kind::Code
                    && *loader == Loader::Lua
                    && path(entry_path, false)
                    && crate::packages::lua_path(entry_path)
            }
            Self::Managed {
                schema_version,
                kind,
                loader,
                source_root,
                entry_assembly,
                entry_type,
            } => {
                *schema_version == 1
                    && *kind == Kind::Code
                    && *loader == Loader::Bepinex5
                    && path(source_root, true)
                    && path(entry_assembly, false)
                    && entry_assembly.to_ascii_lowercase().ends_with(".dll")
                    && (3..=240).contains(&entry_type.len())
                    && entry_type.split('.').count() <= 16
                    && entry_type.split('.').all(|part| {
                        !part.is_empty()
                            && part.bytes().enumerate().all(|(index, byte)| {
                                byte.is_ascii_alphabetic()
                                    || byte == b'_'
                                    || (index > 0 && byte.is_ascii_digit())
                            })
                    })
            }
            Self::Content {
                schema_version,
                kind,
                source_root,
                destination,
                folder,
            } => {
                *schema_version == 1
                    && path(source_root, false)
                    && matches!(
                        (kind, destination),
                        (Kind::Map, Destination::SanctuaryMaps)
                            | (Kind::Ai, Destination::SanctuaryAiMods)
                    )
                    && (1..=80).contains(&folder.len())
                    && path(folder, false)
                    && folder.bytes().enumerate().all(|(index, byte)| {
                        byte.is_ascii_alphanumeric() || (index > 0 && b" _.-".contains(&byte))
                    })
            }
        }
    }

    pub fn validate_files(&self, files: &[crate::packages::PreparedFile]) -> Result<(), String> {
        if !self.valid() {
            return Err("The signed installation declaration is invalid or unsupported. Ask the author for a corrected release.".into());
        }
        if files.is_empty() || files.len() > 4096 {
            return Err("Registry packages must contain 1–4,096 files.".into());
        }
        let mut paths = std::collections::BTreeSet::new();
        let mut total = 0u64;
        for file in files {
            let normalized = crate::runtime_contract::relative_path(&file.path)?;
            if !paths.insert(normalized) {
                return Err("Registry package paths collide on Windows.".into());
            }
            total = total
                .checked_add(file.size_bytes)
                .ok_or("Registry package size overflow.")?;
            if file.size_bytes > 512 * 1024 * 1024 || total > 2 * 1024 * 1024 * 1024 {
                return Err(
                    "Registry content exceeds the 512 MiB file or 2 GiB expanded limit.".into(),
                );
            }
            let path = file.path.to_ascii_lowercase();
            if (path.ends_with(".sanmap")
                && !matches!(
                    self,
                    Self::Content {
                        kind: Kind::Map,
                        ..
                    }
                ))
                || (path.starts_with("lj/lua/ai/")
                    && !matches!(self, Self::Content { kind: Kind::Ai, .. }))
            {
                return Err("Code, Map and AI content require separate declared releases.".into());
            }
        }
        for path in &paths {
            let mut parent = path.rsplit_once('/');
            while let Some((prefix, _)) = parent {
                if paths.contains(prefix) {
                    return Err("Registry package files overlap a directory.".into());
                }
                parent = prefix.rsplit_once('/');
            }
        }
        let nonempty = |path: &str| {
            files
                .iter()
                .any(|file| file.path == path && file.size_bytes > 0)
        };
        match self {
            Self::Lua { entry_path, .. } => {
                crate::packages::supported_files(files)?;
                if !files
                    .iter()
                    .all(|file| crate::packages::lua_path(&file.path))
                    || !nonempty(entry_path)
                {
                    return Err("The Lua archive must contain its declared nonempty entry and only Lua files under LJ/lua, outside AI.".into());
                }
            }
            Self::Managed {
                source_root,
                entry_assembly,
                ..
            } => {
                crate::packages::supported_files(files)?;
                let prefix = if source_root.is_empty() {
                    String::new()
                } else {
                    format!("{source_root}/")
                };
                if !files.iter().all(|file| file.path.starts_with(&prefix))
                    || files
                        .iter()
                        .any(|file| file.path.to_ascii_lowercase().starts_with("lj/lua/"))
                    || !nonempty(&format!("{prefix}{entry_assembly}"))
                {
                    return Err("The BepInEx archive must contain its declared nonempty DLL and keep all files inside the declared source folder, without Lua overlays.".into());
                }
            }
            Self::Content {
                source_root,
                kind,
                folder,
                ..
            } => {
                let prefix = format!("{source_root}/");
                if !files.iter().all(|file| file.path.starts_with(&prefix))
                    || files
                        .iter()
                        .any(|file| file.path.to_ascii_lowercase().ends_with(".dll"))
                {
                    return Err("Map and AI archives must keep all files inside the declared source folder and cannot include DLLs.".into());
                }
                if (*kind == Kind::Map && !nonempty(&format!("{prefix}{folder}.sanmap")))
                    || (*kind == Kind::Ai
                        && !files.iter().any(|file| {
                            file.path.to_ascii_lowercase().ends_with(".lua") && file.size_bytes > 0
                        }))
                {
                    return Err("The declared Map needs its matching nonempty .sanmap file; an AI archive needs a nonempty Lua file.".into());
                }
            }
        }
        Ok(())
    }

    pub fn content_path(&self, file: &str) -> Result<Option<String>, String> {
        if !self.valid() || crate::runtime_contract::relative_path(file).is_err() {
            return Err("Invalid declared installation path.".into());
        }
        let Self::Content {
            source_root,
            destination,
            folder,
            ..
        } = self
        else {
            return Ok(None);
        };
        let relative = file
            .strip_prefix(&format!("{source_root}/"))
            .ok_or("The file is outside its declared source folder.")?;
        let base = match destination {
            Destination::SanctuaryMaps => "engine/Sanctuary_Data/Maps",
            Destination::SanctuaryAiMods => "engine/LJ/lua/AI/mods",
        };
        let path = format!("{base}/{folder}/{relative}");
        crate::runtime_contract::relative_path(&path)?;
        Ok(Some(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn website_installation_shapes_are_closed_and_versioned() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-installation-v1.json"
        ))
        .unwrap();
        assert_eq!(fixture["websiteContractRevision"], CONTRACT_REVISION);
        for example in fixture["cases"]["valid"].as_array().unwrap() {
            let plan = &example["plan"];
            let parsed: Installation = serde_json::from_value(plan.clone()).unwrap();
            assert!(parsed.valid(), "{}", example["name"]);
            assert_eq!(serde_json::to_value(parsed).unwrap(), *plan);
            for (field, value) in [("schemaVersion", json!(2)), ("extra", json!(true))] {
                let mut changed = plan.clone();
                changed[field] = value;
                assert!(
                    serde_json::from_value::<Installation>(changed)
                        .ok()
                        .is_none_or(|plan| !plan.valid())
                );
            }
        }
        for plan in [
            json!({"schemaVersion":1,"kind":"code","loader":"lua","entryPath":"../main.lua"}),
            json!({"schemaVersion":1,"kind":"map","sourceRoot":"Maps/Test","destination":"sanctuary_ai_mods","folder":"Test"}),
            json!({"schemaVersion":1,"kind":"ai","sourceRoot":"AI/Test","destination":"sanctuary_ai_mods","folder":"a/b"}),
            json!({"schemaVersion":1,"kind":"ai","sourceRoot":"AI/Test","destination":"sanctuary_ai_mods","folder":"CON"}),
        ] {
            assert!(
                serde_json::from_value::<Installation>(plan)
                    .ok()
                    .is_none_or(|plan| !plan.valid())
            );
        }
    }

    #[test]
    fn website_inventory_cases_validate_without_guessing_paths() {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-installation-v1.json"
        ))
        .unwrap();
        let examples = fixtures["cases"]["valid"].as_array().unwrap();
        let files = |example: &Value| {
            example["inventory"]
                .as_array()
                .unwrap()
                .iter()
                .map(|file| crate::packages::PreparedFile {
                    path: file["path"].as_str().unwrap().into(),
                    size_bytes: file["bytes"].as_u64().unwrap(),
                    sha256: file["sha256"].as_str().unwrap().into(),
                })
                .collect::<Vec<_>>()
        };
        for example in examples {
            let plan: Installation = serde_json::from_value(example["plan"].clone()).unwrap();
            plan.validate_files(&files(example)).unwrap();
            if example["modType"] == "map" {
                assert_eq!(
                    plan.content_path("Maps/Example/Textures/height.png")
                        .unwrap()
                        .unwrap(),
                    "engine/Sanctuary_Data/Maps/Example/Textures/height.png"
                );
            } else if example["modType"] == "ai" {
                assert_eq!(
                    plan.content_path("AI/Example/formers/rush.lua")
                        .unwrap()
                        .unwrap(),
                    "engine/LJ/lua/AI/mods/Example/formers/rush.lua"
                );
            }
        }
        for invalid in fixtures["cases"]["invalid"].as_array().unwrap() {
            let mut example = examples
                .iter()
                .find(|example| example["name"] == invalid["from"])
                .unwrap()
                .clone();
            if let Some(change) = invalid["change"].as_object() {
                for (key, value) in change {
                    example["plan"][key] = value.clone();
                }
            }
            if let Some(path) = invalid["removePath"].as_str() {
                example["inventory"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|file| file["path"] != path);
            }
            if invalid.get("addFile").is_some() {
                example["inventory"]
                    .as_array_mut()
                    .unwrap()
                    .push(invalid["addFile"].clone());
            }
            if invalid.get("modType").is_some() {
                continue;
            }
            assert!(
                serde_json::from_value::<Installation>(example["plan"].clone())
                    .ok()
                    .is_none_or(|plan| plan.validate_files(&files(&example)).is_err()),
                "{}",
                invalid["name"]
            );
        }
        let plan: Installation = serde_json::from_value(examples[2]["plan"].clone()).unwrap();
        assert!(plan.content_path("Other/Example.sanmap").is_err());
        assert!(plan.content_path("Maps/Example/../Other.sanmap").is_err());
        let mut oversized = files(&examples[2]);
        oversized[0].size_bytes = 512 * 1024 * 1024 + 1;
        assert!(plan.validate_files(&oversized).is_err());
    }
}
