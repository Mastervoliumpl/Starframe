use super::*;
use crate::{mods, sharing};
use serde_json::json;
use std::time::Instant;

fn fixture(source: &Path, id: &str, lua: bool) {
    fs::create_dir_all(source).unwrap();
    let layout = if lua {
        fs::create_dir_all(source.join("LJ/lua")).unwrap();
        fs::write(source.join("LJ/lua/local.lua"), b"return 'fixture'").unwrap();
        json!({"kind":"starframe_lua_zip"})
    } else {
        fs::write(source.join("Mod.dll"), b"inert DLL fixture; never executed").unwrap();
        json!({"kind":"starframe_managed_zip", "root":"", "entryAssembly":"Mod.dll", "entryType":"Fixture.Mod"})
    };
    fs::write(source.join(MANIFEST), serde_json::to_vec(&json!({"schemaVersion":1,"modId":id,"name":id,"author":"Test fixture","version":"dev.1","layout":layout})).unwrap()).unwrap();
}

fn wait(queue: &mut Packages, store: &mut Storage, request: &str) -> Operation {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        queue.poll(store).unwrap();
        let op = store.package_request(request).unwrap().unwrap();
        if !matches!(op.status, Status::Preparing | Status::Cancelling) {
            return op;
        }
        assert!(Instant::now() < deadline, "local import did not finish");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn import(queue: &mut Packages, store: &mut Storage, path: &Path) -> ModReference {
    let request = Uuid::new_v4().to_string();
    queue
        .import_local(store, &request, path.to_str().unwrap())
        .unwrap();
    let op = wait(queue, store, &request);
    assert_eq!(op.status, Status::Completed, "{}", op.message);
    store
        .load()
        .unwrap()
        .library
        .into_iter()
        .find(|e| e.reference.hash == op.hash)
        .unwrap()
        .reference
}

#[test]
fn offline_dll_and_folder_imports_use_managed_copies_and_retain_sources_after_restart_and_uninstall()
 {
    for lua in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let data = root.path().join("data");
        fixture(&source, "fixture.local", lua);
        let chosen = if lua {
            source.clone()
        } else {
            fs::rename(source.join(MANIFEST), source.join("Mod.starframe.json")).unwrap();
            source.join("Mod.dll")
        };
        let original = if lua {
            source.join("LJ/lua/local.lua")
        } else {
            chosen.clone()
        };
        let before = fs::read(&original).unwrap();
        let mut store = Storage::open(&data).unwrap();
        let mut queue = Packages::open(&mut store).unwrap();
        assert!(store.catalog_cache().unwrap().is_none());
        let reference = import(&mut queue, &mut store, &chosen);
        assert!(store.catalog_cache().unwrap().is_none());
        let revision = store.load().unwrap().revision.to_string();
        let view = mods::action(
            &mut store,
            mods::Action::SetEnabled {
                reference: reference.clone(),
                enabled: true,
                expected_revision: revision,
            },
        )
        .unwrap();
        assert_eq!(view.enabled, vec![reference.clone()]);
        assert!(view.order_error.is_none());
        let activation = mods::requested(&store).unwrap();
        assert_eq!(activation["mods"][0]["source"]["kind"], "local");
        assert!(!activation.to_string().contains(source.to_str().unwrap()));
        assert!(!mods::payload(&store, &activation).unwrap().is_empty());
        drop(queue);
        drop(store);
        let mut store = Storage::open(&data).unwrap();
        assert_eq!(
            store.local_sources().unwrap()[0].path,
            chosen.to_str().unwrap()
        );
        fs::write(&original, b"next author build").unwrap();
        let activation = mods::requested(&store).unwrap();
        let payload = mods::payload(&store, &activation).unwrap();
        match &payload[0].1 {
            crate::deployment::Source::File { path, .. } => {
                assert_eq!(fs::read(path).unwrap(), before)
            }
            _ => panic!("expected a managed file"),
        }
        let revision = store.load().unwrap().revision.to_string();
        mods::action(
            &mut store,
            mods::Action::Uninstall {
                reference: reference.clone(),
                expected_revision: revision,
                confirm_references: true,
            },
        )
        .unwrap();
        assert_eq!(fs::read(&original).unwrap(), b"next author build");
        assert!(store.local_sources().unwrap().is_empty());
        assert!(!store.artifact_directory(&reference).unwrap().exists());
        assert!(
            source
                .join(if lua { MANIFEST } else { "Mod.starframe.json" })
                .exists()
        );
    }
}

#[test]
fn content_matches_across_source_locations_and_collections_keep_unmatched_requirements() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fixture(&first, "fixture.local", true);
    fixture(&second, "fixture.local", true);
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(second.join(MANIFEST)).unwrap()).unwrap();
    fs::write(
        second.join(MANIFEST),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let mut store = Storage::open(&root.path().join("data")).unwrap();
    let mut queue = Packages::open(&mut store).unwrap();
    let reference = import(&mut queue, &mut store, &first);
    let original = store.package_operations().unwrap()[0].clone();
    assert_eq!(
        queue
            .import_local(&mut store, &original.request_id, first.to_str().unwrap())
            .unwrap()
            .id,
        original.id
    );
    assert!(
        queue
            .import_local(&mut store, &original.request_id, second.to_str().unwrap())
            .is_err()
    );
    assert_eq!(import(&mut queue, &mut store, &second), reference);
    assert_eq!(store.load().unwrap().library.len(), 1);
    let text = serde_json::to_string(&sharing::Portable {
        format: "starframe-collection".into(),
        schema_version: 1,
        name: "Local sharing fixture".into(),
        entries: vec![reference.clone()],
    })
    .unwrap();
    let review =
        sharing::action(&mut store, sharing::Action::Review { text: text.clone() }).unwrap();
    assert!(review.order_error.is_none());
    assert_eq!(review.entries[0].status, sharing::Status::Pending);
    let id = Uuid::new_v4().to_string();
    let expected_revision = store.load().unwrap().revision.to_string();
    sharing::action(
        &mut store,
        sharing::Action::Accept {
            text: text.clone(),
            request_id: id.clone(),
            expected_revision,
        },
    )
    .unwrap();
    for _ in 0..200 {
        queue.poll(&mut store).unwrap();
        sharing::poll(&mut store, &mut queue).unwrap();
        if store.collection_imports().unwrap()[0].entries[0].status == sharing::Status::Ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        store.collection_imports().unwrap()[0].entries[0].status,
        sharing::Status::Ready
    );
    assert_eq!(
        sharing::action(&mut store, sharing::Action::Export { id })
            .unwrap()
            .text
            .unwrap(),
        serde_json::to_string_pretty(&sharing::Portable::read(&text).unwrap()).unwrap()
    );
    let mut empty = Storage::open(&root.path().join("empty")).unwrap();
    let missing = sharing::action(&mut empty, sharing::Action::Review { text }).unwrap();
    assert_eq!(missing.entries[0].status, sharing::Status::Unresolved);
    assert!(missing.entries[0].message.contains("Local-only"));
    assert!(empty.package_operations().unwrap().is_empty());
}

#[test]
fn local_dependencies_are_enabled_exactly_and_constrain_manual_order() {
    let root = tempfile::tempdir().unwrap();
    let core = root.path().join("core");
    let dependent = root.path().join("dependent");
    fixture(&core, "fixture.core", true);
    fixture(&dependent, "fixture.dependent", true);
    let mut store = Storage::open(&root.path().join("data")).unwrap();
    let mut queue = Packages::open(&mut store).unwrap();
    let core_ref = import(&mut queue, &mut store, &core);
    let mut metadata: Manifest =
        serde_json::from_slice(&fs::read(dependent.join(MANIFEST)).unwrap()).unwrap();
    metadata.requires = vec![core_ref.clone()];
    fs::write(
        dependent.join(MANIFEST),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    let dependent_ref = import(&mut queue, &mut store, &dependent);
    let expected_revision = store.load().unwrap().revision.to_string();
    let view = mods::action(
        &mut store,
        mods::Action::SetEnabled {
            reference: dependent_ref.clone(),
            enabled: true,
            expected_revision,
        },
    )
    .unwrap();
    assert_eq!(view.enabled, vec![core_ref.clone(), dependent_ref.clone()]);
    let view = mods::action(
        &mut store,
        mods::Action::Reorder {
            mod_ids: vec![dependent_ref.mod_id.clone(), core_ref.mod_id.clone()],
            expected_revision: view.revision,
        },
    )
    .unwrap();
    assert_eq!(
        view.order.unwrap().effective,
        vec![core_ref.clone(), dependent_ref]
    );
    let view = mods::action(
        &mut store,
        mods::Action::SetEnabled {
            reference: core_ref,
            enabled: false,
            expected_revision: view.revision,
        },
    )
    .unwrap();
    assert!(view.order_error.unwrap().contains("requires"));
    assert!(mods::requested(&store).is_err());
}

#[test]
fn invalid_inputs_and_cancelled_imports_never_enter_the_library() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fixture(&source, "fixture.local", true);
    let mut store = Storage::open(&root.path().join("data")).unwrap();
    let mut queue = Packages::open(&mut store).unwrap();
    for path in ["relative.dll", "C:/source/../bad", ""] {
        assert!(
            queue
                .import_local(&mut store, &Uuid::new_v4().to_string(), path)
                .is_err()
        );
    }
    fs::write(source.join("unsupported.txt"), b"wrong Lua layout").unwrap();
    let request = Uuid::new_v4().to_string();
    queue
        .import_local(&mut store, &request, source.to_str().unwrap())
        .unwrap();
    assert_eq!(
        wait(&mut queue, &mut store, &request).status,
        Status::Failed
    );
    assert!(store.load().unwrap().library.is_empty());
    fs::remove_file(source.join("unsupported.txt")).unwrap();
    let request = Uuid::new_v4().to_string();
    let op = queue
        .import_local(&mut store, &request, source.to_str().unwrap())
        .unwrap();
    queue.cancel(&mut store, &op.id).unwrap();
    assert_eq!(
        wait(&mut queue, &mut store, &request).status,
        Status::Cancelled
    );
    assert!(store.load().unwrap().library.is_empty());
    assert_eq!(
        fs::read(source.join("LJ/lua/local.lua")).unwrap(),
        b"return 'fixture'"
    );
    for bytes in [
        b"{}".as_slice(),
        br#"{"schemaVersion":99}"#,
        br#"{"schemaVersion":1,"path":"../../escape"}"#,
    ] {
        fs::write(source.join(MANIFEST), bytes).unwrap();
        let request = Uuid::new_v4().to_string();
        queue
            .import_local(&mut store, &request, source.to_str().unwrap())
            .unwrap();
        assert_eq!(
            wait(&mut queue, &mut store, &request).status,
            Status::Failed
        );
    }
}

#[test]
fn local_commit_failure_rolls_back_records_and_can_reuse_promoted_bytes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let data = root.path().join("data");
    fixture(&source, "fixture.local", true);
    let mut store = Storage::open(&data).unwrap();
    let mut queue = Packages::open(&mut store).unwrap();
    let connection = rusqlite::Connection::open(data.join("sqlite/state.db")).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_local BEFORE INSERT ON local_sources BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;").unwrap();
    let request = Uuid::new_v4().to_string();
    queue
        .import_local(&mut store, &request, source.to_str().unwrap())
        .unwrap();
    let op = wait(&mut queue, &mut store, &request);
    assert_eq!(op.status, Status::Failed);
    assert!(store.load().unwrap().library.is_empty());
    assert!(store.prepared_artifact(&op.hash).unwrap().is_none());
    assert!(store.local_sources().unwrap().is_empty());
    connection
        .execute_batch("DROP TRIGGER fail_local;")
        .unwrap();
    let reference = import(&mut queue, &mut store, &source);
    assert_eq!(reference.hash, op.hash);
    assert_eq!(
        fs::read(source.join("LJ/lua/local.lua")).unwrap(),
        b"return 'fixture'"
    );
}

#[cfg(windows)]
#[test]
fn local_junctions_and_locked_build_outputs_are_rejected_without_source_changes() {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::process::CommandExt;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let outside = root.path().join("outside");
    fixture(&source, "fixture.local", true);
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep.lua"), b"keep").unwrap();
    let junction = source.join("LJ").join("lua").join("redirect");
    assert!(
        std::process::Command::new("cmd")
            .creation_flags(0x08000000)
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut store = Storage::open(&root.path().join("data")).unwrap();
    let mut queue = Packages::open(&mut store).unwrap();
    let request = Uuid::new_v4().to_string();
    queue
        .import_local(&mut store, &request, source.to_str().unwrap())
        .unwrap();
    assert_eq!(
        wait(&mut queue, &mut store, &request).status,
        Status::Failed
    );
    fs::remove_dir(&junction).unwrap();
    let locked = OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(source.join("LJ/lua/local.lua"))
        .unwrap();
    let request = Uuid::new_v4().to_string();
    queue
        .import_local(&mut store, &request, source.to_str().unwrap())
        .unwrap();
    assert_eq!(
        wait(&mut queue, &mut store, &request).status,
        Status::Failed
    );
    drop(locked);
    assert!(store.load().unwrap().library.is_empty());
    assert_eq!(fs::read(outside.join("keep.lua")).unwrap(), b"keep");
    assert_eq!(
        fs::read(source.join("LJ/lua/local.lua")).unwrap(),
        b"return 'fixture'"
    );
}
