use super::*;
use rusqlite::{Connection as Sqlite, params_from_iter, types::Value as SqlValue};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

type Check<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type Snapshot = BTreeMap<String, Vec<Vec<Json>>>;
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

pub(super) fn snapshot(conn: &Sqlite, schema: i64) -> Check<Snapshot> {
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

fn text(row: &[Json], index: usize) -> Check<&str> {
    row[index]
        .as_str()
        .ok_or_else(|| "Expected text record".into())
}

pub(super) fn validate_records(conn: &Sqlite) -> Check<()> {
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

pub(super) fn regular(path: &Path) -> Check<()> {
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

pub(super) fn pause(phase: &str, stop: Option<&str>, destination: &Path) -> Check<()> {
    #[cfg(test)]
    let override_phase = std::env::var("STARFRAME_CONVERSION_STOP").ok();
    #[cfg(test)]
    let stop = stop.or(override_phase.as_deref());
    if stop == Some(phase) {
        #[cfg(test)]
        let signal = std::env::var_os("STARFRAME_CONVERSION_SIGNAL")
            .map(PathBuf::from)
            .unwrap_or_else(|| destination.join("ready"));
        #[cfg(not(test))]
        let signal = destination.join("ready");
        File::create(signal)?.sync_all()?;
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

// The caller holds the data-directory lock. SQLite opens only the staged copy.
pub(super) fn convert_locked(source: &Path, destination: &Path, stop: Option<&str>) -> Check<()> {
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
    for migration in MIGRATIONS.iter().take(3) {
        tx.execute_batch(migration)?;
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
