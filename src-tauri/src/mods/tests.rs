use super::*;
use crate::packages::{Operation, Prepared, PreparedFile, Status};
use sha2::{Digest, Sha256};
use std::fs;
use uuid::Uuid;

fn fixture() -> (tempfile::TempDir, Storage, Vec<LibraryEntry>) {
    fixture_with_lua(false)
}

fn fixture_with_lua(lua: bool) -> (tempfile::TempDir, Storage, Vec<LibraryEntry>) {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Storage::open(temp.path()).unwrap();
    let mut mods = Vec::new();
    let mut entries = Vec::new();
    for (index, id) in ["fixture.core", "fixture.addon"].iter().enumerate() {
        let bytes = format!("inert {id} fixture").into_bytes();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let artifact_hash = format!("{:x}", Sha256::digest(id.as_bytes()));
        let release_id = format!("{id}.1");
        let entry = LibraryEntry {
            reference: ModReference {
                mod_id: id.to_string(),
                hash: artifact_hash.clone(),
                origin: Origin::Catalog,
                release_id: Some(release_id.clone()),
            },
            name: id.to_string(),
            author: "Fixture".into(),
            version: "1".into(),
        };
        mods.push(json!({"id":id,"name":id,"author":"Fixture","sourceUrl":"https://example.invalid/source","releases":[{"id":release_id,"version":"1","withdrawn":false,"testedGameBuilds":[],"requires":if index == 0 {vec![]} else {vec!["fixture.core.1"]},"artifact":{"url":"https://example.invalid/package.zip","sha256":artifact_hash,"sizeBytes":10,"layout":{"kind":"starframe_managed_zip","root":if index == 0 {""} else {"package"},"entryAssembly":if index == 0 {"package/Fixture.dll"} else {"Fixture.dll"},"entryType":"Fixture.Entry"}}}]}));
        if lua {
            let release = &mut mods.last_mut().unwrap()["releases"][0];
            release["artifact"]["layout"] = json!({"kind":"starframe_lua_zip"});
            release["requires"] = json!([]);
        }
        let path = store
            .artifact_directory(&entry.reference)
            .unwrap()
            .join(if lua { "LJ/lua" } else { "package" });
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join(if lua { "Fixture.lua" } else { "Fixture.dll" }),
            &bytes,
        )
        .unwrap();
        let operation = Operation {
            id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            release_id,
            hash: artifact_hash.clone(),
            status: Status::Completed,
            message: "Fixture prepared".into(),
            received_bytes: 10,
            total_bytes: 10,
        };
        store.save_package(&operation).unwrap();
        store
            .complete_package(
                &operation,
                &entry,
                &Prepared {
                    hash: artifact_hash,
                    files: vec![PreparedFile {
                        path: if lua {
                            "LJ/lua/Fixture.lua"
                        } else {
                            "package/Fixture.dll"
                        }
                        .into(),
                        sha256: digest,
                        size_bytes: bytes.len() as u64,
                    }],
                },
            )
            .unwrap();
        entries.push(entry);
    }
    let catalog = Catalog::read(
        &serde_json::to_vec(
            &json!({"schemaVersion":if lua {2} else {1},"catalogRevision":"1","mods":mods}),
        )
        .unwrap(),
    )
    .unwrap();
    store
        .save_catalog_cache(&crate::catalog::refresh::Cache {
            catalog: Some(catalog),
            ..Default::default()
        })
        .unwrap();
    (temp, store, entries)
}

fn enable(store: &mut Storage, entry: &LibraryEntry, enabled: bool) -> View {
    let expected_revision = store.load().unwrap().revision.to_string();
    action(
        store,
        Action::SetEnabled {
            mod_id: entry.reference.mod_id.clone(),
            hash: entry.reference.hash.clone(),
            enabled,
            expected_revision,
        },
    )
    .unwrap()
}

#[test]
fn collision_winners_follow_effective_order_and_content_payloads_keep_their_paths() {
    let (_temp, mut store, entries) = fixture_with_lua(true);
    enable(&mut store, &entries[0], true);
    let initial = enable(&mut store, &entries[1], true);
    assert_eq!(initial.collisions.len(), 1);
    assert_eq!(initial.collisions[0].winner, "fixture.addon");
    assert_eq!(initial.collisions[0].path, "lj/lua/fixture.lua");
    let view = action(
        &mut store,
        Action::Reorder {
            mod_ids: vec!["fixture.addon".into(), "fixture.core".into()],
            expected_revision: initial.revision,
        },
    )
    .unwrap();
    assert_eq!(view.collisions[0].winner, "fixture.core");
    let activation = requested(&store).unwrap();
    assert!(activation["mods"][0]["entryAssembly"].is_null());
    assert!(
        payload(&store, &activation).unwrap()[0]
            .0
            .ends_with("LJ/lua/Fixture.lua")
    );
}

#[test]
fn named_collections_switch_and_delete_without_uninstall_and_survive_restart() {
    let (temp, mut store, entries) = fixture();
    let initial = enable(&mut store, &entries[1], true);
    let original = initial.active_collection.unwrap();
    let settings = temp.path().join("settings-fixture.cfg");
    fs::write(&settings, "keep settings").unwrap();
    let created = action(
        &mut store,
        Action::CreateCollection {
            name: "  Empty setup  ".into(),
            expected_revision: initial.revision,
        },
    )
    .unwrap();
    let empty = created
        .collections
        .iter()
        .find(|c| c.name == "Empty setup")
        .unwrap()
        .id
        .clone();
    assert_eq!(created.active_collection.as_ref(), Some(&original));
    assert_eq!(created.enabled.len(), 2);
    let selected = action(
        &mut store,
        Action::SelectCollection {
            id: empty.clone(),
            expected_revision: created.revision,
        },
    )
    .unwrap();
    assert!(selected.enabled.is_empty());
    assert_eq!(requested(&store).unwrap()["mods"], json!([]));
    let renamed = action(
        &mut store,
        Action::RenameCollection {
            id: original.clone(),
            name: "With mods".into(),
            expected_revision: selected.revision,
        },
    )
    .unwrap();
    let selected = action(
        &mut store,
        Action::SelectCollection {
            id: original.clone(),
            expected_revision: renamed.revision,
        },
    )
    .unwrap();
    assert_eq!(
        selected.enabled,
        entries
            .iter()
            .map(|e| e.reference.clone())
            .collect::<Vec<_>>()
    );
    let before = store.load().unwrap();
    drop(store);
    let mut store = Storage::open(temp.path()).unwrap();
    assert_eq!(store.load().unwrap(), before);
    let removed = action(
        &mut store,
        Action::DeleteCollection {
            id: empty,
            expected_revision: before.revision.to_string(),
        },
    )
    .unwrap();
    assert_eq!(removed.active_collection.as_ref(), Some(&original));
    let removed = action(
        &mut store,
        Action::DeleteCollection {
            id: original,
            expected_revision: removed.revision,
        },
    )
    .unwrap();
    assert!(removed.active_collection.is_none());
    assert!(removed.collections.is_empty());
    assert!(removed.enabled.is_empty());
    assert_eq!(removed.library, before.library);
    assert!(store.pending_removals().unwrap().is_empty());
    for entry in &entries {
        assert!(
            store
                .artifact_directory(&entry.reference)
                .unwrap()
                .join("package/Fixture.dll")
                .exists()
        );
    }
    assert_eq!(fs::read_to_string(settings).unwrap(), "keep settings");
    assert_eq!(requested(&store).unwrap()["mods"], json!([]));
}

#[test]
fn collection_edits_reject_stale_or_invalid_requests_and_keep_latest_order() {
    let (_temp, mut store, entries) = fixture_with_lua(true);
    enable(&mut store, &entries[0], true);
    let initial = enable(&mut store, &entries[1], true);
    let id = initial.active_collection.unwrap();
    let manifest = requested(&store).unwrap();
    let stale = initial.revision.clone();
    let mut current = initial.revision;
    for ids in [
        vec!["fixture.addon", "fixture.core"],
        vec!["fixture.core", "fixture.addon"],
        vec!["fixture.addon", "fixture.core"],
    ] {
        let updated = action(
            &mut store,
            Action::Reorder {
                mod_ids: ids.into_iter().map(str::to_owned).collect(),
                expected_revision: current,
            },
        )
        .unwrap();
        current = updated.revision;
    }
    assert!(payload(&store, &manifest).is_err());
    let before = store.load().unwrap();
    for edit in [
        Action::RenameCollection {
            id: id.clone(),
            name: "stale".into(),
            expected_revision: stale.clone(),
        },
        Action::DeleteCollection {
            id: id.clone(),
            expected_revision: stale.clone(),
        },
        Action::SelectCollection {
            id: id.clone(),
            expected_revision: stale.clone(),
        },
        Action::CreateCollection {
            name: "stale".into(),
            expected_revision: stale,
        },
        Action::CreateCollection {
            name: "  ".into(),
            expected_revision: current.clone(),
        },
        Action::RenameCollection {
            id: id.clone(),
            name: "x".repeat(201),
            expected_revision: current.clone(),
        },
        Action::DeleteCollection {
            id: "missing".into(),
            expected_revision: current.clone(),
        },
        Action::SelectCollection {
            id: "missing".into(),
            expected_revision: current,
        },
    ] {
        assert!(action(&mut store, edit).is_err());
        assert_eq!(store.load().unwrap(), before);
    }
    let latest = requested(&store).unwrap();
    assert_eq!(latest["mods"][0]["modId"], "fixture.addon");
    assert_eq!(latest["deploymentRevision"], before.revision.to_string());
}

#[test]
fn selecting_collection_with_uninstalled_content_reports_unresolved_membership() {
    let (_temp, mut store, entries) = fixture_with_lua(true);
    let initial = enable(&mut store, &entries[0], true);
    let id = initial.active_collection.unwrap();
    let rev = store
        .set_active_collection(None, initial.revision.parse().unwrap())
        .unwrap();
    let removed = action(
        &mut store,
        Action::Uninstall {
            mod_id: entries[0].reference.mod_id.clone(),
            hash: entries[0].reference.hash.clone(),
            expected_revision: rev.to_string(),
            confirm_references: true,
        },
    )
    .unwrap();
    let selected = action(
        &mut store,
        Action::SelectCollection {
            id,
            expected_revision: removed.revision,
        },
    )
    .unwrap();
    assert!(selected.order.is_none());
    assert!(
        selected
            .order_error
            .unwrap()
            .contains("unavailable in the library")
    );
    assert_eq!(selected.enabled, vec![entries[0].reference.clone()]);
    assert!(requested(&store).is_err());
}

#[test]
fn membership_orders_exact_dependencies_and_survives_restart_and_withdrawal() {
    let (temp, mut store, entries) = fixture();
    let view = enable(&mut store, &entries[1], true);
    assert_eq!(
        view.enabled,
        entries
            .iter()
            .map(|e| e.reference.clone())
            .collect::<Vec<_>>()
    );
    let activation = requested(&store).unwrap();
    assert_eq!(activation["mods"][0]["modId"], "fixture.core");
    assert_eq!(payload(&store, &activation).unwrap().len(), 2);
    let mut cache = store.catalog_cache().unwrap().unwrap();
    cache.catalog.as_mut().unwrap().catalog_revision = "2".into();
    cache.catalog.as_mut().unwrap().mods[0].releases[0].withdrawn = true;
    store.save_catalog_cache(&cache).unwrap();
    assert!(requested(&store).is_ok());
    enable(&mut store, &entries[0], false);
    assert!(
        requested(&store)
            .unwrap_err()
            .contains("disabled dependency")
    );
    enable(&mut store, &entries[1], false);
    assert_eq!(
        requested(&store).unwrap()["installedMods"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        requested(&store).unwrap()["mods"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .artifact_directory(&entries[0].reference)
            .unwrap()
            .exists()
    );
    drop(store);
    let store = Storage::open(temp.path()).unwrap();
    assert!(super::view(&store).unwrap().enabled.is_empty());
}

#[test]
fn archive_root_entry_is_relative_in_the_activation_manifest() {
    let (_temp, mut store, entries) = fixture();
    enable(&mut store, &entries[0], true);
    let activation = requested(&store).unwrap();
    assert_eq!(
        activation["mods"][0]["entryAssembly"],
        "package/Fixture.dll"
    );
    assert_eq!(payload(&store, &activation).unwrap().len(), 1);
}

#[test]
fn requested_priority_survives_restart_while_activation_keeps_dependencies_first() {
    let (temp, mut store, entries) = fixture();
    let initial = enable(&mut store, &entries[1], true);
    let reversed: Vec<_> = entries
        .iter()
        .rev()
        .map(|e| e.reference.mod_id.clone())
        .collect();
    let changed = action(
        &mut store,
        Action::Reorder {
            mod_ids: reversed.clone(),
            expected_revision: initial.revision.clone(),
        },
    )
    .unwrap();
    assert_eq!(changed.enabled[0], entries[1].reference);
    assert_eq!(changed.order.unwrap().effective[0], entries[0].reference);
    assert_eq!(
        requested(&store).unwrap()["mods"][0]["modId"],
        "fixture.core"
    );
    assert!(
        action(
            &mut store,
            Action::Reorder {
                mod_ids: reversed.clone(),
                expected_revision: initial.revision
            }
        )
        .is_err()
    );
    for invalid in [
        vec![reversed[0].clone(); 2],
        vec!["foreign".into(), reversed[0].clone()],
        vec![],
    ] {
        assert!(
            action(
                &mut store,
                Action::Reorder {
                    mod_ids: invalid,
                    expected_revision: changed.revision.clone()
                }
            )
            .is_err()
        );
    }
    let unchanged = enable(&mut store, &entries[1], true);
    assert_eq!(unchanged.enabled[0], entries[1].reference);
    drop(store);
    let store = Storage::open(temp.path()).unwrap();
    let restored = super::view(&store).unwrap();
    assert_eq!(restored.enabled[0], entries[1].reference);
    assert_eq!(restored.order.unwrap().effective[0], entries[0].reference);
}

#[test]
fn stale_edits_and_changed_library_files_never_authorize_payloads() {
    let (_temp, mut store, entries) = fixture();
    let old = store.load().unwrap().revision.to_string();
    enable(&mut store, &entries[0], true);
    assert!(
        action(
            &mut store,
            Action::SetEnabled {
                mod_id: entries[0].reference.mod_id.clone(),
                hash: entries[0].reference.hash.clone(),
                enabled: false,
                expected_revision: old
            }
        )
        .is_err()
    );
    let activation = requested(&store).unwrap();
    fs::write(
        store
            .artifact_directory(&entries[0].reference)
            .unwrap()
            .join("package/Fixture.dll"),
        b"changed",
    )
    .unwrap();
    assert!(payload(&store, &activation).is_err());
    assert_eq!(super::view(&store).unwrap().enabled.len(), 1);
}

#[test]
fn uninstall_requires_reference_confirmation_preserves_other_collections_and_settings() {
    let (temp, mut store, entries) = fixture();
    enable(&mut store, &entries[0], true);
    let other = Uuid::new_v4().to_string();
    store
        .save_collection(&other, "Other setup", &[entries[0].reference.clone()], 0)
        .unwrap();
    fs::write(temp.path().join("user-settings.cfg"), b"retain").unwrap();
    let revision = store.load().unwrap().revision;
    assert!(
        store
            .uninstall_mod(
                &entries[0].reference.mod_id,
                &entries[0].reference.hash,
                revision,
                false
            )
            .unwrap_err()
            .to_string()
            .contains("Other setup")
    );
    assert_eq!(store.load().unwrap().revision, revision);
    store
        .uninstall_mod(
            &entries[0].reference.mod_id,
            &entries[0].reference.hash,
            revision,
            true,
        )
        .unwrap();
    cleanup(&mut store).unwrap();
    assert!(
        !store
            .artifact_directory(&entries[0].reference)
            .unwrap()
            .exists()
    );
    assert_eq!(
        store
            .load()
            .unwrap()
            .collections
            .iter()
            .find(|c| c.id == other)
            .unwrap()
            .entries
            .len(),
        1
    );
    assert_eq!(
        fs::read(temp.path().join("user-settings.cfg")).unwrap(),
        b"retain"
    );
    assert!(store.pending_removals().unwrap().is_empty());
}

#[test]
fn uninstall_transaction_failure_preserves_membership_and_library() {
    let (temp, mut store, entries) = fixture();
    enable(&mut store, &entries[0], true);
    let before = store.load().unwrap();
    let connection = rusqlite::Connection::open(temp.path().join("sqlite/state.db")).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_uninstall BEFORE DELETE ON library BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;").unwrap();
    assert!(
        store
            .uninstall_mod(
                &entries[0].reference.mod_id,
                &entries[0].reference.hash,
                before.revision,
                true
            )
            .is_err()
    );
    assert_eq!(store.load().unwrap(), before);
    assert!(store.pending_removals().unwrap().is_empty());
    assert!(
        store
            .artifact_directory(&entries[0].reference)
            .unwrap()
            .exists()
    );
}

#[test]
#[cfg(windows)]
fn locked_uninstall_retains_persistent_cleanup_and_retries_after_restart() {
    use std::os::windows::fs::OpenOptionsExt;
    let (temp, mut store, entries) = fixture();
    let path = store
        .artifact_directory(&entries[0].reference)
        .unwrap()
        .join("package/Fixture.dll");
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .unwrap();
    store
        .uninstall_mod(
            &entries[0].reference.mod_id,
            &entries[0].reference.hash,
            store.load().unwrap().revision,
            false,
        )
        .unwrap();
    cleanup(&mut store).unwrap();
    assert!(!super::view(&store).unwrap().cleanup_errors.is_empty());
    drop(store);
    drop(lock);
    let mut store = Storage::open(temp.path()).unwrap();
    cleanup(&mut store).unwrap();
    assert!(super::view(&store).unwrap().cleanup_errors.is_empty());
}
