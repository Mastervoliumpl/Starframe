use super::*;
use crate::{
    deployment::{fixture_local, fixture_registry},
    sharing,
};
use std::{
    fs,
    time::{Duration, Instant},
};
use uuid::Uuid;

fn enable(store: &mut Storage, reference: &Reference) -> View {
    let expected_revision = store.load().unwrap().revision.to_string();
    action(
        store,
        Action::SetEnabled {
            reference: reference.clone(),
            enabled: true,
            expected_revision,
        },
    )
    .unwrap()
}
fn select(store: &mut Storage, id: &str) -> View {
    let expected_revision = store.load().unwrap().revision.to_string();
    action(
        store,
        Action::SelectCollection {
            id: id.into(),
            expected_revision,
        },
    )
    .unwrap()
}
fn finish_imports(store: &mut Storage, queue: &mut packages::Packages) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        queue.poll(store).unwrap();
        sharing::poll(store, queue).unwrap();
        if store.collection_imports().unwrap().iter().all(|import| {
            import.entries.iter().all(|entry| {
                !matches!(
                    entry.status,
                    sharing::Status::Pending | sharing::Status::Preparing
                )
            })
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "Import verification timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn persisted_mixed_collections_drive_activation_and_survive_restart_and_backup() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let mut store = Storage::open(&data).unwrap();
    let code = Reference::Registry(fixture_registry(&mut store, 0, 1, &[]));
    let map = Reference::Registry(fixture_registry(&mut store, 2, 2, &[]));
    let ai = Reference::Registry(fixture_registry(&mut store, 3, 3, &[]));
    let local =
        Reference::try_from(&fixture_local(&mut store, temp.path(), "fixture.local")).unwrap();
    for reference in [&map, &local, &code, &ai] {
        enable(&mut store, reference);
    }
    let before = store.load().unwrap();
    assert_eq!(before.collections[0].entries, vec![map, local, code, ai]);
    let activation = requested(&store).unwrap();
    assert_eq!(activation["mods"].as_array().unwrap().len(), 2);
    assert_eq!(activation["mods"][0]["modId"], "fixture.local");
    assert_eq!(activation["mods"][1]["modId"], "registry.1");
    assert_eq!(payload(&store, &activation).unwrap().len(), 7);
    let backup = store.backup().unwrap();
    drop(store);
    let store = Storage::open(&data).unwrap();
    assert_eq!(store.load().unwrap(), before);
    assert_eq!(requested(&store).unwrap(), activation);
    assert!(payload(&store, &activation).is_ok());
    let restored = Storage::restore_into(&backup, &temp.path().join("restored")).unwrap();
    assert_eq!(restored.load().unwrap(), before);
    assert_eq!(restored.installed_registry_releases().unwrap().len(), 3);
    assert_eq!(restored.local_sources().unwrap().len(), 1);
}

#[test]
fn stale_edits_and_corrupt_managed_content_never_authorize_membership_or_payload() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Storage::open(&temp.path().join("data")).unwrap();
    let reference = Reference::Registry(fixture_registry(&mut store, 0, 1, &[]));
    let current = enable(&mut store, &reference);
    let activation = requested(&store).unwrap();
    let before = store.load().unwrap();
    assert!(
        action(
            &mut store,
            Action::SetEnabled {
                reference: reference.clone(),
                enabled: false,
                expected_revision: "0".into()
            }
        )
        .is_err()
    );
    assert_eq!(store.load().unwrap(), before);
    fs::write(
        store
            .package_root()
            .join("artifacts")
            .join(reference.hash())
            .join("LJ/lua/Example/main.lua"),
        b"tampered",
    )
    .unwrap();
    assert!(payload(&store, &activation).is_err());
    assert!(
        action(
            &mut store,
            Action::SetEnabled {
                reference,
                enabled: true,
                expected_revision: current.revision
            }
        )
        .is_err()
    );
    assert_eq!(store.load().unwrap(), before);
}

#[test]
fn exact_dependencies_constrain_manual_priority_without_replacing_pinned_releases() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Storage::open(&temp.path().join("data")).unwrap();
    let core = fixture_registry(&mut store, 1, 1, &[]);
    let dependent = fixture_registry(
        &mut store,
        0,
        2,
        &[crate::registry::Dependency::Exact {
            mod_id: core.mod_id,
            release_id: core.release_id,
        }],
    );
    let dependent = Reference::Registry(dependent);
    let before = store.load().unwrap();
    assert!(
        action(
            &mut store,
            Action::SetEnabled {
                reference: dependent.clone(),
                enabled: true,
                expected_revision: before.revision.to_string()
            }
        )
        .is_err()
    );
    assert_eq!(store.load().unwrap(), before);
    let core = Reference::Registry(core);
    enable(&mut store, &core);
    let current = enable(&mut store, &dependent);
    let current = action(
        &mut store,
        Action::Reorder {
            mod_ids: vec![dependent.runtime_id(), core.runtime_id()],
            expected_revision: current.revision,
        },
    )
    .unwrap();
    assert_eq!(current.enabled, vec![dependent.clone(), core.clone()]);
    assert_eq!(
        current.order.unwrap().effective,
        vec![core.clone(), dependent.clone()]
    );
    let newer = Reference::Registry(fixture_registry(&mut store, 1, 1, &[]));
    let revision = store.load().unwrap().revision.to_string();
    assert!(
        action(
            &mut store,
            Action::SetEnabled {
                reference: newer,
                enabled: true,
                expected_revision: revision
            }
        )
        .is_err()
    );
    assert_eq!(view(&store).unwrap().enabled, vec![dependent, core]);
}

#[test]
fn sharing_schema_two_keeps_exact_order_and_offline_verification_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let mut store = Storage::open(&data).unwrap();
    let reference = Reference::Registry(fixture_registry(&mut store, 0, 1, &[]));
    let local =
        Reference::try_from(&fixture_local(&mut store, temp.path(), "fixture.local")).unwrap();
    enable(&mut store, &local);
    let current = enable(&mut store, &reference);
    let text = sharing::action(
        &mut store,
        sharing::Action::Export {
            id: current.active_collection.unwrap(),
        },
    )
    .unwrap()
    .text
    .unwrap();
    let parsed = sharing::Portable::read(&text).unwrap();
    assert_eq!(parsed.schema_version, 2);
    assert_eq!(parsed.entries, vec![local, reference]);
    assert!(!text.contains(temp.path().to_str().unwrap()));
    let request_id = Uuid::new_v4().to_string();
    let mut queue = packages::Packages::open(&mut store).unwrap();
    let revision = store.load().unwrap().revision.to_string();
    let reply = sharing::action(
        &mut store,
        sharing::Action::Accept {
            text: text.clone(),
            request_id: request_id.clone(),
            expected_revision: revision.clone(),
        },
    )
    .unwrap();
    let retry = sharing::action(
        &mut store,
        sharing::Action::Accept {
            text: text.clone(),
            request_id,
            expected_revision: revision,
        },
    )
    .unwrap();
    assert_eq!(reply.collection_id, retry.collection_id);
    finish_imports(&mut store, &mut queue);
    assert!(
        store.collection_imports().unwrap()[0]
            .entries
            .iter()
            .all(|entry| entry.status == sharing::Status::Ready)
    );
    let id = reply.collection_id.unwrap();
    select(&mut store, &id);
    requested(&store).unwrap();
    assert_eq!(
        sharing::action(&mut store, sharing::Action::Export { id })
            .unwrap()
            .text
            .unwrap(),
        text
    );
    drop(queue);
    drop(store);
    let store = Storage::open(&data).unwrap();
    requested(&store).unwrap();
    assert!(payload(&store, &requested(&store).unwrap()).is_ok());
}

#[test]
fn shared_corruption_and_interruption_preserve_refs_for_explicit_retry() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let mut store = Storage::open(&data).unwrap();
    let reference = Reference::Registry(fixture_registry(&mut store, 0, 1, &[]));
    let document = sharing::Portable {
        format: "starframe-collection".into(),
        schema_version: 2,
        name: "Shared".into(),
        entries: vec![reference.clone()],
    };
    let mut queue = packages::Packages::open(&mut store).unwrap();
    let revision = store.load().unwrap().revision.to_string();
    let reply = sharing::action(
        &mut store,
        sharing::Action::Accept {
            text: serde_json::to_string(&document).unwrap(),
            request_id: Uuid::new_v4().to_string(),
            expected_revision: revision,
        },
    )
    .unwrap();
    let id = reply.collection_id.unwrap();
    let file = store
        .package_root()
        .join("artifacts")
        .join(reference.hash())
        .join("LJ/lua/Example/main.lua");
    fs::write(&file, b"tampered").unwrap();
    finish_imports(&mut store, &mut queue);
    assert_eq!(
        store.collection_imports().unwrap()[0].entries[0].status,
        sharing::Status::Unresolved
    );
    select(&mut store, &id);
    assert!(requested(&store).is_err());
    assert_eq!(
        store.load().unwrap().collections[0].entries,
        vec![reference]
    );
    fs::write(&file, b"return {}").unwrap();
    sharing::action(&mut store, sharing::Action::Retry { id: id.clone() }).unwrap();
    drop(queue);
    drop(store);
    let mut store = Storage::open(&data).unwrap();
    sharing::recover(&mut store).unwrap();
    assert_eq!(
        store.collection_imports().unwrap()[0].entries[0].status,
        sharing::Status::Unresolved
    );
    let mut queue = packages::Packages::open(&mut store).unwrap();
    sharing::action(&mut store, sharing::Action::Retry { id }).unwrap();
    finish_imports(&mut store, &mut queue);
    requested(&store).unwrap();
}

#[test]
fn collection_documents_reject_legacy_ambiguous_and_untrusted_fields() {
    let local = serde_json::json!({"kind":"local","reference":{"modId":"fixture.local","sha256":"ab".repeat(32)}});
    let valid = serde_json::json!({"format":"starframe-collection","schemaVersion":2,"name":"Fixture","entries":[local.clone()]});
    sharing::Portable::read(&valid.to_string()).unwrap();
    for invalid in [
        serde_json::json!({"format":"starframe-collection","schemaVersion":1,"name":"Old","entries":[]}),
        serde_json::json!({"format":"starframe-collection","schemaVersion":2,"name":"Duplicate","entries":[local.clone(),local]}),
        serde_json::json!({"format":"starframe-collection","schemaVersion":2,"name":"URL","entries":[{"kind":"local","reference":{"modId":"fixture","sha256":"ab".repeat(32),"url":"https://example.invalid/archive"}}]}),
        serde_json::json!({"format":"starframe-collection","schemaVersion":2,"name":"Path","entries":[{"kind":"local","reference":{"modId":"../outside","sha256":"ab".repeat(32)}}]}),
        serde_json::json!({"format":"starframe-collection","schemaVersion":2,"name":"Settings","entries":[],"settings":{}}),
    ] {
        assert!(sharing::Portable::read(&invalid.to_string()).is_err());
    }
    assert!(sharing::Portable::read("{\"format\":\"starframe-collection\",\"schemaVersion\":2,\"name\":\"first\",\"name\":\"second\",\"entries\":[]}").is_err());
    assert!(sharing::Portable::read(&" ".repeat(sharing::MAX_BYTES + 1)).is_err());
}

#[test]
fn missing_shared_registry_work_is_scoped_idempotent_and_recoverable() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Storage::open(temp.path()).unwrap();
    let installed = Reference::Registry(fixture_registry(&mut store, 0, 1, &[]));
    let missing = crate::registry::ExactReference {
        mod_id: crate::registry::ModId::try_from(2).unwrap(),
        release_id: crate::registry::ReleaseId(Uuid::new_v4()),
        sha256: crate::registry::Sha256::try_from("ab".repeat(32)).unwrap(),
    };
    let local = Reference::Local(crate::references::LocalReference {
        mod_id: "fixture.missing".into(),
        sha256: crate::registry::Sha256::try_from("cd".repeat(32)).unwrap(),
    });
    let document = sharing::Portable {
        format: "starframe-collection".into(),
        schema_version: 2,
        name: "Mixed missing content".into(),
        entries: vec![
            installed.clone(),
            Reference::Registry(missing.clone()),
            local.clone(),
        ],
    };
    let mut queue = packages::Packages::open(&mut store).unwrap();
    let expected_revision = store.load().unwrap().revision.to_string();
    let id = sharing::action(
        &mut store,
        sharing::Action::Accept {
            text: serde_json::to_string(&document).unwrap(),
            request_id: Uuid::new_v4().to_string(),
            expected_revision,
        },
    )
    .unwrap()
    .collection_id
    .unwrap();
    let planned = sharing::online_action(
        &mut store,
        &mut queue,
        sharing::OnlineAction::Plan { id: id.clone() },
    )
    .unwrap();
    assert!(
        matches!(planned, sharing::OnlineReply::Plan(ref entries) if entries == std::slice::from_ref(&missing))
    );
    assert!(
        matches!(sharing::online_action(&mut store, &mut queue, sharing::OnlineAction::Plan { id:id.clone() }).unwrap(), sharing::OnlineReply::Plan(ref entries) if entries.is_empty())
    );
    for _ in 0..50 {
        queue.poll(&mut store).unwrap();
        sharing::poll(&mut store, &mut queue).unwrap();
        if store.collection_imports().unwrap()[0].entries[0].status == sharing::Status::Ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    sharing::online_action(
        &mut store,
        &mut queue,
        sharing::OnlineAction::Fail {
            id: id.clone(),
            reference: missing.clone(),
            message: "Sign in to download this exact release.".into(),
        },
    )
    .unwrap();
    let entries = &store.collection_imports().unwrap()[0].entries;
    assert_eq!(entries[0].status, sharing::Status::Ready);
    assert_eq!(entries[1].status, sharing::Status::Unresolved);
    assert!(entries[1].message.contains("Sign in"));
    assert_eq!(entries[2].status, sharing::Status::Unresolved);
    assert!(entries[2].message.contains("Local-only"));
    assert_eq!(
        store.load().unwrap().collections[0].entries,
        document.entries
    );
    sharing::action(&mut store, sharing::Action::Retry { id: id.clone() }).unwrap();
    sharing::online_action(
        &mut store,
        &mut queue,
        sharing::OnlineAction::Plan { id: id.clone() },
    )
    .unwrap();
    drop(queue);
    drop(store);
    let mut store = Storage::open(temp.path()).unwrap();
    let _queue = packages::Packages::open(&mut store).unwrap();
    assert_eq!(
        store.collection_imports().unwrap()[0].entries[1].status,
        sharing::Status::Unresolved
    );
    assert!(
        store.collection_imports().unwrap()[0].entries[1]
            .message
            .contains("stopped")
    );
    assert_eq!(
        store.load().unwrap().collections[0].entries,
        document.entries
    );
}

#[test]
fn confirmed_uninstall_keeps_other_refs_and_rolls_back_on_storage_failure() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Storage::open(&temp.path().join("data")).unwrap();
    let reference = Reference::Registry(fixture_registry(&mut store, 0, 1, &[]));
    enable(&mut store, &reference);
    let old = store.load().unwrap().active_collection.unwrap();
    let other = Uuid::new_v4().to_string();
    store
        .save_collection(&other, "Other", std::slice::from_ref(&reference), 0)
        .unwrap();
    let before = store.load().unwrap();
    assert!(
        store
            .uninstall_reference(&reference, before.revision, false)
            .is_err()
    );
    let database =
        rusqlite::Connection::open(store.package_root().join("sqlite/state.db")).unwrap();
    database.execute_batch("CREATE TRIGGER fail_remove BEFORE UPDATE ON registry_library BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
    assert!(
        store
            .uninstall_reference(&reference, before.revision, true)
            .is_err()
    );
    assert_eq!(store.load().unwrap(), before);
    database.execute_batch("DROP TRIGGER fail_remove;").unwrap();
    let settings = store.package_root().join("settings/retained.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(&settings, b"retained").unwrap();
    let revision = store.load().unwrap().revision.to_string();
    action(
        &mut store,
        Action::Uninstall {
            reference: reference.clone(),
            expected_revision: revision,
            confirm_references: true,
        },
    )
    .unwrap();
    let records = store.load().unwrap();
    assert!(
        records
            .collections
            .iter()
            .find(|collection| collection.id == old)
            .unwrap()
            .entries
            .is_empty()
    );
    assert_eq!(
        records
            .collections
            .iter()
            .find(|collection| collection.id == other)
            .unwrap()
            .entries,
        vec![reference.clone()]
    );
    assert!(
        !store
            .package_root()
            .join("artifacts")
            .join(reference.hash())
            .exists()
    );
    assert_eq!(fs::read(settings).unwrap(), b"retained");
    assert!(view(&store).unwrap().library.is_empty());
    assert!(select(&mut store, &other).order_error.is_some());
    assert!(requested(&store).is_err());
}
