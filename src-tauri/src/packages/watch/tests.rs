use super::*;
use crate::{mods, sharing};
use serde_json::json;

#[test]
fn fallback_scan_and_resume_reconcile_without_native_notifications() {
    let (_root, mut store, original) = setup();
    let mut watcher = Watcher::start(&store, false);
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].message.contains("current")
    });
    fs::write(
        Path::new(&original.path).join("LJ/lua/local.lua"),
        b"return 'missed notification'",
    )
    .unwrap();
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].source.reference != original.reference
    });
    let latest = store.local_watches().unwrap()[0].source.clone();
    fs::write(
        Path::new(&original.path).join("LJ/lua/local.lua"),
        b"return 'resumed'",
    )
    .unwrap();
    watcher.poll(&mut store, false, true).unwrap();
    let resumed = Instant::now();
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].source.reference != latest.reference
    });
    assert!(
        resumed.elapsed() < RECHECK,
        "resume must not wait for the fallback interval"
    );
}

fn setup() -> (tempfile::TempDir, Storage, LocalSource) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir_all(source.join("LJ/lua")).unwrap();
    fs::write(source.join("LJ/lua/local.lua"), b"return 'original'").unwrap();
    fs::write(source.join(crate::local_import::MANIFEST), serde_json::to_vec(&json!({"schemaVersion":1,"modId":"fixture.watched","name":"Watched fixture","author":"Test fixture","version":"dev.1","layout":{"kind":"starframe_lua_zip"}})).unwrap()).unwrap();
    let mut store = Storage::open(&root.path().join("data")).unwrap();
    let mut queue = Packages::open(&mut store).unwrap();
    queue
        .import_local(
            &mut store,
            &Uuid::new_v4().to_string(),
            source.to_str().unwrap(),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while queue.busy() {
        queue.poll(&mut store).unwrap();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let local = store.local_sources().unwrap().pop().unwrap();
    let revision = store.load().unwrap().revision.to_string();
    mods::action(
        &mut store,
        mods::Action::SetEnabled {
            reference: local.reference.clone(),
            enabled: true,
            expected_revision: revision,
        },
    )
    .unwrap();
    (root, store, local)
}

fn until(watcher: &mut Watcher, store: &mut Storage, condition: impl Fn(&Storage) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        watcher.poll(store, false, false).unwrap();
        if condition(store) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "watcher timed out: {:?}",
            store.local_watches().unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn build(store: &Storage, source: &LocalSource) -> PreparedImport {
    local::prepare_source(
        store.package_root(),
        &Uuid::new_v4().to_string(),
        Path::new(&source.path),
        &Cancel::default(),
        &AtomicU64::new(0),
        local::Mode::Import,
    )
    .unwrap()
}

#[test]
fn migration_watches_unambiguous_sources_and_reimport_selects_an_ambiguous_source() {
    for multiple in [false, true] {
        let (root, mut store, original) = setup();
        if multiple {
            fs::write(
                Path::new(&original.path).join("LJ/lua/local.lua"),
                b"return 'second'",
            )
            .unwrap();
            let next = build(&store, &original);
            store
                .complete_watch(&original, next.local.as_ref().unwrap(), &next.prepared)
                .unwrap();
        }
        drop(store);
        let db = rusqlite::Connection::open(root.path().join("data/sqlite/state.db")).unwrap();
        db.execute_batch("DROP TABLE local_watches; PRAGMA user_version=11;")
            .unwrap();
        drop(db);
        let mut store = Storage::open(&root.path().join("data")).unwrap();
        assert_eq!(store.local_watches().unwrap().len(), usize::from(!multiple));
        if multiple {
            let next = build(&store, &original);
            let operation = Operation {
                id: Uuid::new_v4().to_string(),
                request_id: Uuid::new_v4().to_string(),
                release_id: "local-import".into(),
                hash: next.prepared.hash.clone(),
                status: Status::Completed,
                message: "Reimport fixture".into(),
                received_bytes: 1,
                total_bytes: 1,
            };
            let local = next.local.as_ref().unwrap();
            store.save_package(&operation).unwrap();
            store
                .complete_import(&operation, &local.entry(), &next.prepared, Some(local))
                .unwrap();
            assert_eq!(store.local_watches().unwrap()[0].source, *local);
        } else {
            assert_eq!(store.local_watches().unwrap()[0].source, original);
        }
    }
}

#[test]
fn watching_keeps_invalid_output_and_accepts_only_the_settled_latest_build() {
    let (_root, mut store, original) = setup();
    let source = Path::new(&original.path);
    let manifest = source.join(crate::local_import::MANIFEST);
    let metadata = fs::read(&manifest).unwrap();
    let mut watcher = Watcher::new(&store);
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].message.contains("current")
    });
    fs::write(&manifest, b"{").unwrap();
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].state == WatchState::Error
    });
    assert_eq!(
        mods::view(&store).unwrap().enabled,
        vec![original.reference.clone()]
    );
    fs::write(&manifest, metadata).unwrap();
    fs::write(source.join("LJ/lua/local.lua"), b"return 'first'").unwrap();
    std::thread::sleep(Duration::from_millis(200));
    fs::write(source.join("LJ/lua/local.lua"), b"return 'latest'").unwrap();
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].source.reference != original.reference
    });
    let latest = store.local_watches().unwrap()[0].source.clone();
    assert_eq!(
        mods::view(&store).unwrap().enabled,
        vec![latest.reference.clone()]
    );
    assert_eq!(
        fs::read(
            store
                .artifact_directory(&latest.reference)
                .unwrap()
                .join("LJ/lua/local.lua")
        )
        .unwrap(),
        b"return 'latest'"
    );
    assert_eq!(
        fs::read(
            store
                .artifact_directory(&original.reference)
                .unwrap()
                .join("LJ/lua/local.lua")
        )
        .unwrap(),
        b"return 'original'"
    );
    assert!(store.catalog_cache().unwrap().is_none());
}

#[test]
fn rebuild_commit_preserves_shared_exact_references_and_inactive_collections() {
    let (_root, mut store, original) = setup();
    let original_collection = store.load().unwrap().active_collection.unwrap();
    let text = sharing::action(
        &mut store,
        sharing::Action::Export {
            id: original_collection.clone(),
        },
    )
    .unwrap()
    .text
    .unwrap();
    let revision = store.load().unwrap().revision.to_string();
    sharing::action(
        &mut store,
        sharing::Action::Accept {
            text,
            request_id: Uuid::new_v4().to_string(),
            expected_revision: revision,
        },
    )
    .unwrap();
    let imported = store
        .load()
        .unwrap()
        .collections
        .into_iter()
        .find(|c| c.id != original_collection)
        .unwrap();
    let revision = store.load().unwrap().revision.to_string();
    mods::action(
        &mut store,
        mods::Action::SelectCollection {
            id: imported.id.clone(),
            expected_revision: revision,
        },
    )
    .unwrap();
    fs::write(
        Path::new(&original.path).join("LJ/lua/local.lua"),
        b"return 'new'",
    )
    .unwrap();
    let next = build(&store, &original);
    assert!(
        store
            .complete_watch(&original, next.local.as_ref().unwrap(), &next.prepared)
            .unwrap()
    );
    for collection in store.load().unwrap().collections {
        assert_eq!(collection.entries, vec![original.reference.clone()]);
    }
    let exported = sharing::action(&mut store, sharing::Action::Export { id: imported.id })
        .unwrap()
        .text
        .unwrap();
    assert_eq!(
        sharing::Portable::read(&exported).unwrap().entries,
        vec![original.reference]
    );
}

#[test]
fn missing_sources_recover_and_uninstall_rejects_a_late_prepared_build() {
    let (_root, mut store, original) = setup();
    let source = Path::new(&original.path);
    let mut watcher = Watcher::new(&store);
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].message.contains("current")
    });
    fs::remove_file(source.join("LJ/lua/local.lua")).unwrap();
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].state == WatchState::Error
    });
    assert_eq!(store.local_watches().unwrap()[0].source, original);
    drop(watcher);
    fs::write(source.join("LJ/lua/local.lua"), b"return 'restored'").unwrap();
    let next = build(&store, &original);
    let revision = store.load().unwrap().revision;
    store
        .uninstall_mod(&original.reference, revision, true)
        .unwrap();
    assert!(
        !store
            .complete_watch(&original, next.local.as_ref().unwrap(), &next.prepared)
            .unwrap()
    );
    assert!(store.local_watches().unwrap().is_empty());
    assert!(store.load().unwrap().library.is_empty());
    assert_eq!(
        fs::read(source.join("LJ/lua/local.lua")).unwrap(),
        b"return 'restored'"
    );
}

#[test]
fn restart_reconciles_changes_without_a_notification_and_rejects_new_identity() {
    let (root, mut store, original) = setup();
    fs::write(
        Path::new(&original.path).join("LJ/lua/local.lua"),
        b"return 'offline build'",
    )
    .unwrap();
    drop(store);
    store = Storage::open(&root.path().join("data")).unwrap();
    let mut watcher = Watcher::new(&store);
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].source.reference != original.reference
    });
    let latest = store.local_watches().unwrap()[0].source.clone();
    let mut metadata = latest.manifest.clone();
    metadata.mod_id = "fixture.different".into();
    fs::write(
        Path::new(&latest.path).join(crate::local_import::MANIFEST),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    until(&mut watcher, &mut store, |s| {
        s.local_watches().unwrap()[0].state == WatchState::Error
    });
    assert_eq!(
        store.local_watches().unwrap()[0].source.reference,
        latest.reference
    );
}

#[test]
fn rebuild_transaction_failure_preserves_head_collection_and_previous_bytes() {
    let (_root, mut store, original) = setup();
    fs::write(
        Path::new(&original.path).join("LJ/lua/local.lua"),
        b"return 'next'",
    )
    .unwrap();
    let next = build(&store, &original);
    let database =
        rusqlite::Connection::open(store.package_root().join("sqlite/state.db")).unwrap();
    database.execute_batch("CREATE TRIGGER reject_watch BEFORE UPDATE ON local_watches BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;").unwrap();
    assert!(
        store
            .complete_watch(&original, next.local.as_ref().unwrap(), &next.prepared)
            .is_err()
    );
    assert_eq!(store.local_watches().unwrap()[0].source, original);
    assert_eq!(
        mods::view(&store).unwrap().enabled,
        vec![original.reference.clone()]
    );
    database
        .execute_batch("DROP TRIGGER reject_watch;")
        .unwrap();
    assert!(
        store
            .complete_watch(&original, next.local.as_ref().unwrap(), &next.prepared)
            .unwrap()
    );
}
