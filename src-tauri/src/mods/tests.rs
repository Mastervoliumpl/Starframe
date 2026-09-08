use super::*;
use crate::packages::{Operation, Prepared, PreparedFile, Status};
use sha2::{Digest, Sha256};
use std::fs;
use uuid::Uuid;

fn shared(name: &str, entries: &[ModReference]) -> String {
    serde_json::to_string(&crate::sharing::Portable {
        format: "starframe-collection".into(),
        schema_version: 1,
        name: name.into(),
        entries: entries.to_vec(),
    })
    .unwrap()
}

fn import(store: &mut Storage, text: &str) -> String {
    let id = Uuid::new_v4().to_string();
    crate::sharing::action(
        store,
        crate::sharing::Action::Accept {
            text: text.into(),
            request_id: id.clone(),
            expected_revision: store.load().unwrap().revision.to_string(),
        },
    )
    .unwrap();
    id
}

fn finish_imports(store: &mut Storage, queue: &mut packages::Packages) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        queue.poll(store).unwrap();
        crate::sharing::poll(store, queue).unwrap();
        if store.collection_imports().unwrap().iter().all(|i| {
            i.entries.iter().all(|e| {
                matches!(
                    e.status,
                    crate::sharing::Status::Ready | crate::sharing::Status::Unresolved
                )
            })
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "import did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn sharing_roundtrip_reuses_verified_files_in_a_second_library_and_blocks_partial_application() {
    let (_source_root, mut source, entries) = fixture();
    let references: Vec<_> = entries.iter().rev().map(|e| e.reference.clone()).collect();
    let source_id = Uuid::new_v4().to_string();
    source
        .save_collection(&source_id, "Shared order", &references, 0)
        .unwrap();
    let exported = crate::sharing::action(
        &mut source,
        crate::sharing::Action::Export { id: source_id },
    )
    .unwrap()
    .text
    .unwrap();
    let json: Value = serde_json::from_str(&exported).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 4);
    assert!(!exported.contains("example.invalid"));
    assert!(!exported.contains("sourcePath"));
    let (_target_root, mut target, _) = fixture();
    let mut queue = packages::Packages::open(&mut target).unwrap();
    let id = import(&mut target, &exported);
    assert!(target.load().unwrap().active_collection.is_none());
    target
        .set_active_collection(Some(&id), target.load().unwrap().revision)
        .unwrap();
    assert!(requested(&target).is_err());
    finish_imports(&mut target, &mut queue);
    assert!(
        target.collection_imports().unwrap()[0]
            .entries
            .iter()
            .all(|e| e.status == crate::sharing::Status::Ready)
    );
    assert_eq!(target.load().unwrap().collections[0].entries, references);
    let activation = requested(&target).unwrap();
    assert_eq!(activation["mods"][0]["modId"], "fixture.core");
    assert_eq!(target.load().unwrap().library.len(), 2);
    let reexported = crate::sharing::action(&mut target, crate::sharing::Action::Export { id })
        .unwrap()
        .text
        .unwrap();
    assert_eq!(exported, reexported);
}

#[test]
fn shared_references_keep_missing_withdrawn_changed_local_and_reapproved_identities_unresolved() {
    let (_root, mut store, entries) = fixture();
    let mut cache = store.catalog_cache().unwrap().unwrap();
    let catalog = cache.catalog.as_mut().unwrap();
    catalog.catalog_revision = "2".into();
    catalog.mods[0].releases[0].withdrawn = true;
    let mut reapproval = catalog.mods[1].releases[0].clone();
    reapproval.id = "fixture.addon.2".into();
    reapproval.requires.clear();
    catalog.mods[1].releases.push(reapproval);
    store.save_catalog_cache(&cache).unwrap();
    let mut cases = vec![entries[0].reference.clone()];
    let mut missing = entries[1].reference.clone();
    missing.release_id = Some("fixture.deleted.1".into());
    cases.push(missing);
    let mut changed = entries[1].reference.clone();
    changed.hash = "ff".repeat(32);
    cases.push(changed);
    let mut local = entries[1].reference.clone();
    local.origin = Origin::LocalImport;
    local.release_id = None;
    cases.push(local);

    for reference in cases {
        let before = store.load().unwrap();
        let text = shared("Unresolved", std::slice::from_ref(&reference));
        let reply = crate::sharing::action(
            &mut store,
            crate::sharing::Action::Review { text: text.clone() },
        )
        .unwrap();
        assert_eq!(reply.entries[0].status, crate::sharing::Status::Unresolved);
        assert_eq!(store.load().unwrap(), before);
        let id = import(&mut store, &text);
        assert_eq!(
            store
                .load()
                .unwrap()
                .collections
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .entries,
            vec![reference]
        );
    }
    assert_eq!(store.load().unwrap().library.len(), entries.len());
    assert!(
        entries
            .iter()
            .all(|e| store.load().unwrap().library.contains(e))
    );
}

#[test]
fn sharing_rejects_untrusted_fields_formats_paths_duplicates_and_oversize_documents_before_writing()
{
    let (_root, mut store, entries) = fixture();
    let valid: Value =
        serde_json::from_str(&shared("Fixture", &[entries[0].reference.clone()])).unwrap();
    let before = store.load().unwrap();
    let mut cases = vec![];
    for key in ["url", "settings", "sourcePath", "commands", "binary"] {
        let mut document = valid.clone();
        document["entries"][0][key] = json!("https://unapproved.invalid/archive.zip");
        cases.push(document);
    }
    let mut document = valid.clone();
    document["schemaVersion"] = json!(99);
    cases.push(document);
    let mut document = valid.clone();
    document["entries"][0]["modId"] = json!("C:\\private\\mod.dll");
    cases.push(document);
    let mut document = valid.clone();
    document["entries"] = json!([valid["entries"][0], valid["entries"][0]]);
    cases.push(document);
    let mut document = valid;
    document["settings"] = json!({});
    cases.push(document);
    for document in cases {
        assert!(
            crate::sharing::action(
                &mut store,
                crate::sharing::Action::Accept {
                    text: document.to_string(),
                    request_id: Uuid::new_v4().to_string(),
                    expected_revision: before.revision.to_string()
                }
            )
            .is_err()
        );
        assert_eq!(store.load().unwrap(), before);
    }
    assert!(crate::sharing::Portable::read(&" ".repeat(crate::sharing::MAX_BYTES + 1)).is_err());
}

#[test]
fn corrupt_cached_import_stays_unresolved_and_restart_retry_preserves_the_collection() {
    let (root, mut store, entries) = fixture();
    let text = shared("Retained", &[entries[0].reference.clone()]);
    let id = import(&mut store, &text);
    drop(store);
    let mut store = Storage::open(root.path()).unwrap();
    let mut queue = packages::Packages::open(&mut store).unwrap();
    assert_eq!(
        store.collection_imports().unwrap()[0].entries[0].status,
        crate::sharing::Status::Unresolved
    );
    crate::sharing::action(&mut store, crate::sharing::Action::Retry { id: id.clone() }).unwrap();
    fs::write(
        store
            .artifact_directory(&entries[0].reference)
            .unwrap()
            .join("package/Fixture.dll"),
        b"changed bytes",
    )
    .unwrap();
    finish_imports(&mut store, &mut queue);
    let imported = store.collection_imports().unwrap();
    assert_eq!(
        imported[0].entries[0].status,
        crate::sharing::Status::Unresolved
    );
    assert!(
        imported[0].entries[0].message.contains("changed")
            || imported[0].entries[0].message.contains("size")
    );
    assert_eq!(
        store.load().unwrap().collections[0].entries[0],
        entries[0].reference
    );
    store
        .delete_collection(&id, store.load().unwrap().revision)
        .unwrap();
    assert!(store.collection_imports().unwrap().is_empty());
    assert_eq!(store.load().unwrap().library.len(), 2);
}

#[test]
fn shared_local_content_is_verified_without_a_catalog_download() {
    let (_root, mut store, mut entries) = fixture();
    let entry = &mut entries[0];
    entry.reference.origin = Origin::LocalImport;
    entry.reference.release_id = None;
    store
        .put_library_entry(entry, store.load().unwrap().revision)
        .unwrap();
    let mut queue = packages::Packages::open(&mut store).unwrap();
    import(
        &mut store,
        &shared("Local bytes", std::slice::from_ref(&entry.reference)),
    );
    finish_imports(&mut store, &mut queue);
    assert_eq!(
        store.collection_imports().unwrap()[0].entries[0].status,
        crate::sharing::Status::Ready
    );
    assert!(store.load().unwrap().active_collection.is_none());
}

#[test]
fn accepted_import_is_idempotent_and_storage_failure_rolls_back_collection_and_progress() {
    let (root, mut store, entries) = fixture();
    let text = shared("Once", &[entries[0].reference.clone()]);
    let id = Uuid::new_v4().to_string();
    let expected = store.load().unwrap().revision.to_string();
    let accept = || crate::sharing::Action::Accept {
        text: text.clone(),
        request_id: id.clone(),
        expected_revision: expected.clone(),
    };
    crate::sharing::action(&mut store, accept()).unwrap();
    crate::sharing::action(&mut store, accept()).unwrap();
    assert_eq!(store.load().unwrap().collections.len(), 1);
    let connection = rusqlite::Connection::open(root.path().join("sqlite/state.db")).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON collection_imports BEGIN SELECT RAISE(ABORT, 'fixture storage failure'); END;").unwrap();
    let before = store.load().unwrap();
    assert!(
        crate::sharing::action(
            &mut store,
            crate::sharing::Action::Accept {
                text,
                request_id: Uuid::new_v4().to_string(),
                expected_revision: before.revision.to_string()
            }
        )
        .is_err()
    );
    assert_eq!(store.load().unwrap(), before);
}

#[test]
fn sharing_queues_missing_approved_content_after_one_acceptance_and_retains_cancelled_references() {
    let (_source, source, entries) = fixture();
    let target = tempfile::tempdir().unwrap();
    let mut store = Storage::open(target.path()).unwrap();
    store
        .save_catalog_cache(&source.catalog_cache().unwrap().unwrap())
        .unwrap();
    let mut queue = packages::Packages::open(&mut store).unwrap();
    let id = import(
        &mut store,
        &shared("Missing package", &[entries[0].reference.clone()]),
    );
    crate::sharing::poll(&mut store, &mut queue).unwrap();
    let operations = queue.operations(&store).unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].release_id, "fixture.core.1");
    assert_eq!(operations[0].hash, entries[0].reference.hash);
    queue.cancel(&mut store, &operations[0].id).unwrap();
    finish_imports(&mut store, &mut queue);
    assert_eq!(
        store.collection_imports().unwrap()[0].entries[0].status,
        crate::sharing::Status::Unresolved
    );
    assert!(store.load().unwrap().library.is_empty());
    assert_eq!(
        store.load().unwrap().collections[0].entries[0],
        entries[0].reference
    );
    store
        .set_active_collection(Some(&id), store.load().unwrap().revision)
        .unwrap();
    assert!(requested(&store).is_err());
}

#[test]
fn sharing_explains_missing_required_references_without_adding_them_to_the_shared_order() {
    let (_root, mut store, entries) = fixture();
    let reply = crate::sharing::action(
        &mut store,
        crate::sharing::Action::Review {
            text: shared("Missing dependency", &[entries[1].reference.clone()]),
        },
    )
    .unwrap();
    assert_eq!(reply.entries.len(), 1);
    assert_eq!(reply.entries[0].status, crate::sharing::Status::Unresolved);
    assert!(reply.entries[0].message.contains("fixture.core.1"));
    assert!(reply.order_error.is_some());
}

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
            reference: entry.reference.clone(),
            enabled,
            expected_revision,
        },
    )
    .unwrap()
}

#[test]
fn reapproved_bytes_preserve_both_release_references_and_satisfy_new_dependencies() {
    let (root, mut store, entries) = fixture();
    let old = entries[0].clone();
    let mut new = old.clone();
    new.reference.release_id = Some("fixture.core.2".into());
    let pinned = Uuid::new_v4().to_string();
    store
        .save_collection(
            &pinned,
            "Old approval",
            std::slice::from_ref(&old.reference),
            0,
        )
        .unwrap();
    let mut cache = store.catalog_cache().unwrap().unwrap();
    let catalog = cache.catalog.as_mut().unwrap();
    let mut release = catalog.mods[0].releases[0].clone();
    catalog.catalog_revision = "2".into();
    release.id = new.reference.release_id.clone().unwrap();
    catalog.mods[0].releases[0].withdrawn = true;
    catalog.mods[0].releases.push(release);
    let mut addon_release = catalog.mods[1].releases[0].clone();
    addon_release.id = "fixture.addon.2".into();
    addon_release.requires = vec!["fixture.core.2".into()];
    catalog.mods[1].releases.push(addon_release);
    store.save_catalog_cache(&cache).unwrap();
    let operation = Operation {
        id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        release_id: "fixture.core.2".into(),
        hash: new.reference.hash.clone(),
        status: Status::Completed,
        message: "Reapproved".into(),
        received_bytes: 10,
        total_bytes: 10,
    };
    store.save_package(&operation).unwrap();
    let prepared = store
        .prepared_artifact(&new.reference.hash)
        .unwrap()
        .unwrap();
    store.complete_package(&operation, &new, &prepared).unwrap();
    assert_eq!(store.load().unwrap().library.len(), 3);
    let mut addon = entries[1].clone();
    addon.reference.release_id = Some("fixture.addon.2".into());
    store
        .put_library_entry(&addon, store.load().unwrap().revision)
        .unwrap();
    let view = enable(&mut store, &addon, true);
    assert!(view.enabled.contains(&new.reference));
    assert!(!view.enabled.contains(&old.reference));
    requested(&store).unwrap();
    drop(store);
    let mut store = Storage::open(root.path()).unwrap();
    assert!(store.load().unwrap().library.contains(&old));
    assert!(store.load().unwrap().library.contains(&new));
    let exported =
        crate::sharing::action(&mut store, crate::sharing::Action::Export { id: pinned })
            .unwrap()
            .text
            .unwrap();
    let document: crate::sharing::Portable = serde_json::from_str(&exported).unwrap();
    assert_eq!(document.entries, vec![old.reference.clone()]);
    store
        .uninstall_mod(&old.reference, store.load().unwrap().revision, true)
        .unwrap();
    cleanup(&mut store).unwrap();
    assert!(store.artifact_directory(&new.reference).unwrap().exists());
    assert!(store.load().unwrap().library.contains(&new));
    assert!(
        super::view(&store)
            .unwrap()
            .enabled
            .contains(&new.reference)
    );
    assert_eq!(
        store
            .prepared_artifact(&new.reference.hash)
            .unwrap()
            .unwrap(),
        prepared
    );
}

#[test]
fn large_disabled_libraries_keep_bounded_inventory_and_active_mods_across_restart() {
    for count in [255, 256, 257] {
        let (root, mut store, entries) = fixture();
        for i in 2..count {
            let mut entry = entries[0].clone();
            entry.reference.mod_id = format!("aaa.disabled{i}");
            entry.reference.origin = Origin::LocalImport;
            entry.reference.release_id = None;
            store
                .put_library_entry(&entry, store.load().unwrap().revision)
                .unwrap();
        }
        fs::write(root.path().join("settings.cfg"), b"retain").unwrap();
        for active in [false, true] {
            if active {
                enable(&mut store, &entries[1], true);
            }
            let activation = requested(&store).unwrap();
            assert_eq!(
                activation["installedMods"].as_array().unwrap().len(),
                count.min(256)
            );
            assert_eq!(activation["omittedDisabledMods"], count.saturating_sub(256));
            assert_eq!(
                activation["mods"].as_array().unwrap().len(),
                if active { 2 } else { 0 }
            );
            if active {
                assert_eq!(
                    activation["installedMods"][0]["modId"],
                    entries[0].reference.mod_id
                );
                payload(&store, &activation).unwrap();
            }
            drop(store);
            store = Storage::open(root.path()).unwrap();
            assert_eq!(requested(&store).unwrap(), activation);
            assert_eq!(store.load().unwrap().library.len(), count);
            assert_eq!(
                fs::read(root.path().join("settings.cfg")).unwrap(),
                b"retain"
            );
        }
        if count == 257 {
            let records = store.load().unwrap();
            let all: Vec<_> = records
                .library
                .iter()
                .map(|e| e.reference.clone())
                .collect();
            assert!(store.set_mod_membership(&all, records.revision).is_err());
            assert_eq!(store.load().unwrap(), records);
        }
        store
            .set_mod_membership(&[], store.load().unwrap().revision)
            .unwrap();
        assert_eq!(requested(&store).unwrap()["mods"], json!([]));
    }
}

#[test]
fn runtime_file_boundaries_hold_from_commit_through_membership_and_activation() {
    for lua in [false, true] {
        for count in [1024, 1025] {
            let (root, mut store, entries) = fixture_with_lua(lua);
            let entry = &entries[0];
            let mut prepared = store
                .prepared_artifact(&entry.reference.hash)
                .unwrap()
                .unwrap();
            for i in 1..count {
                let path = if lua {
                    format!("LJ/lua/extra{i}.lua")
                } else {
                    format!("package/extra{i}.txt")
                };
                fs::write(
                    store
                        .artifact_directory(&entry.reference)
                        .unwrap()
                        .join(&path),
                    b"fixture",
                )
                .unwrap();
                prepared.files.push(PreparedFile {
                    path,
                    sha256: format!("{:x}", Sha256::digest(b"fixture")),
                    size_bytes: 7,
                });
            }
            let connection =
                rusqlite::Connection::open(root.path().join("sqlite/state.db")).unwrap();
            connection
                .execute(
                    "UPDATE prepared_artifacts SET record=? WHERE hash=?",
                    rusqlite::params![serde_json::to_string(&prepared).unwrap(), prepared.hash],
                )
                .unwrap();
            drop(connection);
            let before = store.load().unwrap();
            let result = action(
                &mut store,
                Action::SetEnabled {
                    reference: entry.reference.clone(),
                    enabled: true,
                    expected_revision: before.revision.to_string(),
                },
            );
            if count == 1024 {
                result.unwrap();
                let activation = requested(&store).unwrap();
                assert_eq!(
                    activation["mods"][0]["files"].as_array().unwrap().len(),
                    count
                );
                assert_eq!(payload(&store, &activation).unwrap().len(), count);
            } else {
                assert!(result.err().unwrap().contains("1,024"));
                assert_eq!(store.load().unwrap(), before);
                assert!(
                    packages::verify_artifact(&store, &entry.reference)
                        .unwrap_err()
                        .contains("1,024")
                );
                let operation = Operation {
                    id: Uuid::new_v4().to_string(),
                    request_id: Uuid::new_v4().to_string(),
                    release_id: entry.reference.release_id.clone().unwrap(),
                    hash: entry.reference.hash.clone(),
                    status: Status::Completed,
                    message: "Fixture".into(),
                    received_bytes: 10,
                    total_bytes: 10,
                };
                store.save_package(&operation).unwrap();
                assert!(
                    store
                        .complete_package(&operation, entry, &prepared)
                        .unwrap_err()
                        .to_string()
                        .contains("1,024")
                );
                assert_eq!(store.load().unwrap(), before);
                store
                    .set_mod_membership(std::slice::from_ref(&entry.reference), before.revision)
                    .unwrap();
                assert!(requested(&store).unwrap_err().contains("1,024"));
            }
            drop(store);
            let store = Storage::open(root.path()).unwrap();
            assert!(super::view(&store).unwrap().library.contains(entry));
            assert_eq!(
                store
                    .prepared_artifact(&entry.reference.hash)
                    .unwrap()
                    .unwrap(),
                prepared
            );
        }
    }
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
            reference: entries[0].reference.clone(),
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
                reference: entries[0].reference.clone(),
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
            .uninstall_mod(&entries[0].reference, revision, false)
            .unwrap_err()
            .to_string()
            .contains("Other setup")
    );
    assert_eq!(store.load().unwrap().revision, revision);
    store
        .uninstall_mod(&entries[0].reference, revision, true)
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
            .uninstall_mod(&entries[0].reference, before.revision, true)
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
        .uninstall_mod(&entries[0].reference, store.load().unwrap().revision, false)
        .unwrap();
    cleanup(&mut store).unwrap();
    assert!(!super::view(&store).unwrap().cleanup_errors.is_empty());
    drop(store);
    drop(lock);
    let mut store = Storage::open(temp.path()).unwrap();
    cleanup(&mut store).unwrap();
    assert!(super::view(&store).unwrap().cleanup_errors.is_empty());
}
