use super::*;
use crate::{
    deployment::{Engine, apply, apply_sources, hash, load},
    packages::{Kind, Operation, Status},
    registry::{ModId, ReleaseId},
    storage::RegistryDisplay,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::signature::Ed25519KeyPair;
use serde_json::Value;
use std::{fs, io::Write, sync::Arc};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

fn install(store: &mut Storage, ai: bool, id: u64) -> ExactReference {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let files = if ai {
        vec![
            ("AI/Example/main.lua", b"return {}".as_slice()),
            ("AI/Example/formers/rush.lua", b"return {}"),
        ]
    } else {
        vec![
            ("Maps/Example/Example.sanmap", b"inert map".as_slice()),
            ("Maps/Example/Textures/height.png", b"inert asset"),
        ]
    };
    for (path, bytes) in files {
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    let archive = zip.finish().unwrap().into_inner();
    let keys: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/registry-keys-v1.json"
    ))
    .unwrap();
    let plans: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/registry-installation-v1.json"
    ))
    .unwrap();
    let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
    let root: [u8; 32] = STANDARD
        .decode(keys["rootPublicKey"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    store
        .accept_registry_keys(&serde_json::to_vec(&keys["envelope"]).unwrap(), &root, now)
        .unwrap();
    store
        .accept_registry_security(
            &serde_json::to_vec(&keys["securityEnvelope"]).unwrap(),
            &root,
            now,
        )
        .unwrap();
    let mut envelope = plans["releases"][if ai { 3 } else { 2 }]["envelope"].clone();
    let mod_id = ModId::try_from(id).unwrap();
    let release_id = ReleaseId(Uuid::new_v4());
    envelope["signed"]["release"]["modId"] = id.into();
    envelope["signed"]["release"]["releaseId"] = release_id.0.to_string().into();
    envelope["signed"]["release"]["artifact"]["sha256"] = hash(&archive).into();
    envelope["signed"]["release"]["artifact"]["bytes"] = (archive.len() as u64).into();
    let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
    envelope["signatures"][0]["signature"] = STANDARD
        .encode(
            online
                .sign(&serde_json::to_vec(&envelope["signed"]).unwrap())
                .as_ref(),
        )
        .into();
    let release = store
        .accept_registry_release(
            &serde_json::to_vec(&envelope).unwrap(),
            &root,
            mod_id,
            release_id,
            now,
        )
        .unwrap();
    let identity = release.download_identity().unwrap();
    fs::create_dir_all(store.package_root().join("registry-archives")).unwrap();
    fs::write(
        store
            .package_root()
            .join("registry-archives")
            .join(format!("{}.zip", hash(&archive))),
        archive,
    )
    .unwrap();
    let prepared = packages::prepare_registry_archive(
        store.package_root(),
        Uuid::new_v4(),
        &release,
        Arc::default(),
    )
    .unwrap();
    let mut operation = Operation {
        id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        release_id: release_id.0.to_string(),
        hash: prepared.hash.clone(),
        kind: Kind::RegistryInstall,
        receipt_id: None,
        status: Status::Preparing,
        message: "Fixture installation".into(),
        received_bytes: identity.bytes,
        total_bytes: identity.bytes,
    };
    store.save_package(&operation).unwrap();
    operation.status = Status::Completed;
    store
        .complete_registry_install(
            &root,
            &identity,
            &RegistryDisplay {
                name: "Fixture content".into(),
                author: String::new(),
            },
            &prepared,
            &operation,
            now,
        )
        .unwrap()
        .reference
}

#[test]
fn declared_content_uses_shared_owner_rollback_and_removal_without_touching_originals() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Storage::open(&temp.path().join("data")).unwrap();
    let map = install(&mut store, false, 1);
    let ai = install(&mut store, true, 2);
    let root = temp.path().join("engine");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("original.game"), b"original").unwrap();
    let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
    let root = engine.root.clone();
    let base = || {
        (
            "Starframe/runtime.dll".into(),
            Source::Bytes(b"inert runtime".to_vec()),
        )
    };
    let mut sources = payload(&store, &[map.clone(), ai.clone()]).unwrap();
    sources.push(base());
    apply_sources(
        &mut store,
        &mut engine,
        sources,
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        fs::read(root.join("Sanctuary_Data/Maps/Example/Example.sanmap")).unwrap(),
        b"inert map"
    );
    assert_eq!(
        fs::read(root.join("Sanctuary_Data/Maps/Example/Textures/height.png")).unwrap(),
        b"inert asset"
    );
    assert_eq!(
        fs::read(root.join("LJ/lua/AI/mods/Example/formers/rush.lua")).unwrap(),
        b"return {}"
    );
    assert!(!root.join("engine").exists());
    assert_eq!(load(&store, &root).unwrap().owned.len(), 5);
    let mut failed = false;
    let error = apply_sources(
        &mut store,
        &mut engine,
        vec![base()],
        &mut || Ok(()),
        &mut |step| {
            if step == "file-changed" && !failed {
                failed = true;
                return Err("Injected content removal failure".into());
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("Previous deployment restored"));
    assert_eq!(load(&store, &root).unwrap().owned.len(), 5);
    assert!(load(&store, &root).unwrap().pending.is_none());
    apply_sources(
        &mut store,
        &mut engine,
        vec![base()],
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(
        !root
            .join("Sanctuary_Data/Maps/Example/Example.sanmap")
            .exists()
    );
    assert!(!root.join("LJ/lua/AI/mods/Example/main.lua").exists());
    assert_eq!(
        fs::read(root.join("Starframe/runtime.dll")).unwrap(),
        b"inert runtime"
    );
    assert_eq!(fs::read(root.join("original.game")).unwrap(), b"original");
    assert!(
        store
            .package_root()
            .join("artifacts")
            .join(map.sha256.as_str())
            .exists()
    );
    apply(&mut store, &mut engine, vec![], &mut || Ok(()), &mut |_| {
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read(root.join("original.game")).unwrap(), b"original");
}

#[test]
fn content_conflicts_changed_sources_and_changed_owned_files_are_retained() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Storage::open(&temp.path().join("data")).unwrap();
    let map = install(&mut store, false, 1);
    let other = install(&mut store, false, 2);
    assert!(
        payload(&store, &[map.clone(), other])
            .err()
            .unwrap()
            .contains("conflicting")
    );
    assert!(
        payload(&store, &[map.clone(), map.clone()])
            .err()
            .unwrap()
            .contains("more than one")
    );
    let mut missing = map.clone();
    missing.release_id = ReleaseId(Uuid::new_v4());
    assert!(
        payload(&store, &[missing])
            .err()
            .unwrap()
            .contains("not installed")
    );
    let root = temp.path().join("engine");
    fs::create_dir_all(root.join("Sanctuary_Data/Maps/Example")).unwrap();
    let target = root.join("Sanctuary_Data/Maps/Example/Example.sanmap");
    fs::write(&target, b"foreign map").unwrap();
    let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
    let sources = payload(&store, std::slice::from_ref(&map)).unwrap();
    assert!(
        apply_sources(
            &mut store,
            &mut engine,
            sources,
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .unwrap_err()
        .contains("Unowned")
    );
    assert_eq!(fs::read(&target).unwrap(), b"foreign map");
    fs::remove_file(&target).unwrap();
    let sources = payload(&store, std::slice::from_ref(&map)).unwrap();
    apply_sources(
        &mut store,
        &mut engine,
        sources,
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    fs::write(&target, b"user edit").unwrap();
    assert!(
        apply(
            &mut store,
            &mut engine,
            vec![],
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .is_err()
    );
    assert_eq!(fs::read(&target).unwrap(), b"user edit");
    let source = store
        .package_root()
        .join("artifacts")
        .join(map.sha256.as_str())
        .join("Maps/Example/Example.sanmap");
    fs::write(source, b"changed source").unwrap();
    assert!(payload(&store, &[map]).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"user edit");
}

#[test]
fn interrupted_content_removal_recovers_from_retained_bytes_after_restart() {
    use std::cell::Cell;
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let mut store = Storage::open(&data).unwrap();
    let ai = install(&mut store, true, 1);
    let root = temp.path().join("engine");
    fs::create_dir(&root).unwrap();
    let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
    let sources = payload(&store, std::slice::from_ref(&ai)).unwrap();
    apply_sources(
        &mut store,
        &mut engine,
        sources,
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    let stopped = Cell::new(false);
    let error = apply_sources(
        &mut store,
        &mut engine,
        vec![],
        &mut || {
            if stopped.get() {
                Err("Injected stopping boundary".into())
            } else {
                Ok(())
            }
        },
        &mut |step| {
            if step == "file-changed" {
                stopped.set(true);
                return Err("Injected interruption".into());
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("Recovery remains pending"));
    assert!(load(&store, &engine.root).unwrap().pending.is_some());
    drop(engine);
    drop(store);
    let mut store = Storage::open(&data).unwrap();
    let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
    let mut record = load(&store, &engine.root).unwrap();
    super::super::rollback(
        &mut store,
        &mut engine,
        &mut record,
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(load(&store, &engine.root).unwrap().pending.is_none());
    assert_eq!(
        fs::read(root.join("LJ/lua/AI/mods/Example/main.lua")).unwrap(),
        b"return {}"
    );
    assert_eq!(
        fs::read(root.join("LJ/lua/AI/mods/Example/formers/rush.lua")).unwrap(),
        b"return {}"
    );
    assert_eq!(
        store.installed_registry_releases().unwrap()[0].reference,
        ai
    );
}
