use super::*;
use rusqlite::{Connection as Sqlite, types::Value as SqlValue};
use serde_json::{Value as Json, json};
type Check<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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
    let reference_sql = "SELECT mod_id, hash, origin, release_id FROM library";
    for row in sqlite_rows(conn, reference_sql)? {
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
    {
        for row in sqlite_rows(conn, "SELECT runtime_key, record FROM collection_entries")? {
            let reference: crate::references::Reference = serde_json::from_value(
                crate::runtime_contract::unique_json(text(&row, 1)?.as_bytes())?,
            )?;
            reference.validate()?;
            require(
                reference.runtime_id() == text(&row, 0)?,
                "Collection identity does not match its reference",
            )?;
        }
        for row in sqlite_rows(conn, "SELECT collection_id, record FROM collection_imports")? {
            let import: crate::sharing::Import = serde_json::from_value(
                crate::runtime_contract::unique_json(text(&row, 1)?.as_bytes())?,
            )?;
            require(
                import.collection_id == text(&row, 0)?,
                "Import identity does not match its collection",
            )?;
            let portable = crate::sharing::Portable {
                format: "starframe-collection".into(),
                schema_version: 2,
                name: "Backup validation".into(),
                entries: import
                    .entries
                    .into_iter()
                    .map(|entry| entry.reference)
                    .collect(),
            };
            portable.validate()?;
        }
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
    let max_entries = crate::runtime_contract::MAX_MODS;
    require(sqlite_rows(conn, &format!("SELECT collection_id FROM collection_entries GROUP BY collection_id HAVING min(position) != 0 OR max(position) != count(*) - 1 OR count(*) > {max_entries}"))?.is_empty(), "Invalid collection order")?;
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
    let override_phase = std::env::var("STARFRAME_STORAGE_STOP").ok();
    #[cfg(test)]
    let stop = stop.or(override_phase.as_deref());
    if stop == Some(phase) {
        #[cfg(test)]
        let signal = std::env::var_os("STARFRAME_STORAGE_SIGNAL")
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
