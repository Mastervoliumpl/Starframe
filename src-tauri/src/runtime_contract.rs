use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;

pub const MAX_DOCUMENT_BYTES: usize = 1_048_576;
pub const MAX_MODS: usize = 256;
pub const MAX_FILES_PER_MOD: usize = 1024;
type Result<T> = std::result::Result<T, String>;

pub fn read(bytes: &[u8], kind: &str) -> Result<Value> {
    require(bytes.len() <= MAX_DOCUMENT_BYTES, "document size")?;
    let Strict(value) = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    depth(&value, 0)?;
    require(
        if kind == "activation" {
            matches!(value["schemaVersion"].as_u64(), Some(2 | 3))
        } else {
            value["schemaVersion"].as_u64() == Some(1)
        },
        "schema version",
    )?;
    require(
        value["runtimeContractVersion"].as_u64() == Some(1),
        "runtime contract version",
    )?;
    require(
        text(&value["integrationId"], 64, false)? == "starframe.bepinex",
        "integration ID",
    )?;
    match kind {
        "activation" => activation(&value)?,
        "capabilities" => capabilities(&value)?,
        "report" => report(&value)?,
        _ => return Err("Unknown document kind".into()),
    }
    Ok(value)
}

fn activation(root: &Value) -> Result<()> {
    let mut expected = vec![
        "schemaVersion",
        "runtimeContractVersion",
        "integrationId",
        "deploymentRevision",
        "installedMods",
        "mods",
    ];
    if root["schemaVersion"] == 3 {
        expected.push("omittedDisabledMods");
        require(
            root["omittedDisabledMods"]
                .as_u64()
                .is_some_and(|n| n <= i32::MAX as u64),
            "omitted inventory count",
        )?;
    }
    fields(root, &expected)?;
    decimal(&root["deploymentRevision"])?;
    let mut installed = HashSet::new();
    for item in array(&root["installedMods"], MAX_MODS)? {
        fields(item, &["modId", "name", "version"])?;
        require(
            installed.insert(id(&item["modId"])?),
            "duplicate inventory ID",
        )?;
        text(&item["name"], 256, false)?;
        text(&item["version"], 128, false)?;
    }
    require(
        root["omittedDisabledMods"].as_u64().unwrap_or(0) == 0 || installed.len() == MAX_MODS,
        "incomplete bounded inventory",
    )?;
    let mut active = HashSet::new();
    let mut roots: Vec<String> = Vec::new();
    let mut total_files = 0;
    for item in array(&root["mods"], MAX_MODS)? {
        fields(
            item,
            &[
                "modId",
                "source",
                "root",
                "entryAssembly",
                "entryType",
                "requires",
                "files",
            ],
        )?;
        let mod_id = id(&item["modId"])?;
        require(
            installed.contains(mod_id) && !active.contains(mod_id),
            "activation inventory mismatch",
        )?;
        let mut dependencies = HashSet::new();
        for dependency in array(&item["requires"], 256)? {
            let required = text(dependency, 128, false)?;
            require(
                active.contains(required) && dependencies.insert(required),
                "dependency order",
            )?;
        }
        active.insert(mod_id);
        let directory = path(&item["root"])?;
        require(
            !roots.iter().any(|other| overlaps(&directory, other)),
            "overlapping roots",
        )?;
        roots.push(directory);
        let content = item["entryAssembly"].is_null();
        let assembly = if content {
            require(item["entryType"].is_null(), "content entry type")?;
            None
        } else {
            let assembly = path(&item["entryAssembly"])?;
            require(assembly.ends_with(".dll"), "entry assembly")?;
            let entry = text(&item["entryType"], 256, false)?;
            require(
                entry.split('.').all(|part| {
                    !part.is_empty()
                        && part.bytes().enumerate().all(|(i, c)| {
                            c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                        })
                }),
                "entry type",
            )?;
            Some(assembly)
        };
        let mut paths: Vec<String> = Vec::new();
        for file in array(&item["files"], MAX_FILES_PER_MOD)? {
            fields(file, &["path", "sha256"])?;
            let file_path = path(&file["path"])?;
            require(
                !paths.iter().any(|other| overlaps(&file_path, other)),
                "conflicting files",
            )?;
            paths.push(file_path);
            let hash = text(&file["sha256"], 64, false)?;
            require(
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                "file hash",
            )?;
            total_files += 1;
            require(total_files <= 8192, "total files")?;
        }
        require(
            assembly
                .as_ref()
                .map_or(!paths.is_empty(), |assembly| paths.contains(assembly)),
            "entry assembly missing from files",
        )?;
        let source = &item["source"];
        match text(&source["kind"], 16, false)? {
            "catalog" => {
                fields(source, &["kind", "releaseId"])?;
                id(&source["releaseId"])?;
            }
            "local" => {
                fields(source, &["kind", "contentId"])?;
                require(
                    text(&source["contentId"], 71, false)? == content_id(&item["files"])?,
                    "local content ID",
                )?;
            }
            _ => return Err("Unsupported source kind".into()),
        }
    }
    Ok(())
}

pub fn content_id(files: &Value) -> Result<String> {
    let mut lines = Vec::new();
    for file in array(files, 1024)? {
        lines.push(format!(
            "{}\0{}\n",
            path(&file["path"])?,
            text(&file["sha256"], 64, false)?
        ));
    }
    lines.sort();
    let canonical = format!("starframe-inventory-v1\n{}", lines.concat());
    Ok(format!("sha256:{:x}", Sha256::digest(canonical.as_bytes())))
}

fn capabilities(root: &Value) -> Result<()> {
    fields(
        root,
        &[
            "schemaVersion",
            "runtimeContractVersion",
            "integrationId",
            "capabilities",
        ],
    )?;
    let caps = &root["capabilities"];
    fields(
        caps,
        &[
            "managedLifecycle",
            "manualPriority",
            "contentOverlays",
            "settingsUi",
            "activationReport",
        ],
    )?;
    for capability in caps.as_object().ok_or("expected object")?.values() {
        fields(capability, &["supported", "reason"])?;
        let supported = capability["supported"]
            .as_bool()
            .ok_or("capability support")?;
        text(&capability["reason"], 512, supported)?;
    }
    Ok(())
}

fn report(root: &Value) -> Result<()> {
    fields(
        root,
        &[
            "schemaVersion",
            "runtimeContractVersion",
            "integrationId",
            "deploymentRevision",
            "gameSessionId",
            "processId",
            "processStartFileTime",
            "mods",
        ],
    )?;
    decimal(&root["deploymentRevision"])?;
    decimal(&root["processStartFileTime"])?;
    let session = text(&root["gameSessionId"], 36, false)?;
    let guid = uuid::Uuid::parse_str(session).map_err(|e| e.to_string())?;
    require(!guid.is_nil() && guid.to_string() == session, "session ID")?;
    let pid = root["processId"].as_u64().ok_or("process ID")?;
    require(pid > 0 && pid <= u32::MAX as u64, "process ID")?;
    let mut ids = HashSet::new();
    for item in array(&root["mods"], MAX_MODS)? {
        fields(item, &["modId", "outcome", "errorCode", "message"])?;
        require(ids.insert(id(&item["modId"])?), "duplicate report ID")?;
        let outcome = text(&item["outcome"], 32, false)?;
        let loaded = outcome == "loaded";
        require(
            matches!(outcome, "loaded" | "failed" | "skipped_dependency"),
            "outcome",
        )?;
        let code = text(&item["errorCode"], 64, loaded)?;
        let message = text(&item["message"], 2048, loaded)?;
        require(
            if loaded {
                code.is_empty() && message.is_empty()
            } else {
                code.bytes().enumerate().all(|(i, c)| {
                    c.is_ascii_lowercase() || (i > 0 && (c.is_ascii_digit() || c == b'_'))
                })
            },
            "failure details",
        )?;
        require(
            outcome != "skipped_dependency" || code == "dependency_failed",
            "dependency error code",
        )?;
    }
    Ok(())
}

fn require(condition: bool, reason: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("Invalid runtime contract: {reason}"))
    }
}
fn fields(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or("expected object")?;
    require(
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key)),
        "missing or unknown field",
    )
}
fn array(value: &Value, max: usize) -> Result<&Vec<Value>> {
    let array = value.as_array().ok_or("expected array")?;
    require(array.len() <= max, "array limit")?;
    Ok(array)
}
fn text(value: &Value, max: usize, empty: bool) -> Result<&str> {
    let value = value.as_str().ok_or("expected string")?;
    require(
        (empty || !value.is_empty()) && value.len() <= max && !value.chars().any(char::is_control),
        "string limit or control character",
    )?;
    Ok(value)
}
fn id(value: &Value) -> Result<&str> {
    let id = text(value, 128, false)?;
    require(
        id.bytes().enumerate().all(|(i, c)| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || (i > 0 && b"._-".contains(&c))
        }),
        "identifier",
    )?;
    Ok(id)
}
fn decimal(value: &Value) -> Result<()> {
    let value = text(value, 20, false)?;
    require(
        value.bytes().all(|c| c.is_ascii_digit()) && (value == "0" || !value.starts_with('0')),
        "decimal string",
    )
}
fn path(value: &Value) -> Result<String> {
    relative_path(text(value, 240, false)?)
}

pub(crate) fn relative_path(value: &str) -> Result<String> {
    require(
        !value.is_empty() && value.len() <= 240,
        "relative path length",
    )?;
    for part in value.split('/') {
        require(
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_ .-".contains(&c))
                && !matches!(part, "." | "..")
                && !part.ends_with(['.', ' ']),
            "relative path",
        )?;
        let stem = part
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end_matches(' ')
            .to_ascii_uppercase();
        require(
            !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && (b'1'..=b'9').contains(&stem.as_bytes()[3])),
            "Windows device path",
        )?;
    }
    Ok(value.to_ascii_lowercase())
}
fn overlaps(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}
fn depth(value: &Value, level: usize) -> Result<()> {
    match value {
        Value::Array(array) => {
            require(level < 32, "JSON depth")?;
            for item in array {
                depth(item, level + 1)?;
            }
        }
        Value::Object(object) => {
            require(level < 32, "JSON depth")?;
            for item in object.values() {
                depth(item, level + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

// serde_json::Value otherwise keeps the last duplicate key, hiding malformed contracts.
struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = Strict;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JSON without duplicate properties")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut input: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut output = Map::new();
                while let Some((key, Strict(value))) = input.next_entry::<String, Strict>()? {
                    if output.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate JSON property"));
                    }
                }
                Ok(Strict(Value::Object(output)))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut input: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut output = Vec::new();
                while let Some(Strict(value)) = input.next_element()? {
                    output.push(value);
                }
                Ok(Strict(Value::Array(output)))
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Strict, E> {
                Ok(Strict(value.into()))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Strict, E> {
                Ok(Strict(value.into()))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Strict, E> {
                Ok(Strict(value.into()))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Strict, E> {
                Ok(Strict(value.into()))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Strict, E> {
                Ok(Strict(value.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
        }
        deserializer.deserialize_any(StrictVisitor)
    }
}

/// Bind a parsed report to the selected process and complete prepared activation list.
pub fn report_matches(activation: &Value, report: &Value, pid: u32, start_file_time: &str) -> bool {
    report["deploymentRevision"] == activation["deploymentRevision"]
        && report["processId"].as_u64() == Some(u64::from(pid))
        && report["processStartFileTime"].as_str() == Some(start_file_time)
        && activation["mods"]
            .as_array()
            .zip(report["mods"].as_array())
            .is_some_and(|(expected, actual)| {
                expected.len() == actual.len()
                    && expected
                        .iter()
                        .zip(actual)
                        .all(|(a, b)| a["modId"] == b["modId"])
            })
}
