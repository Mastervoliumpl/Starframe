use super::*;
use rusqlite::{Connection as Sqlite, params_from_iter, types::Value as SqlValue};
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeMap,
    process::{Child, Command, Stdio},
    time::Instant,
};

type Check<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type Snapshot = BTreeMap<String, Vec<Vec<Json>>>;
const COLLECTION: &str = "11111111-1111-4111-8111-111111111111";
const GAME: &str = "22222222-2222-4222-8222-222222222222";
const TABLES: &[(i64, &str, &str, &str)] = &[
    (1, "metadata", "id, revision", "id"),
    (
        1,
        "library",
        "mod_id, hash, name, author, version, origin, release_id",
        "mod_id, hash",
    ),
    (1, "collections", "id, name, revision", "id"),
    (
        1,
        "collection_entries",
        "collection_id, position, mod_id, hash, origin, release_id",
        "collection_id, position",
    ),
    (2, "preferences", "id, active_collection", "id"),
    (
        3,
        "game_selection",
        "singleton, installation_id, path",
        "singleton",
    ),
];

fn require(condition: bool, message: &str) -> Check<()> {
    if !condition {
        return Err(message.into());
    }
    Ok(())
}

fn sqlite_rows(conn: &Sqlite, sql: &str) -> Check<Vec<Vec<Json>>> {
    let mut statement = conn.prepare(sql)?;
    let columns = statement.column_count();
    let rows = statement.query_map([], |row| {
        (0..columns)
            .map(|i| {
                Ok(match row.get::<_, SqlValue>(i)? {
                    SqlValue::Null => Json::Null,
                    SqlValue::Integer(n) => json!(n),
                    SqlValue::Text(s) => json!(s),
                    _ => return Err(rusqlite::Error::InvalidQuery),
                })
            })
            .collect::<rusqlite::Result<Vec<_>>>()
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn snapshot(conn: &Sqlite, schema: i64) -> Check<Snapshot> {
    TABLES
        .iter()
        .map(|&(since, table, columns, order)| {
            let rows = if schema >= since {
                sqlite_rows(
                    conn,
                    &format!("SELECT {columns} FROM {table} ORDER BY {order}"),
                )?
            } else if table == "preferences" {
                vec![vec![json!(1), Json::Null]]
            } else {
                vec![]
            };
            Ok((table.to_owned(), rows))
        })
        .collect()
}

async fn legacy_snapshot(conn: &Connection, schema: i64) -> Check<Snapshot> {
    let mut result = Snapshot::new();
    for &(since, table, columns, order) in TABLES {
        let mut values = vec![];
        if schema >= since {
            let mut rows = conn
                .query(
                    format!("SELECT {columns} FROM {table} ORDER BY {order}"),
                    (),
                )
                .await?;
            while let Some(row) = rows.next().await? {
                let mut fields = vec![];
                for i in 0..row.column_count() {
                    fields.push(match row.get_value(i)? {
                        turso::Value::Null => Json::Null,
                        turso::Value::Integer(n) => json!(n),
                        turso::Value::Text(s) => json!(s),
                        _ => return Err("Unexpected legacy value type".into()),
                    });
                }
                values.push(fields);
            }
        } else if table == "preferences" {
            values.push(vec![json!(1), Json::Null]);
        }
        result.insert(table.into(), values);
    }
    Ok(result)
}

fn text(row: &[Json], index: usize) -> Check<&str> {
    row[index]
        .as_str()
        .ok_or_else(|| "Expected text record".into())
}

fn validate_records(conn: &Sqlite) -> Check<()> {
    require(
        sqlite_rows(conn, "PRAGMA integrity_check")? == vec![vec![json!("ok")]],
        "Failed integrity check",
    )?;
    require(
        sqlite_rows(conn, "PRAGMA foreign_key_check")?.is_empty(),
        "Broken foreign key",
    )?;
    for row in sqlite_rows(
        conn,
        "SELECT mod_id, hash, origin, release_id FROM library UNION ALL SELECT mod_id, hash, origin, release_id FROM collection_entries",
    )? {
        let reference = ModReference {
            mod_id: text(&row, 0)?.into(),
            hash: text(&row, 1)?.into(),
            origin: Origin::parse(text(&row, 2)?.into())?,
            release_id: if row[3].is_null() {
                None
            } else {
                Some(text(&row, 3)?.into())
            },
        };
        validate_reference(&reference)?;
    }
    for row in sqlite_rows(conn, "SELECT name, author, version FROM library")? {
        validate_text(text(&row, 0)?, "Mod name")?;
        require(text(&row, 1)?.chars().count() <= 200, "Author too long")?;
        validate_text(text(&row, 2)?, "Version")?;
    }
    for row in sqlite_rows(conn, "SELECT id, name FROM collections")? {
        Uuid::parse_str(text(&row, 0)?)?;
        validate_text(text(&row, 1)?, "Collection name")?;
    }
    require(sqlite_rows(conn, "SELECT collection_id FROM collection_entries GROUP BY collection_id HAVING min(position) != 0 OR max(position) != count(*) - 1 OR count(*) > 10000")?.is_empty(), "Invalid collection order")?;
    for row in sqlite_rows(conn, "SELECT installation_id, path FROM game_selection")? {
        validate_game_selection(text(&row, 0)?, text(&row, 1)?)?;
    }
    Ok(())
}

fn regular(path: &Path) -> Check<()> {
    let metadata = fs::symlink_metadata(path)?;
    require(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Expected an ordinary file",
    )?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        require(
            metadata.file_attributes() & 0x400 == 0,
            "Reparse-point files are not supported",
        )?;
    }
    Ok(())
}

fn pause(phase: &str, stop: Option<&str>, destination: &Path) -> Check<()> {
    if stop == Some(phase) {
        File::create(destination.join("ready"))?.sync_all()?;
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

// Test-only conversion proof. The app does not call this until issue #40 ports it.
fn convert(source: &Path, destination: &Path, backup: bool, stop: Option<&str>) -> Check<()> {
    let source = source.canonicalize()?;
    let parent = destination
        .parent()
        .ok_or("Missing destination parent")?
        .canonicalize()?;
    require(
        !parent.starts_with(&source),
        "Destination must be outside the source",
    )?;
    if backup {
        regular(&source.join("complete"))?;
    }
    let _guard = if backup {
        None
    } else {
        regular(&source.join("state.lock"))?;
        let guard = OpenOptions::new()
            .read(true)
            .write(true)
            .open(source.join("state.lock"))?;
        guard.try_lock()?;
        Some(guard)
    };
    regular(&source.join("state.db"))?;
    fs::create_dir(destination)?;
    let staging = destination.join("staging");
    fs::create_dir(&staging)?;
    for name in ["state.db", "state.db-wal"] {
        if name == "state.db" || source.join(name).exists() {
            regular(&source.join(name))?;
            let mut reader = File::open(source.join(name))?;
            let mut writer = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(staging.join(name))?;
            std::io::copy(&mut reader, &mut writer)?;
            writer.sync_all()?;
            if name == "state.db" {
                pause("copying", stop, destination)?;
            }
        }
    }
    pause("copied", stop, destination)?;
    let copy = Sqlite::open(staging.join("state.db"))?;
    copy.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")?;
    require(
        copy.query_row("PRAGMA application_id", [], |r| r.get::<_, i64>(0))? == APPLICATION_ID,
        "Unknown database owner",
    )?;
    let schema: i64 = copy.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    require((1..=3).contains(&schema), "Unsupported legacy schema")?;
    require(
        sqlite_rows(&copy, "SELECT engine FROM metadata ORDER BY id")?
            == vec![vec![json!("turso")]],
        "Unsupported engine marker",
    )?;
    require(
        sqlite_rows(&copy, "PRAGMA integrity_check")? == vec![vec![json!("ok")]],
        "Invalid legacy database",
    )?;
    require(
        sqlite_rows(
            &copy,
            "SELECT name FROM sqlite_schema WHERE type IN ('view', 'trigger')",
        )?
        .is_empty(),
        "Unexpected database objects",
    )?;
    let tables: Vec<_> = sqlite_rows(
        &copy,
        "SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name",
    )?
    .into_iter()
    .map(|r| r[0].clone())
    .collect();
    let mut expected: Vec<_> = TABLES
        .iter()
        .filter(|t| t.0 <= schema)
        .map(|t| json!(t.1))
        .collect();
    expected.sort_by_key(|v| v.as_str().unwrap().to_owned());
    require(tables == expected, "Unexpected database tables")?;
    for &(since, table, columns, _) in TABLES {
        if since > schema {
            continue;
        }
        let expected_columns = if table == "metadata" {
            "id, engine, revision"
        } else {
            columns
        };
        let expected: Vec<_> = expected_columns.split(", ").map(|s| json!(s)).collect();
        let actual: Vec<_> = sqlite_rows(&copy, &format!("PRAGMA table_info({table})"))?
            .into_iter()
            .map(|row| row[1].clone())
            .collect();
        require(actual == expected, "Unexpected database columns")?;
    }
    let before = snapshot(&copy, schema)?;
    require(
        before["metadata"].len() == 1 && before["metadata"][0][0] == json!(1),
        "Invalid metadata singleton",
    )?;
    require(
        before["preferences"].len() == 1 && before["preferences"][0][0] == json!(1),
        "Invalid preferences singleton",
    )?;
    drop(copy);

    let candidate_path = staging.join("candidate.db");
    let mut candidate = Sqlite::open(&candidate_path)?;
    candidate.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE;",
    )?;
    let tx = candidate.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    for migration in MIGRATIONS {
        tx.execute_batch(&migration.replace("'turso'", "'sqlite'"))?;
    }
    for &(_, table, columns, _) in TABLES {
        if table == "metadata" {
            let revision = before[table][0][1].as_i64().ok_or("Invalid revision")?;
            tx.execute("UPDATE metadata SET revision=? WHERE id=1", [revision])?;
            continue;
        }
        if table == "preferences" {
            tx.execute("DELETE FROM preferences", [])?;
        }
        for row in &before[table] {
            let values: Vec<SqlValue> = row
                .iter()
                .map(|v| match v {
                    Json::Null => Ok(SqlValue::Null),
                    Json::String(s) => Ok(SqlValue::Text(s.clone())),
                    Json::Number(n) => n.as_i64().map(SqlValue::Integer).ok_or("Invalid integer"),
                    _ => Err("Invalid value"),
                })
                .collect::<std::result::Result<_, _>>()?;
            let placeholders = vec!["?"; values.len()].join(",");
            tx.execute(
                &format!("INSERT INTO {table} ({columns}) VALUES ({placeholders})"),
                params_from_iter(values),
            )?;
        }
    }
    tx.execute_batch(&format!(
        "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version=4;"
    ))?;
    validate_records(&tx)?;
    require(snapshot(&tx, 4)? == before, "Record mismatch before commit")?;
    pause("transaction", stop, destination)?;
    tx.commit()?;
    drop(candidate);
    let validated = Sqlite::open(&candidate_path)?;
    validate_records(&validated)?;
    require(
        snapshot(&validated, 4)? == before,
        "Record mismatch after reopen",
    )?;
    require(
        validated.query_row("SELECT engine FROM metadata", [], |r| r.get::<_, String>(0))?
            == "sqlite",
        "Invalid target engine",
    )?;
    require(
        validated.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? == 4,
        "Invalid target schema",
    )?;
    drop(validated);
    OpenOptions::new()
        .write(true)
        .open(&candidate_path)?
        .sync_all()?;
    pause("validated", stop, destination)?;
    fs::rename(candidate_path, destination.join("state.db"))?;
    pause("promoted", stop, destination)?;
    File::create(destination.join("complete"))?.sync_all()?;
    pause("complete", stop, destination)?;
    Ok(())
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
    fs::create_dir(root).unwrap();
    let child = worker(
        "storage::sqlite_proof::legacy_worker",
        root,
        root,
        schema,
        if populated { "populated" } else { "empty" },
        &root.join("ready"),
    );
    assert!(fs::metadata(root.join("state.db-wal")).unwrap().len() > 32);
    drop(child);
}

#[test]
#[ignore = "pinned Turso fixture writer, terminated by parent tests"]
fn legacy_worker() {
    let root = PathBuf::from(std::env::var_os("STARFRAME_PROOF_SOURCE").unwrap());
    let schema: i64 = std::env::var("STARFRAME_PROOF_SCHEMA")
        .unwrap()
        .parse()
        .unwrap();
    let mode = std::env::var("STARFRAME_PROOF_MODE").unwrap();
    let populated = mode != "empty";
    tauri::async_runtime::block_on(async {
        let _guard = lock(&root).unwrap();
        let db = turso::Builder::new_local(root.join("state.db").to_str().unwrap())
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")
            .await
            .unwrap();
        for migration in MIGRATIONS.iter().take(schema as usize) {
            conn.execute_batch(migration).await.unwrap();
        }
        conn.execute_batch(format!(
            "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={schema};"
        ))
        .await
        .unwrap();
        if populated {
            conn.execute("INSERT INTO library VALUES ('local', ?, 'Local 日本語', 'Rémy', 'dev build', 'local_import', NULL)", ["ab".repeat(32)]).await.unwrap();
            conn.execute("INSERT INTO library VALUES ('catalog', ?, 'Catalog fixture', 'Author', '1.2.3', 'catalog', 'release-1')", ["cd".repeat(32)]).await.unwrap();
            conn.execute(
                "INSERT INTO collections VALUES (?, 'Before WAL', 7)",
                [COLLECTION],
            )
            .await
            .unwrap();
            conn.execute("INSERT INTO collections VALUES ('33333333-3333-4333-8333-333333333333', 'Empty collection', 2)", ()).await.unwrap();
            conn.execute("INSERT INTO collection_entries VALUES (?, 0, 'missing', ?, 'catalog', 'release-missing')", [COLLECTION.to_string(), "ef".repeat(32)]).await.unwrap();
            conn.execute(
                "INSERT INTO collection_entries VALUES (?, 1, 'local', ?, 'local_import', NULL)",
                [COLLECTION.to_string(), "ab".repeat(32)],
            )
            .await
            .unwrap();
            if schema >= 2 {
                conn.execute("UPDATE preferences SET active_collection=?", [COLLECTION])
                    .await
                    .unwrap();
            }
            if schema >= 3 {
                conn.execute(
                    "INSERT INTO game_selection VALUES (1, ?, ?)",
                    [
                        GAME.to_owned(),
                        root.join("Game 日本語").to_str().unwrap().to_owned(),
                    ],
                )
                .await
                .unwrap();
            }
        }
        fs::create_dir_all(root.join("artifacts").join("ab".repeat(32))).unwrap();
        fs::write(
            root.join("artifacts").join("ab".repeat(32)).join("payload"),
            b"Retain source bytes",
        )
        .unwrap();
        let mut rows = conn
            .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
            .await
            .unwrap();
        while rows.next().await.unwrap().is_some() {}
        drop(rows);
        fs::create_dir(root.join("backup")).unwrap();
        copy_database(&root, &root.join("backup")).unwrap();
        File::create(root.join("backup/complete"))
            .unwrap()
            .sync_all()
            .unwrap();
        fs::write(
            root.join("backup/expected.json"),
            serde_json::to_vec(&legacy_snapshot(&conn, schema).await.unwrap()).unwrap(),
        )
        .unwrap();
        conn.execute("UPDATE metadata SET revision=9007199254741003", ())
            .await
            .unwrap();
        if populated {
            conn.execute(
                "UPDATE collections SET name='Après WAL 日本語' WHERE id=?",
                [COLLECTION],
            )
            .await
            .unwrap();
        }
        conn.cacheflush().unwrap();
        fs::write(
            root.join("expected.json"),
            serde_json::to_vec(&legacy_snapshot(&conn, schema).await.unwrap()).unwrap(),
        )
        .unwrap();
        if mode == "transaction" {
            conn.execute_batch(
                "BEGIN IMMEDIATE; DELETE FROM library; UPDATE metadata SET revision=1;",
            )
            .await
            .unwrap();
            conn.cacheflush().unwrap();
        }
        File::create(root.join("ready"))
            .unwrap()
            .sync_all()
            .unwrap();
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    });
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
        let db = tauri::async_runtime::block_on(
            turso::Builder::new_local(source.join("state.db").to_str().unwrap()).build(),
        )
        .unwrap();
        let conn = db.connect().unwrap();
        tauri::async_runtime::block_on(conn.execute_batch(sql)).unwrap();
        drop(conn);
        drop(db);
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
    fs::create_dir(&source).unwrap();
    let child = worker(
        "storage::sqlite_proof::legacy_worker",
        &source,
        &source,
        3,
        "populated",
        &source.join("ready"),
    );
    assert!(convert(&source, &temp.path().join("busy"), false, None).is_err());
    assert!(!temp.path().join("busy").exists());
    drop(child);
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
    fs::create_dir(&source).unwrap();
    let child = worker(
        "storage::sqlite_proof::legacy_worker",
        &source,
        &source,
        3,
        "transaction",
        &source.join("ready"),
    );
    drop(child);
    let before = tree(&source);
    let destination = temp.path().join("converted");
    convert(&source, &destination, false, None).unwrap();
    let conn = Sqlite::open(destination.join("state.db")).unwrap();
    assert_eq!(snapshot(&conn, 4).unwrap(), expected(&source));
    assert_eq!(tree(&source), before);
}
