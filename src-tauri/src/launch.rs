use crate::{runtime_contract, storage::Records};
use serde::Serialize;
use serde_json::{Value, json};
use std::{fs::File, io::Read, path::Path};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    SetupRequired,
    Ready,
    Preparing,
    LaunchRequested,
    ProcessObserved,
    RuntimeReady,
    RuntimeFailed,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LaunchView {
    pub phase: Phase,
    pub message: String,
    pub details: Vec<String>,
}
impl Default for LaunchView {
    fn default() -> Self {
        Self::new(
            Phase::SetupRequired,
            "Choose a game installation to finish setup.",
        )
    }
}
impl LaunchView {
    pub fn new(phase: Phase, message: &str) -> Self {
        Self {
            phase,
            message: message.into(),
            details: vec![],
        }
    }
}

pub fn requested(records: &Records) -> Result<Value, String> {
    let collection = records
        .active_collection
        .as_ref()
        .map(|id| {
            records
                .collections
                .iter()
                .find(|c| &c.id == id)
                .ok_or("The active collection is missing.")
        })
        .transpose()?;
    if collection.is_some_and(|c| !c.entries.is_empty()) {
        return Err("This build cannot prepare a collection with mods yet. The existing deployment was retained.".into());
    }
    Ok(json!({
        "schemaVersion": 2, "runtimeContractVersion": 1,
        "integrationId": "starframe.bepinex", "deploymentRevision": records.revision.to_string(),
        "installedMods": [], "mods": []
    }))
}

/// Re-read the requested revision after preparation and immediately before dispatch.
pub fn prepare_latest(
    mut requested: impl FnMut() -> Result<Value, String>,
    mut prepare: impl FnMut(&Value) -> Result<(), String>,
    mut dispatch: impl FnMut() -> Result<(), String>,
) -> Result<Value, String> {
    for _ in 0..4 {
        let next = requested()?;
        prepare(&next)?;
        if requested()? != next {
            continue;
        }
        dispatch()?;
        return Ok(next);
    }
    Err("The requested setup kept changing. Launch was not requested; try again.".into())
}

pub fn read_report(path: &Path) -> Result<Option<Value>, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Could not read the runtime report: {e}")),
    };
    let mut bytes = Vec::new();
    file.take(runtime_contract::MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    runtime_contract::read(&bytes, "report").map(Some)
}

pub fn runtime_view(
    activation: &Value,
    report: Option<&Value>,
    pid: u32,
    start: &str,
) -> LaunchView {
    let Some(report) =
        report.filter(|r| runtime_contract::report_matches(activation, r, pid, start))
    else {
        return LaunchView::new(
            Phase::ProcessObserved,
            "Game started. Waiting for this game's runtime report.",
        );
    };
    let errors: Vec<String> = report["mods"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["outcome"] != "loaded")
        .map(|m| {
            format!(
                "{}: {} ({})",
                m["modId"].as_str().unwrap(),
                m["message"].as_str().unwrap(),
                m["errorCode"].as_str().unwrap()
            )
        })
        .collect();
    if errors.is_empty() {
        LaunchView::new(
            Phase::RuntimeReady,
            &format!(
                "Game running. Runtime confirmed {} active mods.",
                report["mods"].as_array().unwrap().len()
            ),
        )
    } else {
        LaunchView {
            phase: Phase::RuntimeFailed,
            message: "Game running. Some mods failed to activate.".into(),
            details: errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    #[test]
    fn launch_uses_latest_revision_and_does_not_dispatch_after_failure() {
        let revision = Cell::new(1);
        let prepared = RefCell::new(vec![]);
        let launched = Cell::new(false);
        let result = prepare_latest(
            || Ok(json!({"revision": revision.get()})),
            |value| {
                prepared
                    .borrow_mut()
                    .push(value["revision"].as_i64().unwrap());
                revision.set(2);
                Ok(())
            },
            || {
                launched.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*prepared.borrow(), vec![1, 2]);
        assert_eq!(result["revision"], 2);
        assert!(launched.get());
        launched.set(false);
        assert!(
            prepare_latest(
                || Ok(json!(1)),
                |_| Err("Game started during write".into()),
                || {
                    launched.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!launched.get());
        assert!(
            prepare_latest(
                || Ok(json!(1)),
                |_| Ok(()),
                || Err("Executable launch failed".into())
            )
            .is_err()
        );
    }
    #[test]
    fn continuous_edits_stop_without_launch_and_nonempty_collections_are_not_erased() {
        let revision = Cell::new(0);
        let launched = Cell::new(false);
        assert!(
            prepare_latest(
                || {
                    revision.set(revision.get() + 1);
                    Ok(json!(revision.get()))
                },
                |_| Ok(()),
                || {
                    launched.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!launched.get());
        let mut records = Records {
            revision: 3,
            library: vec![],
            collections: vec![],
            active_collection: None,
        };
        let activation = requested(&records).unwrap();
        runtime_contract::read(&serde_json::to_vec(&activation).unwrap(), "activation").unwrap();
        assert_eq!(activation["deploymentRevision"], "3");
        records.active_collection = Some("missing".into());
        assert!(requested(&records).is_err());
        records.collections.push(crate::storage::Collection {
            id: "missing".into(),
            name: "Test".into(),
            revision: 1,
            entries: vec![crate::storage::ModReference {
                mod_id: "mod".into(),
                hash: "0".repeat(64),
                origin: crate::storage::Origin::LocalImport,
                release_id: None,
            }],
        });
        assert!(requested(&records).unwrap_err().contains("cannot prepare"));
        assert_eq!(records.collections[0].entries.len(), 1);
    }

    #[test]
    fn reports_require_the_current_process_start_revision_and_complete_mod_list() {
        let activation = json!({"deploymentRevision":"7", "mods":[{"modId":"first"}]});
        let mut report = json!({"deploymentRevision":"7","processId":42,"processStartFileTime":"123", "mods":[{"modId":"first","outcome":"loaded","message":"","errorCode":""}]});
        assert_eq!(
            runtime_view(&activation, Some(&report), 42, "123").phase,
            Phase::RuntimeReady
        );
        for (pid, start) in [(43, "123"), (42, "124")] {
            assert_eq!(
                runtime_view(&activation, Some(&report), pid, start).phase,
                Phase::ProcessObserved
            );
        }
        report["mods"][0]["outcome"] = json!("failed");
        report["mods"][0]["message"] = json!("Fixture failure");
        report["mods"][0]["errorCode"] = json!("load_failed");
        let view = runtime_view(&activation, Some(&report), 42, "123");
        assert_eq!(view.phase, Phase::RuntimeFailed);
        assert!(view.details[0].contains("Fixture failure"));
        report["deploymentRevision"] = json!("6");
        assert_eq!(
            runtime_view(&activation, Some(&report), 42, "123").phase,
            Phase::ProcessObserved
        );
        report["deploymentRevision"] = json!("7");
        report["mods"] = json!([]);
        assert_eq!(
            runtime_view(&activation, Some(&report), 42, "123").phase,
            Phase::ProcessObserved
        );
    }
}
