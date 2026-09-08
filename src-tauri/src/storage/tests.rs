use super::*;
use std::{
    io::Write,
    process::{Command, Stdio},
    time::Instant,
};

fn local(id: &str) -> ModReference {
    ModReference {
        mod_id: id.into(),
        hash: "ab".repeat(32),
        origin: Origin::LocalImport,
        release_id: None,
    }
}
fn entry() -> LibraryEntry {
    LibraryEntry {
        reference: local("fixture"),
        name: "Fixture mod".into(),
        author: "Fixture author".into(),
        version: "local build".into(),
    }
}

#[test]
fn catalog_migration_and_failed_replacement_retain_records() {
    use crate::catalog::{Catalog, refresh::Cache};
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    store.put_library_entry(&entry(), 0).unwrap();
    store
        .conn
        .execute_batch("DROP TABLE local_sources; DROP TABLE collection_imports; DROP TABLE pending_removals; DROP TABLE package_operations; DROP TABLE prepared_artifacts; DROP TABLE catalog_cache; PRAGMA user_version=5;")
        .unwrap();
    drop(store);
    let mut store = Storage::open(root.path()).unwrap();
    assert_eq!(integer(&store.conn, "PRAGMA user_version").unwrap(), SCHEMA);
    assert_eq!(store.load().unwrap().library, vec![entry()]);
    assert!(store.catalog_cache().unwrap().is_none());
    let cache = Cache {
        catalog: Some(
            Catalog::read(br#"{"schemaVersion":1,"catalogRevision":"1","mods":[]}"#).unwrap(),
        ),
        etag: Some("\"one\"".into()),
        last_success: Some(10),
        ..Default::default()
    };
    store.save_catalog_cache(&cache).unwrap();
    let backup = store.backup().unwrap();
    store.conn.execute_batch("CREATE TRIGGER fail_catalog BEFORE UPDATE ON catalog_cache BEGIN SELECT RAISE(ABORT, 'fixture write failure'); END;").unwrap();
    let mut replacement = cache.clone();
    replacement.catalog.as_mut().unwrap().catalog_revision = "2".into();
    replacement.etag = Some("\"two\"".into());
    assert!(store.save_catalog_cache(&replacement).is_err());
    assert_eq!(store.catalog_cache().unwrap().unwrap(), cache);
    store
        .conn
        .execute_batch("DROP TRIGGER fail_catalog;")
        .unwrap();
    store.save_catalog_cache(&replacement).unwrap();
    drop(store);
    let destination = root.path().join("restored");
    let store = Storage::restore_into(&backup, &destination).unwrap();
    assert_eq!(store.catalog_cache().unwrap().unwrap(), cache);
    assert_eq!(store.load().unwrap().library, vec![entry()]);
}

#[test]
fn game_selection_survives_migration_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    store.put_library_entry(&entry(), 0).unwrap();
    store
        .conn
        .execute_batch("DROP TABLE local_sources; DROP TABLE collection_imports; DROP TABLE pending_removals; DROP TABLE game_selection; DROP TABLE deployments; DROP TABLE deployment_blobs; DROP TABLE catalog_cache; DROP TABLE package_operations; DROP TABLE prepared_artifacts; PRAGMA user_version = 2;")
        .unwrap();
    drop(store);
    let mut store = Storage::open(root.path()).unwrap();
    assert!(store.selected_game().unwrap().is_none());
    let id = Uuid::new_v4().to_string();
    let path = root.path().join("game").to_str().unwrap().to_owned();
    assert_eq!(store.select_game(&id, &path).unwrap(), 2);
    assert!(store.select_game("invalid", "relative/path").is_err());
    drop(store);
    let store = Storage::open(root.path()).unwrap();
    assert_eq!(store.selected_game().unwrap(), Some((id, path)));
    assert_eq!(store.load().unwrap().library, vec![entry()]);
}

#[test]
fn records_survive_restart_with_order_origin_and_revisions() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    assert_eq!(integer(&store.conn, "PRAGMA foreign_keys").unwrap(), 1);
    assert_eq!(integer(&store.conn, "PRAGMA synchronous").unwrap(), 2);
    let id = Uuid::new_v4().to_string();
    assert_eq!(store.put_library_entry(&entry(), 0).unwrap(), 1);
    let catalog = ModReference {
        mod_id: "not-downloaded".into(),
        hash: "cd".repeat(32),
        origin: Origin::Catalog,
        release_id: Some("release-1".into()),
    };
    assert_eq!(
        store
            .save_collection(
                &id,
                "Fixture collection",
                &[catalog.clone(), local("fixture")],
                0
            )
            .unwrap(),
        1
    );
    store.set_active_collection(Some(&id), 2).unwrap();
    let before = store.load().unwrap();
    assert_eq!(before.revision, 3);
    assert!(
        !store
            .artifact_directory(&local("fixture"))
            .unwrap()
            .exists()
    );
    drop(store);
    let mut store = Storage::open(root.path()).unwrap();
    assert_eq!(store.load().unwrap(), before);
    assert!(matches!(
        store.save_collection(&id, "Old draft", &[], 0),
        Err(Error::Stale { current: 1 })
    ));
    assert!(
        store
            .save_collection(
                &id,
                "Duplicate mod",
                &[local("fixture"), local("fixture")],
                1
            )
            .is_err()
    );
    assert_eq!(store.load().unwrap(), before);
    assert!(store.set_active_collection(Some("missing"), 3).is_err());
    assert_eq!(store.load().unwrap(), before);
    assert!(
        store
            .conn
            .execute("INSERT INTO collections VALUES ('invalid', '', 0)", [])
            .is_err()
    );
    store
        .save_collection(&id, "Reordered", &[local("fixture"), catalog], 1)
        .unwrap();
    assert_eq!(
        store.load().unwrap().collections[0].entries[0],
        local("fixture")
    );
    let mut invalid = entry();
    invalid.reference.hash = "../outside".into();
    assert!(store.put_library_entry(&invalid, 4).is_err());
}

#[test]
fn migrations_back_up_and_failed_migrations_roll_back() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    store.put_library_entry(&entry(), 0).unwrap();
    store
        .conn
        .execute_batch(
            "DROP TABLE local_sources; DROP TABLE collection_imports; DROP TABLE pending_removals; DROP TABLE preferences; DROP TABLE game_selection; DROP TABLE deployments; DROP TABLE deployment_blobs; DROP TABLE catalog_cache; DROP TABLE package_operations; DROP TABLE prepared_artifacts; PRAGMA user_version = 1;",
        )
        .unwrap();
    let backup = store.backup().unwrap();
    assert!(
        store
            .apply_migration(
                2,
                "CREATE TABLE partial (id INTEGER); INSERT INTO missing_table VALUES (1);"
            )
            .is_err()
    );
    assert_eq!(integer(&store.conn, "PRAGMA user_version").unwrap(), 1);
    assert_eq!(
        integer(
            &store.conn,
            "SELECT count(*) FROM sqlite_schema WHERE name = 'partial'"
        )
        .unwrap(),
        0
    );
    drop(store);
    let store = Storage::open(root.path()).unwrap();
    assert_eq!(store.load().unwrap().library, vec![entry()]);
    assert_eq!(integer(&store.conn, "PRAGMA user_version").unwrap(), SCHEMA);
    assert_eq!(
        fs::read_dir(root.path().join("backups")).unwrap().count(),
        2
    );
    let restored = tempfile::tempdir().unwrap();
    let recovered = Storage::restore_into(&backup, restored.path()).unwrap();
    assert_eq!(recovered.load().unwrap(), store.load().unwrap());
    assert!(Storage::restore_into(&backup, root.path()).is_err());
    assert!(Storage::restore_into(root.path(), tempfile::tempdir().unwrap().path()).is_err());
}

#[test]
fn schema_ten_migration_and_backup_preserve_distinct_approvals_and_local_deduplication() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sqlite")).unwrap();
    fs::write(root.path().join("sqlite/complete"), []).unwrap();
    let conn = Connection::open(root.path().join("sqlite/state.db")).unwrap();
    for migration in &MIGRATIONS[..9] {
        conn.execute_batch(migration).unwrap();
    }
    conn.execute_batch(&format!(
        "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version=9;"
    ))
    .unwrap();
    conn.execute("INSERT INTO library VALUES ('fixture', ?, 'Fixture', 'Author', '1', 'catalog', 'fixture.1')", ["ab".repeat(32)]).unwrap();
    drop(conn);
    let mut store = Storage::open(root.path()).unwrap();
    let mut second = store.load().unwrap().library[0].clone();
    second.reference.release_id = Some("fixture.2".into());
    store
        .put_library_entry(&second, store.load().unwrap().revision)
        .unwrap();
    let mut local = second.clone();
    local.reference.origin = Origin::LocalImport;
    local.reference.release_id = None;
    for _ in 0..2 {
        store
            .put_library_entry(&local, store.load().unwrap().revision)
            .unwrap();
    }
    assert_eq!(store.load().unwrap().library.len(), 3);
    let backup = store.backup().unwrap();
    let target = tempfile::tempdir().unwrap();
    let restored = Storage::restore_into(&backup, target.path()).unwrap();
    assert_eq!(restored.load().unwrap(), store.load().unwrap());
    drop(store);
    assert_eq!(
        Storage::open(root.path()).unwrap().load().unwrap(),
        restored.load().unwrap()
    );
}

#[test]
fn corrupt_newer_and_foreign_databases_are_retained() {
    let root = tempfile::tempdir().unwrap();
    let corrupt = vec![0x5a; 8192];
    fs::create_dir(root.path().join("sqlite")).unwrap();
    File::create(root.path().join("sqlite/complete")).unwrap();
    fs::write(root.path().join("sqlite/state.db"), &corrupt).unwrap();
    assert!(Storage::open(root.path()).is_err());
    assert_eq!(
        fs::read(root.path().join("sqlite/state.db")).unwrap(),
        corrupt
    );
    let newer = tempfile::tempdir().unwrap();
    let store = Storage::open(newer.path()).unwrap();
    store.conn.execute("PRAGMA user_version = 99", []).unwrap();
    store.backup().unwrap();
    drop(store);
    let before = fs::read(newer.path().join("sqlite/state.db")).unwrap();
    assert!(Storage::open(newer.path()).is_err());
    assert_eq!(
        fs::read(newer.path().join("sqlite/state.db")).unwrap(),
        before
    );
    let foreign = tempfile::tempdir().unwrap();
    let conn = Connection::open(foreign.path().join("state.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE other_data (value TEXT); INSERT INTO other_data VALUES ('retain');",
    )
    .unwrap();
    drop(conn);
    assert!(Storage::open(foreign.path()).is_err());
}

#[test]
fn concurrent_owners_and_busy_writes_fail_without_losing_state() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    assert!(Storage::open(root.path()).is_err());
    let second = Connection::open(root.path().join("sqlite/state.db")).unwrap();
    second.busy_timeout(Duration::from_millis(50)).unwrap();
    store.conn.execute("BEGIN IMMEDIATE", []).unwrap();
    let start = Instant::now();
    assert!(second.execute("BEGIN IMMEDIATE", []).is_err());
    assert!(start.elapsed() < Duration::from_secs(2));
    store.conn.execute("ROLLBACK", []).unwrap();
    store.put_library_entry(&entry(), 0).unwrap();
    assert_eq!(store.load().unwrap().revision, 1);
}

#[test]
fn forced_termination_preserves_commits_and_rolls_back_unfinished_work() {
    for mode in ["transaction", "migration", "writer"] {
        let root = tempfile::tempdir().unwrap();
        let ready = root.path().join("ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::crash_worker",
                "--ignored",
                "--nocapture",
            ])
            .env("STARFRAME_TEST_ROOT", root.path())
            .env("STARFRAME_TEST_MODE", mode)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let until = Instant::now() + Duration::from_secs(15);
        while !ready.exists() && Instant::now() < until {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("storage worker exited before ready: {status}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !ready.exists() {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("storage worker did not become ready");
        }
        if mode == "writer" {
            let result = Storage::open(root.path());
            assert!(result.is_err());
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let recovered = Storage::open(root.path()).unwrap();
        let records = recovered.load().unwrap();
        assert_eq!(records.library, vec![entry()]);
        assert_eq!(records.revision, 1);
    }
}

#[test]
#[ignore = "spawned by the process-interruption test"]
fn crash_worker() {
    let root = PathBuf::from(std::env::var_os("STARFRAME_TEST_ROOT").unwrap());
    let mode = std::env::var("STARFRAME_TEST_MODE").unwrap();
    {
        let mut store = Storage::open(&root).unwrap();
        store.put_library_entry(&entry(), 0).unwrap();
        if mode == "migration" {
            store
                .conn
                .execute_batch("DROP TABLE local_sources; DROP TABLE collection_imports; DROP TABLE pending_removals; DROP TABLE preferences; DROP TABLE game_selection; DROP TABLE deployments; DROP TABLE deployment_blobs; DROP TABLE catalog_cache; DROP TABLE package_operations; DROP TABLE prepared_artifacts; PRAGMA user_version = 1;")
                .unwrap();
            store.backup().unwrap();
            store.conn.execute("BEGIN IMMEDIATE", []).unwrap();
            store.conn.execute_batch(MIGRATIONS[1]).unwrap();
            store.conn.execute("PRAGMA user_version = 2", []).unwrap();
        } else if mode == "transaction" {
            store.conn.execute("BEGIN IMMEDIATE", []).unwrap();
            store.conn.execute("DELETE FROM library", []).unwrap();
            store
                .conn
                .execute("UPDATE metadata SET revision = 2", [])
                .unwrap();
        }
        store.conn.cache_flush().unwrap();
        let mut ready = File::create(root.join("ready")).unwrap();
        ready
            .write_all(b"committed base; unfinished work ready")
            .unwrap();
        ready.sync_all().unwrap();
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
