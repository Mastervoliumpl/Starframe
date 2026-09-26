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
fn corrupt_native_reference_key_fails_load_backup_and_restart_without_reset() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    let reference = crate::references::Reference::try_from(&local("fixture")).unwrap();
    store
        .save_collection(&Uuid::new_v4().to_string(), "Fixture", &[reference], 0)
        .unwrap();
    store
        .conn
        .execute("UPDATE collection_entries SET runtime_key='different'", [])
        .unwrap();
    assert!(store.load().is_err());
    assert!(store.backup().is_err());
    drop(store);
    assert!(Storage::open(root.path()).is_err());
    assert!(root.path().join("sqlite/state.db").is_file());
}

#[test]
fn game_selection_survives_restart() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    store.put_library_entry(&entry(), 0).unwrap();
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
    let registry = crate::references::Reference::Registry(crate::registry::ExactReference {
        mod_id: crate::registry::ModId::try_from(7).unwrap(),
        sha256: crate::registry::Sha256::try_from("cd".repeat(32)).unwrap(),
        release_id: crate::registry::ReleaseId(Uuid::new_v4()),
    });
    let native_local = crate::references::Reference::try_from(&local("fixture")).unwrap();
    assert_eq!(
        store
            .save_collection(
                &id,
                "Fixture collection",
                &[registry.clone(), native_local.clone()],
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
                &[native_local.clone(), native_local.clone()],
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
        .save_collection(&id, "Reordered", &[native_local.clone(), registry], 1)
        .unwrap();
    assert_eq!(
        store.load().unwrap().collections[0].entries[0],
        native_local
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
    seed_previous_schema(&store);
    let backup = store.backup().unwrap();
    assert!(
        store
            .apply_migration(
                SCHEMA,
                "CREATE TABLE partial (id INTEGER); INSERT INTO missing_table VALUES (1);"
            )
            .is_err()
    );
    assert_eq!(integer(&store.conn, "PRAGMA user_version").unwrap(), 19);
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
            seed_previous_schema(&store);
            store.backup().unwrap();
            store.conn.execute("BEGIN IMMEDIATE", []).unwrap();
            store.conn.execute_batch(RETIRE_CATALOG).unwrap();
            store
                .conn
                .execute_batch(&format!("PRAGMA user_version={SCHEMA}"))
                .unwrap();
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

fn seed_previous_schema(store: &Storage) {
    store.conn.execute_batch("ALTER TABLE library RENAME TO current_library; CREATE TABLE library(mod_id TEXT, hash TEXT, name TEXT, author TEXT, version TEXT, origin TEXT, release_id TEXT); INSERT INTO library SELECT * FROM current_library; DROP TABLE current_library; CREATE TABLE catalog_cache(id INTEGER PRIMARY KEY,record TEXT); CREATE TABLE catalog_security(id INTEGER PRIMARY KEY,record TEXT); INSERT INTO catalog_cache VALUES(1,'{}'); INSERT INTO catalog_security VALUES(1,'{}'); PRAGMA user_version=19;").unwrap();
}

#[test]
fn retirement_preserves_current_records_and_files_without_mapping_obsolete_ids() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    store.put_library_entry(&entry(), 0).unwrap();
    let id = Uuid::new_v4().to_string();
    let reference = crate::references::Reference::try_from(&entry().reference).unwrap();
    store
        .save_collection(&id, "Current collection", &[reference], 0)
        .unwrap();
    store.set_active_collection(Some(&id), 2).unwrap();
    let before = store.load().unwrap();
    seed_previous_schema(&store);
    store
        .conn
        .execute(
            "INSERT INTO library VALUES ('obsolete',?,'Old','Fixture','1','catalog','old.1')",
            ["cd".repeat(32)],
        )
        .unwrap();
    store
        .conn
        .execute(
            "INSERT INTO registry_library VALUES (1,'11111111-1111-4111-8111-111111111111',?,NULL)",
            ["cd".repeat(32)],
        )
        .unwrap();
    store
        .conn
        .execute(
            "INSERT INTO registry_trust_streams VALUES ('security',7,'{}',X'010203')",
            [],
        )
        .unwrap();
    store.conn.execute("INSERT INTO registry_decisions VALUES (?,7,?)",rusqlite::params!["cd".repeat(32),serde_json::json!({"sha256":"cd".repeat(32),"revision":7,"status":"blocked","reason":"Retained fixture"}).to_string()]).unwrap();
    store.conn.execute("INSERT INTO registry_receipt_attempts VALUES ('11111111-1111-4111-8111-111111111111','22222222-2222-4222-8222-222222222222','{}')",[]).unwrap();
    store
        .conn
        .execute(
            "INSERT INTO deployments VALUES (?, '{}')",
            [root.path().join("fake-game").to_str().unwrap()],
        )
        .unwrap();
    for (index, kind, release) in [
        (1, "package", "old.1"),
        (2, "package", "local-import"),
        (
            3,
            "registry_install",
            "11111111-1111-4111-8111-111111111111",
        ),
    ] {
        let record = serde_json::json!({"kind":kind,"releaseId":release}).to_string();
        store
            .conn
            .execute(
                "INSERT INTO package_operations VALUES (?,?,?)",
                rusqlite::params![
                    format!("operation-{index}"),
                    format!("request-{index}"),
                    record
                ],
            )
            .unwrap();
    }
    let unchanged_tables = [
        "collections",
        "collection_entries",
        "collection_imports",
        "preferences",
        "registry_library",
        "registry_trust_streams",
        "registry_decisions",
        "registry_receipt_attempts",
        "deployments",
        "local_sources",
        "local_watches",
        "app_updates",
        "prepared_artifacts",
    ];
    let snapshot = |store: &Storage| -> Vec<Vec<Vec<rusqlite::types::Value>>> {
        unchanged_tables
            .iter()
            .map(|table| {
                let mut statement = store
                    .conn
                    .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                    .unwrap();
                let count = statement.column_count();
                statement
                    .query_map([], |row| {
                        (0..count)
                            .map(|index| row.get(index))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
            })
            .collect()
    };
    let retained = snapshot(&store);
    let source = root.path().join("original-source.lua");
    let settings = root.path().join("settings.json");
    fs::write(&source, b"original source").unwrap();
    fs::write(&settings, b"saved settings").unwrap();
    drop(store);
    let store = Storage::open(root.path()).unwrap();
    assert_eq!(store.load().unwrap(), before);
    assert_eq!(snapshot(&store), retained);
    assert_eq!(
        integer(&store.conn, "SELECT count(*) FROM package_operations").unwrap(),
        2
    );
    assert_eq!(
        integer(
            &store.conn,
            "SELECT count(*) FROM sqlite_schema WHERE name IN ('catalog_cache','catalog_security')"
        )
        .unwrap(),
        0
    );
    assert_eq!(fs::read(source).unwrap(), b"original source");
    assert_eq!(fs::read(settings).unwrap(), b"saved settings");
    let backup = fs::read_dir(root.path().join("backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved = Connection::open(backup.join("state.db")).unwrap();
    assert_eq!(integer(&saved, "PRAGMA user_version").unwrap(), 19);
    assert_eq!(integer(&saved, "SELECT count(*) FROM library").unwrap(), 2);
    let target = tempfile::tempdir().unwrap();
    let restored = Storage::restore_into(&backup, target.path()).unwrap();
    assert_eq!(restored.load().unwrap(), before);
    assert_eq!(snapshot(&restored), retained);
}

#[test]
fn obsolete_schema_is_rejected_without_reset_or_conversion() {
    let root = tempfile::tempdir().unwrap();
    let store = Storage::open(root.path()).unwrap();
    store.conn.execute_batch("PRAGMA user_version=18").unwrap();
    drop(store);
    let before = fs::read(root.path().join("sqlite/state.db")).unwrap();
    assert!(Storage::open(root.path()).is_err());
    assert_eq!(
        fs::read(root.path().join("sqlite/state.db")).unwrap(),
        before
    );
    assert!(!root.path().join("backups").exists());
}
