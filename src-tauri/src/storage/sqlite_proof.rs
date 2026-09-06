use super::conversion::{convert_locked, regular, snapshot};
use super::*;
use rusqlite::Connection as Sqlite;
use serde_json::Value as Json;
use std::{
    collections::BTreeMap,
    process::{Child, Command, Stdio},
    time::Instant,
};
type Snapshot = BTreeMap<String, Vec<Vec<Json>>>;
type Check<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn convert(source: &Path, destination: &Path, backup: bool, stop: Option<&str>) -> Check<()> {
    let source = source.canonicalize()?;
    if destination
        .parent()
        .ok_or("Missing parent")?
        .canonicalize()?
        .starts_with(&source)
    {
        return Err("Nested destination".into());
    }
    if backup {
        regular(&source.join("complete"))?;
    }
    let _guard = if backup { None } else { Some(lock(&source)?) };
    convert_locked(&source, destination, stop)
}
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, folder: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().into(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn worker(
    name: &str,
    source: &Path,
    destination: &Path,
    schema: i64,
    mode: &str,
    ready: &Path,
) -> Worker {
    let mut child = Worker(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--ignored", "--nocapture"])
            .env("STARFRAME_PROOF_SOURCE", source)
            .env("STARFRAME_PROOF_DESTINATION", destination)
            .env("STARFRAME_PROOF_SCHEMA", schema.to_string())
            .env("STARFRAME_PROOF_MODE", mode)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let until = Instant::now() + Duration::from_secs(30);
    while !ready.exists() && Instant::now() < until {
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!("{name} exited before ready: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(ready.exists(), "{name} did not become ready");
    child
}

fn fixture(root: &Path, schema: i64, populated: bool) {
    fixture_mode(root, schema, if populated { "populated" } else { "empty" });
}
fn fixture_mode(root: &Path, schema: i64, mode: &str) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/turso-0.7.2")
        .join(format!("{schema}-{mode}"));
    for (path, bytes) in tree(&source) {
        let target = root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
}

#[test]
#[ignore = "conversion process terminated at persisted boundaries by parent test"]
fn conversion_worker() {
    let source = PathBuf::from(std::env::var_os("STARFRAME_PROOF_SOURCE").unwrap());
    let destination = PathBuf::from(std::env::var_os("STARFRAME_PROOF_DESTINATION").unwrap());
    let phase = std::env::var("STARFRAME_PROOF_MODE").unwrap();
    convert(&source, &destination, false, Some(&phase)).unwrap();
}

fn expected(root: &Path) -> Snapshot {
    serde_json::from_slice(&fs::read(root.join("expected.json")).unwrap()).unwrap()
}

#[test]
fn legacy_schemas_wal_backups_and_empty_data_preserve_every_record() {
    for schema in 1..=3 {
        for populated in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("source");
            fixture(&source, schema, populated);
            let retained = tree(&source);
            for (backup, input) in [(false, source.clone()), (true, source.join("backup"))] {
                let destination = temp
                    .path()
                    .join(if backup { "restored" } else { "converted" });
                convert(&input, &destination, backup, None).unwrap();
                let conn = Sqlite::open(destination.join("state.db")).unwrap();
                assert_eq!(snapshot(&conn, 4).unwrap(), expected(&input));
                assert!(destination.join("complete").is_file());
                let sqlite_backup = temp.path().join(if backup {
                    "backup-restored.db"
                } else {
                    "backup-converted.db"
                });
                conn.backup("main", &sqlite_backup, None).unwrap();
                let restored = Sqlite::open(sqlite_backup).unwrap();
                assert_eq!(snapshot(&restored, 4).unwrap(), expected(&input));
                drop(restored);
                drop(conn);
                let completed = tree(&destination);
                assert!(convert(&input, &destination, backup, None).is_err());
                assert_eq!(tree(&destination), completed);
            }
            assert_eq!(tree(&source), retained);
        }
    }
}

#[test]
fn interrupted_copy_transaction_validation_and_promotion_retain_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fixture(&source, 3, true);
    let retained = tree(&source);
    for phase in [
        "copying",
        "copied",
        "transaction",
        "validated",
        "promoted",
        "complete",
    ] {
        let destination = temp.path().join(phase);
        let child = worker(
            "storage::sqlite_proof::conversion_worker",
            &source,
            &destination,
            3,
            phase,
            &destination.join("ready"),
        );
        drop(child);
        let interrupted = tree(&destination);
        assert!(convert(&source, &destination, false, None).is_err());
        assert_eq!(tree(&destination), interrupted);
        if ["promoted", "complete"].contains(&phase) {
            let conn = Sqlite::open(destination.join("state.db")).unwrap();
            assert_eq!(snapshot(&conn, 4).unwrap(), expected(&source));
        } else {
            assert!(!destination.join("state.db").exists());
        }
        let retry = temp.path().join(format!("{phase}-retry"));
        convert(&source, &retry, false, None).unwrap();
        assert_eq!(tree(&source), retained);
    }
}

#[test]
fn invalid_ownership_schema_records_and_incomplete_backups_fail_without_reset() {
    let temp = tempfile::tempdir().unwrap();
    for (name, sql) in [
        ("newer", "PRAGMA user_version=99"),
        ("foreign", "PRAGMA application_id=0"),
        (
            "wrong-engine",
            "PRAGMA ignore_check_constraints=ON; UPDATE metadata SET engine='other'",
        ),
        (
            "broken-order",
            "UPDATE collection_entries SET position=5 WHERE position=1",
        ),
        (
            "orphan",
            "PRAGMA foreign_keys=OFF; UPDATE preferences SET active_collection='missing'",
        ),
        (
            "invalid-path",
            "UPDATE game_selection SET path='relative/path'",
        ),
        (
            "negative-revision",
            "PRAGMA ignore_check_constraints=ON; UPDATE metadata SET revision=-1",
        ),
        ("missing-preferences", "DELETE FROM preferences"),
        ("unknown-table", "CREATE TABLE unknown(value TEXT)"),
        (
            "unknown-column",
            "ALTER TABLE library ADD COLUMN secret TEXT",
        ),
        (
            "unknown-trigger",
            "CREATE TRIGGER extra AFTER INSERT ON library BEGIN UPDATE metadata SET revision=0; END",
        ),
    ] {
        let source = temp.path().join(name);
        fixture(&source, 3, true);
        let conn = Sqlite::open(source.join("state.db")).unwrap();
        conn.execute_batch(sql).unwrap();
        drop(conn);
        let before = tree(&source);
        let destination = temp.path().join(format!("{name}-out"));
        let error = convert(&source, &destination, false, None)
            .unwrap_err()
            .to_string();
        let expected_error = match name {
            "newer" => "Unsupported legacy schema",
            "foreign" => "Unknown database owner",
            "wrong-engine" => "Unsupported engine marker",
            "broken-order" => "Invalid collection order",
            "orphan" => "FOREIGN KEY constraint failed",
            "invalid-path" => "saved game selection is invalid",
            "negative-revision" => "Invalid legacy database",
            "missing-preferences" => "Invalid preferences singleton",
            "unknown-table" => "Unexpected database tables",
            "unknown-column" => "Unexpected database columns",
            "unknown-trigger" => "Unexpected database objects",
            _ => unreachable!(),
        };
        assert!(error.contains(expected_error), "{name}: {error}");
        assert!(!destination.join("state.db").exists(), "{name}");
        assert_eq!(tree(&source), before, "{name}");
    }
    let corrupt = temp.path().join("corrupt");
    fixture(&corrupt, 3, true);
    fs::write(corrupt.join("state.db"), b"invalid header").unwrap();
    let before = tree(&corrupt);
    assert!(convert(&corrupt, &temp.path().join("corrupt-out"), false, None).is_err());
    assert_eq!(tree(&corrupt), before);
    let incomplete = temp.path().join("incomplete");
    fs::create_dir(&incomplete).unwrap();
    fs::write(incomplete.join("state.db"), b"retained partial backup").unwrap();
    assert!(convert(&incomplete, &temp.path().join("incomplete-out"), true, None).is_err());
    assert!(!temp.path().join("incomplete-out").exists());
}

#[test]
fn active_writer_and_nested_or_occupied_destination_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fixture(&source, 3, true);
    let guard = lock(&source).unwrap();
    assert!(convert(&source, &temp.path().join("busy"), false, None).is_err());
    assert!(!temp.path().join("busy").exists());
    drop(guard);
    let before = tree(&source);
    assert!(convert(&source, &source.join("nested"), false, None).is_err());
    let occupied = temp.path().join("occupied");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("notes"), b"Do not overwrite").unwrap();
    assert!(convert(&source, &occupied, false, None).is_err());
    assert_eq!(
        fs::read(occupied.join("notes")).unwrap(),
        b"Do not overwrite"
    );
    assert_eq!(tree(&source), before);
}

#[test]
fn unfinished_legacy_wal_does_not_become_committed_sqlite_data() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fixture_mode(&source, 3, "transaction");
    let before = tree(&source);
    let destination = temp.path().join("converted");
    convert(&source, &destination, false, None).unwrap();
    let conn = Sqlite::open(destination.join("state.db")).unwrap();
    assert_eq!(snapshot(&conn, 4).unwrap(), expected(&source));
    assert_eq!(tree(&source), before);
}

#[test]
fn production_startup_and_backup_restore_preserve_legacy_records_and_artifacts() {
    for schema in 1..=3 {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fixture(&source, schema, true);
        let before = tree(&source);
        let store = Storage::open(&source).unwrap();
        assert_eq!(snapshot(&store.conn, SCHEMA).unwrap(), expected(&source));
        assert_eq!(integer(&store.conn, "PRAGMA foreign_keys").unwrap(), 1);
        assert_eq!(integer(&store.conn, "PRAGMA synchronous").unwrap(), 2);
        assert_eq!(
            store
                .conn
                .query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        let backup = store.backup().unwrap();
        let restored = Storage::restore_into(&backup, &temp.path().join("restored")).unwrap();
        assert_eq!(snapshot(&restored.conn, SCHEMA).unwrap(), expected(&source));
        let legacy =
            Storage::restore_into(&source.join("backup"), &temp.path().join("legacy-restored"))
                .unwrap();
        assert_eq!(
            snapshot(&legacy.conn, SCHEMA).unwrap(),
            expected(&source.join("backup"))
        );
        drop(store);
        let restarted = Storage::open(&source).unwrap();
        assert_eq!(
            snapshot(&restarted.conn, SCHEMA).unwrap(),
            expected(&source)
        );
        drop(restarted);
        for (path, bytes) in before {
            assert_eq!(fs::read(source.join(path)).unwrap(), bytes);
        }
    }
}

#[test]
#[ignore = "production startup/backup worker terminated by parent test"]
fn production_worker() {
    let source = PathBuf::from(std::env::var_os("STARFRAME_PROOF_SOURCE").unwrap());
    let store = Storage::open(&source).unwrap();
    if std::env::var("STARFRAME_PROOF_MODE")
        .unwrap()
        .starts_with("backup-")
    {
        store.backup().unwrap();
    }
}

#[test]
fn interrupted_production_switch_and_backups_retry_without_replacing_originals() {
    for phase in [
        "copying",
        "copied",
        "transaction",
        "validated",
        "promoted",
        "complete",
        "before-switch",
        "after-switch",
        "backup-written",
        "backup-complete",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fixture(&source, 3, true);
        let before = tree(&source);
        let mut child = Worker(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "storage::sqlite_proof::production_worker",
                    "--ignored",
                    "--nocapture",
                ])
                .env("STARFRAME_PROOF_SOURCE", &source)
                .env("STARFRAME_PROOF_MODE", phase)
                .env("STARFRAME_CONVERSION_STOP", phase)
                .env("STARFRAME_CONVERSION_SIGNAL", source.join("ready"))
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        let until = Instant::now() + Duration::from_secs(30);
        while !source.join("ready").exists() && Instant::now() < until {
            if let Some(status) = child.0.try_wait().unwrap() {
                panic!("{phase}: exited before signal: {status}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(source.join("ready").exists(), "{phase}: missing signal");
        drop(child);
        let store = Storage::open(&source).unwrap();
        assert_eq!(
            snapshot(&store.conn, SCHEMA).unwrap(),
            expected(&source),
            "{phase}"
        );
        if phase.starts_with("backup-") {
            let backup = fs::read_dir(source.join("backups"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let result = Storage::restore_into(&backup, &temp.path().join("restored"));
            assert_eq!(result.is_ok(), phase == "backup-complete");
        }
        drop(store);
        for (path, bytes) in before {
            assert_eq!(fs::read(source.join(path)).unwrap(), bytes, "{phase}");
        }
    }
}

#[test]
fn incomplete_active_destination_never_falls_back_to_legacy_data() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fixture(&source, 3, true);
    fs::create_dir(source.join("sqlite")).unwrap();
    fs::write(
        source.join("sqlite/state.db"),
        b"retained incomplete SQLite",
    )
    .unwrap();
    let before = tree(&source);
    assert!(Storage::open(&source).is_err());
    assert_eq!(tree(&source), before);
}
