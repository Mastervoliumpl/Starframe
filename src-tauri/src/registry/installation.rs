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
}
