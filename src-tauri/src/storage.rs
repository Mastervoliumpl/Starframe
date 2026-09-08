use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

mod mods;
mod packages;
mod sharing;

const SCHEMA: i64 = 10;
const APPLICATION_ID: i64 = 0x53544652;
const MIGRATIONS: [&str; 10] = [
    "CREATE TABLE metadata (id INTEGER PRIMARY KEY CHECK(id = 1), engine TEXT NOT NULL CHECK(engine = 'sqlite'), revision INTEGER NOT NULL CHECK(revision >= 0));
     INSERT INTO metadata VALUES (1, 'sqlite', 0);
     CREATE TABLE library (mod_id TEXT NOT NULL CHECK(length(mod_id) BETWEEN 1 AND 200), hash TEXT NOT NULL CHECK(length(hash) = 64 AND hash NOT GLOB '*[^0-9a-f]*'), name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200), author TEXT NOT NULL CHECK(length(author) <= 200), version TEXT NOT NULL CHECK(length(version) BETWEEN 1 AND 200), origin TEXT NOT NULL CHECK(origin IN ('catalog', 'local_import')), release_id TEXT, PRIMARY KEY(mod_id, hash), CHECK((origin = 'catalog' AND release_id IS NOT NULL AND length(release_id) BETWEEN 1 AND 200) OR (origin = 'local_import' AND release_id IS NULL)));
     CREATE TABLE collections (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200), revision INTEGER NOT NULL CHECK(revision > 0));
     CREATE TABLE collection_entries (collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE, position INTEGER NOT NULL CHECK(position >= 0), mod_id TEXT NOT NULL CHECK(length(mod_id) BETWEEN 1 AND 200), hash TEXT NOT NULL CHECK(length(hash) = 64 AND hash NOT GLOB '*[^0-9a-f]*'), origin TEXT NOT NULL CHECK(origin IN ('catalog', 'local_import')), release_id TEXT, PRIMARY KEY(collection_id, position), UNIQUE(collection_id, mod_id), CHECK((origin = 'catalog' AND release_id IS NOT NULL AND length(release_id) BETWEEN 1 AND 200) OR (origin = 'local_import' AND release_id IS NULL)));",
    "CREATE TABLE preferences (id INTEGER PRIMARY KEY CHECK(id = 1), active_collection TEXT REFERENCES collections(id)); INSERT INTO preferences VALUES (1, NULL);",
    "CREATE TABLE game_selection (singleton INTEGER PRIMARY KEY CHECK(singleton = 1), installation_id TEXT NOT NULL, path TEXT NOT NULL CHECK(length(path) BETWEEN 1 AND 32768));",
    "SELECT 1;",
    "CREATE TABLE deployments (root TEXT PRIMARY KEY NOT NULL, record TEXT NOT NULL CHECK(length(record) <= 1048576 AND json_valid(record))); CREATE TABLE deployment_blobs (hash TEXT PRIMARY KEY NOT NULL CHECK(length(hash)=64), bytes BLOB NOT NULL CHECK(length(bytes)<=8388608));",
    "CREATE TABLE catalog_cache (id INTEGER PRIMARY KEY CHECK(id=1), record TEXT NOT NULL CHECK(length(record)<=2105344 AND json_valid(record)));",
    "CREATE TABLE package_operations (id TEXT PRIMARY KEY NOT NULL, request_id TEXT UNIQUE NOT NULL, record TEXT NOT NULL CHECK(length(record)<=8192 AND json_valid(record))); CREATE TABLE prepared_artifacts (hash TEXT PRIMARY KEY NOT NULL CHECK(length(hash)=64 AND hash NOT GLOB '*[^0-9a-f]*'), record TEXT NOT NULL CHECK(length(record)<=2097152 AND json_valid(record)));",
    "CREATE TABLE pending_removals (hash TEXT PRIMARY KEY NOT NULL CHECK(length(hash)=64 AND hash NOT GLOB '*[^0-9a-f]*'), error TEXT NOT NULL DEFAULT '');",
    "CREATE TABLE collection_imports (collection_id TEXT PRIMARY KEY NOT NULL REFERENCES collections(id) ON DELETE CASCADE, record TEXT NOT NULL CHECK(length(record)<=1048576 AND json_valid(record)));",
    "CREATE TABLE library_new (mod_id TEXT NOT NULL CHECK(length(mod_id) BETWEEN 1 AND 200), hash TEXT NOT NULL CHECK(length(hash) = 64 AND hash NOT GLOB '*[^0-9a-f]*'), name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200), author TEXT NOT NULL CHECK(length(author) <= 200), version TEXT NOT NULL CHECK(length(version) BETWEEN 1 AND 200), origin TEXT NOT NULL CHECK(origin IN ('catalog', 'local_import')), release_id TEXT, PRIMARY KEY(mod_id, hash, origin, release_id), CHECK((origin = 'catalog' AND release_id IS NOT NULL AND length(release_id) BETWEEN 1 AND 200) OR (origin = 'local_import' AND release_id IS NULL))); INSERT INTO library_new SELECT * FROM library; DROP TABLE library; ALTER TABLE library_new RENAME TO library; CREATE UNIQUE INDEX local_library_identity ON library(mod_id, hash) WHERE origin = 'local_import';",
];

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Database(rusqlite::Error),
    Invalid(String),
    Stale { current: i64 },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Saved data could not be accessed: {e}"),
            Self::Database(e) => write!(f, "Saved data could not be read or committed: {e}"),
            Self::Invalid(e) => f.write_str(e),
            Self::Stale { current } => {
                write!(f, "The record changed. Its current revision is {current}.")
            }
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e)
    }
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Origin {
    Catalog,
    LocalImport,
}
impl Origin {
    fn as_str(&self) -> &str {
        match self {
            Self::Catalog => "catalog",
            Self::LocalImport => "local_import",
        }
    }
    fn parse(value: String) -> Result<Self> {
        match value.as_str() {
            "catalog" => Ok(Self::Catalog),
            "local_import" => Ok(Self::LocalImport),
            _ => Err(Error::Invalid("The saved mod origin is invalid.".into())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(rename = "Reference"))]
pub struct ModReference {
    pub mod_id: String,
    pub hash: String,
    pub origin: Origin,
    pub release_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LibraryEntry {
    pub reference: ModReference,
    pub name: String,
    pub author: String,
    pub version: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Collection {
    pub id: String,
    pub name: String,
    #[cfg_attr(test, ts(type = "number"))]
    pub revision: i64,
    pub entries: Vec<ModReference>,
}
#[derive(Debug, PartialEq)]
pub struct Records {
    pub revision: i64,
    pub library: Vec<LibraryEntry>,
    pub collections: Vec<Collection>,
    pub active_collection: Option<String>,
}

pub struct Storage {
    conn: Connection,
    root: PathBuf,
    _lock: File,
}

fn validate_game_selection(id: &str, path: &str) -> Result<()> {
    if Uuid::parse_str(id).is_err()
        || path.len() > 32768
        || !Path::new(path).is_absolute()
        || path.chars().any(char::is_control)
    {
        return Err(Error::Invalid(
            "The saved game selection is invalid.".into(),
        ));
    }
    Ok(())
}

fn lock(root: &Path) -> Result<File> {
    fs::create_dir_all(root)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("state.lock"))?;
    file.try_lock().map_err(|_| {
        Error::Invalid(
            "Saved data is already open in another process. Close that process and retry.".into(),
        )
    })?;
    Ok(file)
}

fn integer(conn: &Connection, sql: &str) -> Result<i64> {
    Ok(conn.query_row(sql, [], |row| row.get(0))?)
}

fn finish<T>(tx: Transaction<'_>, result: Result<T>) -> Result<T> {
    match result {
        Ok(value) => {
            tx.commit()?;
            Ok(value)
        }
        Err(error) => {
            tx.rollback()?;
            Err(error)
        }
    }
}

impl Storage {
    pub fn catalog_cache(&self) -> Result<Option<crate::catalog::refresh::Cache>> {
        use rusqlite::OptionalExtension;
        let record: Option<String> = self
            .conn
            .query_row("SELECT record FROM catalog_cache WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        record
            .map(|record| {
                crate::catalog::refresh::Cache::read(record.as_bytes()).map_err(Error::Invalid)
            })
            .transpose()
    }

    pub fn save_catalog_cache(&mut self, cache: &crate::catalog::refresh::Cache) -> Result<()> {
        let record = serde_json::to_vec(cache).map_err(|e| Error::Invalid(e.to_string()))?;
        crate::catalog::refresh::Cache::read(&record).map_err(Error::Invalid)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        {
            use rusqlite::OptionalExtension;
            let previous: Option<String> = tx
                .query_row("SELECT record FROM catalog_cache WHERE id=1", [], |r| {
                    r.get(0)
                })
                .optional()?;
            if let Some(previous) = previous {
                let previous = crate::catalog::refresh::Cache::read(previous.as_bytes())
                    .map_err(Error::Invalid)?;
                if let Some(old) = previous.catalog {
                    cache
                        .catalog
                        .as_ref()
                        .ok_or_else(|| {
                            Error::Invalid("Cannot discard the validated catalog cache.".into())
                        })?
                        .accepts_after(&old)
                        .map_err(Error::Invalid)?;
                }
            }
        }
        tx.execute("INSERT INTO catalog_cache (id, record) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET record=excluded.record", [String::from_utf8(record).map_err(|e| Error::Invalid(e.to_string()))?])?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn deployment_record(&self, root: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn
            .query_row(
                "SELECT record FROM deployments WHERE root=?",
                [root],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(crate) fn save_deployment(
        &mut self,
        root: &str,
        record: &str,
        blobs: &[(String, Vec<u8>)],
    ) -> Result<()> {
        use sha2::{Digest, Sha256};
        if !Path::new(root).is_absolute()
            || record.len() > 1_048_576
            || serde_json::from_str::<serde_json::Value>(record).is_err()
        {
            return Err(Error::Invalid("Invalid deployment record.".into()));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (hash, bytes) in blobs {
            if bytes.len() > 8_388_608 || format!("{:x}", Sha256::digest(bytes)) != *hash {
                return Err(Error::Invalid("Invalid deployment backup.".into()));
            }
            tx.execute(
                "INSERT OR IGNORE INTO deployment_blobs (hash, bytes) VALUES (?, ?)",
                rusqlite::params![hash, bytes],
            )?;
            let retained: Vec<u8> = tx.query_row(
                "SELECT bytes FROM deployment_blobs WHERE hash=?",
                [hash],
                |row| row.get(0),
            )?;
            if retained != *bytes {
                return Err(Error::Invalid(
                    "A retained deployment backup is corrupt. No game files were changed.".into(),
                ));
            }
        }
        tx.execute("INSERT INTO deployments (root, record) VALUES (?, ?) ON CONFLICT(root) DO UPDATE SET record=excluded.record", [root, record])?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn deployment_blob(&self, hash: &str) -> Result<Vec<u8>> {
        use sha2::{Digest, Sha256};
        let bytes: Vec<u8> = self.conn.query_row(
            "SELECT bytes FROM deployment_blobs WHERE hash=?",
            [hash],
            |row| row.get(0),
        )?;
        if bytes.len() > 8_388_608 || format!("{:x}", Sha256::digest(&bytes)) != hash {
            return Err(Error::Invalid(
                "Deployment backup failed its hash check. Retain the database and recovery files."
                    .into(),
            ));
        }
        Ok(bytes)
    }

    pub fn selected_game(&self) -> Result<Option<(String, String)>> {
        let mut statement = self
            .conn
            .prepare("SELECT installation_id, path FROM game_selection WHERE singleton = 1")?;
        let mut rows = statement.query([])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let id: String = row.get(0)?;
        let path: String = row.get(1)?;
        validate_game_selection(&id, &path)?;
        Ok(Some((id, path)))
    }

    pub fn select_game(&mut self, id: &str, path: &str) -> Result<i64> {
        validate_game_selection(id, path)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            tx.execute("INSERT INTO game_selection (singleton, installation_id, path) VALUES (1, ?, ?) ON CONFLICT(singleton) DO UPDATE SET installation_id=excluded.installation_id, path=excluded.path", [id, path])?;
            bump(&tx)
        })();
        finish(tx, result)
    }

    /// Open only from a background worker. File locks, backups and engine work can block.
    pub fn open(root: &Path) -> Result<Self> {
        let guard = lock(root)?;
        #[cfg(debug_assertions)]
        if std::env::var_os("STARFRAME_TEST_DATA_DIR").as_deref() == Some(root.as_os_str()) {
            while root.join("hold-storage-startup").is_file() {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        Self::open_locked(root, guard)
    }

    fn open_locked(root: &Path, guard: File) -> Result<Self> {
        let active = root.join("sqlite");
        if !active.exists() {
            let staging = root.join(format!("sqlite-staging-{}", Uuid::new_v4()));
            if root.join("state.db").exists() {
                conversion::convert_locked(root, &staging, None).map_err(|e| {
                    Error::Invalid(format!(
                        "Saved-data conversion failed: {e}. Original files were retained."
                    ))
                })?;
            } else {
                if root.join("state.db-wal").exists() {
                    return Err(Error::Invalid(
                        "Saved data is missing its database. The WAL was retained for recovery."
                            .into(),
                    ));
                }
                fs::create_dir(&staging)?;
                let mut conn = Connection::open(staging.join("state.db"))?;
                conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")?;
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                for sql in MIGRATIONS {
                    tx.execute_batch(sql)?;
                }
                tx.execute_batch(&format!(
                    "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={SCHEMA};"
                ))?;
                tx.commit()?;
                drop(conn);
                OpenOptions::new()
                    .write(true)
                    .open(staging.join("state.db"))?
                    .sync_all()?;
                File::create(staging.join("complete"))?.sync_all()?;
            }
            conversion::pause("before-switch", None, root)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            fs::rename(&staging, &active)?;
            conversion::pause("after-switch", None, root)
                .map_err(|e| Error::Invalid(e.to_string()))?;
        }
        conversion::regular(&active.join("complete")).map_err(|e| {
            Error::Invalid(format!(
                "The SQLite destination is incomplete: {e}. Its files were retained."
            ))
        })?;
        conversion::regular(&active.join("state.db")).map_err(|e| Error::Invalid(e.to_string()))?;
        let mut header = [0u8; 100];
        use std::io::Read;
        File::open(active.join("state.db"))?.read_exact(&mut header)?;
        if &header[..16] != b"SQLite format 3\0" {
            return Err(Error::Invalid(
                "The saved database header is invalid. Its files were retained.".into(),
            ));
        }
        if i64::from(u32::from_be_bytes(header[60..64].try_into().unwrap())) > SCHEMA {
            return Err(Error::Invalid(
                "Saved data belongs to a newer Starframe version. Its files were retained.".into(),
            ));
        }
        let conn = Connection::open_with_flags(
            active.join("state.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
        )?;
        conn.busy_timeout(Duration::from_millis(250))?;
        conn.execute_batch(
            "PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;",
        )?;
        let version = integer(&conn, "PRAGMA user_version")?;
        if version > SCHEMA {
            return Err(Error::Invalid(
                "Saved data belongs to a newer Starframe version. Its files were retained.".into(),
            ));
        }
        if version < 1 || integer(&conn, "PRAGMA application_id")? != APPLICATION_ID {
            return Err(Error::Invalid(
                "This database is not supported Starframe data. Its files were retained.".into(),
            ));
        }
        let engine: String =
            conn.query_row("SELECT engine FROM metadata WHERE id=1", [], |r| r.get(0))?;
        if engine != "sqlite" {
            return Err(Error::Invalid(
                "The saved database engine is not supported. Its files were retained.".into(),
            ));
        }
        let mut store = Self {
            conn,
            root: root.into(),
            _lock: guard,
        };
        store.check_integrity()?;
        if version < SCHEMA {
            store.backup()?;
            store.migrate(version, SCHEMA)?;
        }
        conversion::validate_records(&store.conn).map_err(|e| Error::Invalid(e.to_string()))?;
        store.load()?;
        let journal: String = store
            .conn
            .query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
        if journal != "wal" {
            return Err(Error::Invalid("SQLite WAL mode is unavailable.".into()));
        }
        Ok(store)
    }

    fn check_integrity(&self) -> Result<()> {
        let mut statement = self.conn.prepare("PRAGMA integrity_check")?;
        let mut rows = statement.query([])?;
        let first = rows.next()?.ok_or_else(|| {
            Error::Invalid("The database integrity check returned no result.".into())
        })?;
        if first.get::<_, String>(0)? != "ok" || rows.next()?.is_some() {
            return Err(Error::Invalid("Saved data failed its integrity check. Retain the database and restore a verified backup.".into()));
        }
        Ok(())
    }

    fn migrate(&mut self, from: i64, to: i64) -> Result<()> {
        for version in from..to {
            self.apply_migration(
                version + 1,
                if version == 3 {
                    "SELECT 1;"
                } else {
                    MIGRATIONS[version as usize]
                },
            )?;
        }
        Ok(())
    }

    fn apply_migration(&mut self, version: i64, sql: &str) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            tx.execute_batch(sql)?;
            tx.execute_batch(&format!(
                "PRAGMA application_id = {APPLICATION_ID}; PRAGMA user_version = {version};"
            ))?;
            Ok(())
        })();
        finish(tx, result)
    }

    pub fn load(&self) -> Result<Records> {
        let orphans = integer(
            &self.conn,
            "SELECT (SELECT count(*) FROM collection_entries e LEFT JOIN collections c ON c.id = e.collection_id WHERE c.id IS NULL) + (SELECT count(*) FROM preferences p LEFT JOIN collections c ON c.id = p.active_collection WHERE p.active_collection IS NOT NULL AND c.id IS NULL)",
        )?;
        if orphans != 0 {
            return Err(Error::Invalid(
                "Saved collection relationships are invalid. Restore a verified backup.".into(),
            ));
        }
        let revision = integer(&self.conn, "SELECT revision FROM metadata WHERE id = 1")?;
        let mut library = vec![];
        let mut statement = self.conn.prepare("SELECT mod_id, hash, origin, release_id, name, author, version FROM library ORDER BY mod_id, hash, origin, release_id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            library.push(LibraryEntry {
                reference: reference(row)?,
                name: row.get(4)?,
                author: row.get(5)?,
                version: row.get(6)?,
            });
        }
        drop(rows);
        let mut collections = vec![];
        let mut statement = self
            .conn
            .prepare("SELECT id, name, revision FROM collections ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            collections.push(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                revision: row.get(2)?,
                entries: vec![],
            });
        }
        drop(rows);
        for collection in &mut collections {
            let mut statement = self.conn.prepare("SELECT mod_id, hash, origin, release_id, position FROM collection_entries WHERE collection_id = ? ORDER BY position")?;
            let mut entries = statement.query([collection.id.as_str()])?;
            while let Some(row) = entries.next()? {
                if row.get::<_, i64>(4)? != collection.entries.len() as i64 {
                    return Err(Error::Invalid(
                        "A saved collection has an incomplete order. Restore a verified backup."
                            .into(),
                    ));
                }
                collection.entries.push(reference(row)?);
            }
        }
        let mut statement = self
            .conn
            .prepare("SELECT active_collection FROM preferences WHERE id = 1")?;
        let mut rows = statement.query([])?;
        let active_collection = rows
            .next()?
            .ok_or_else(|| Error::Invalid("Saved preferences are missing.".into()))?
            .get(0)?;
        Ok(Records {
            revision,
            library,
            collections,
            active_collection,
        })
    }

    pub fn put_library_entry(&mut self, entry: &LibraryEntry, expected: i64) -> Result<i64> {
        validate_reference(&entry.reference)?;
        validate_text(&entry.name, "Mod name")?;
        validate_text(&entry.version, "Version label")?;
        if entry.author.chars().count() > 200 {
            return Err(Error::Invalid("The author name is too long.".into()));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            check_revision(&tx, expected)?;
            tx.execute("INSERT INTO library (mod_id, hash, origin, release_id, name, author, version) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT DO UPDATE SET name=excluded.name, author=excluded.author, version=excluded.version, origin=excluded.origin, release_id=excluded.release_id", rusqlite::params![entry.reference.mod_id.clone(), entry.reference.hash.clone(), entry.reference.origin.as_str(), entry.reference.release_id.clone(), entry.name.clone(), entry.author.clone(), entry.version.clone()])?;
            bump(&tx)
        })();
        finish(tx, result)
    }

    pub fn save_collection(
        &mut self,
        id: &str,
        name: &str,
        entries: &[ModReference],
        expected: i64,
    ) -> Result<i64> {
        Uuid::parse_str(id).map_err(|_| Error::Invalid("The collection ID is invalid.".into()))?;
        validate_text(name, "Collection name")?;
        if entries.len() > 10_000 {
            return Err(Error::Invalid(
                "A collection cannot contain more than 10,000 entries.".into(),
            ));
        }
        for entry in entries {
            validate_reference(entry)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            let mut statement = tx.prepare("SELECT revision FROM collections WHERE id = ?")?;
            let mut rows = statement.query([id])?;
            let current = match rows.next()? {
                Some(row) => row.get::<_, i64>(0)?,
                None => 0,
            };
            drop(rows);
            if current != expected {
                return Err(Error::Stale { current });
            }
            let revision = current
                .checked_add(1)
                .ok_or_else(|| Error::Invalid("Collection revision limit reached.".into()))?;
            tx.execute("INSERT INTO collections (id, name, revision) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET name=excluded.name, revision=excluded.revision", rusqlite::params![id, name, revision])?;
            tx.execute(
                "DELETE FROM collection_entries WHERE collection_id = ?",
                [id],
            )?;
            for (position, entry) in entries.iter().enumerate() {
                tx.execute("INSERT INTO collection_entries (collection_id, position, mod_id, hash, origin, release_id) VALUES (?, ?, ?, ?, ?, ?)", rusqlite::params![id, position as i64, entry.mod_id.clone(), entry.hash.clone(), entry.origin.as_str(), entry.release_id.clone()])?;
            }
            bump(&tx)?;
            Ok(revision)
        })();
        finish(tx, result)
    }

    pub fn set_active_collection(&mut self, id: Option<&str>, expected: i64) -> Result<i64> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            check_revision(&tx, expected)?;
            tx.execute(
                "UPDATE preferences SET active_collection = ? WHERE id = 1",
                [id],
            )?;
            bump(&tx)
        })();
        finish(tx, result)
    }

    pub fn artifact_directory(&self, reference: &ModReference) -> Result<PathBuf> {
        validate_reference(reference)?;
        Ok(self.root.join("artifacts").join(&reference.hash))
    }

    pub fn backup(&self) -> Result<PathBuf> {
        let folder = self.root.join("backups").join(Uuid::new_v4().to_string());
        fs::create_dir_all(&folder)?;
        self.conn.backup("main", folder.join("state.db"), None)?;
        let copy = Connection::open(folder.join("state.db"))?;
        if integer(&copy, "PRAGMA user_version")? == SCHEMA {
            conversion::validate_records(&copy).map_err(|e| Error::Invalid(e.to_string()))?;
        }
        drop(copy);
        OpenOptions::new()
            .write(true)
            .open(folder.join("state.db"))?
            .sync_all()?;
        conversion::pause("backup-written", None, &folder)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        File::create(folder.join("complete"))?.sync_all()?;
        conversion::pause("backup-complete", None, &folder)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        Ok(folder)
    }

    /// Restore into a new directory; the damaged database and original backup remain untouched.
    pub fn restore_into(backup: &Path, destination: &Path) -> Result<Self> {
        conversion::regular(&backup.join("complete"))
            .map_err(|_| Error::Invalid("The backup is incomplete.".into()))?;
        if destination.exists() && fs::read_dir(destination)?.next().is_some() {
            return Err(Error::Invalid(
                "Restore requires an empty destination. Existing files were retained.".into(),
            ));
        }
        let guard = lock(destination)?;
        let staging = destination.join(format!("restore-staging-{}", Uuid::new_v4()));
        fs::create_dir(&staging)?;
        copy_database(backup, &staging)?;
        let mut copy = Connection::open(staging.join("state.db"))?;
        copy.busy_timeout(Duration::from_millis(250))?;
        copy.execute_batch(
            "PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;",
        )?;
        let engine: String =
            copy.query_row("SELECT engine FROM metadata WHERE id=1", [], |r| r.get(0))?;
        if engine == "turso" {
            drop(copy);
            let converted = destination.join(format!("converted-{}", Uuid::new_v4()));
            conversion::convert_locked(&staging, &converted, None)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            fs::rename(converted, destination.join("sqlite"))?;
        } else {
            let version = integer(&copy, "PRAGMA user_version")?;
            if engine != "sqlite"
                || !(1..=SCHEMA).contains(&version)
                || integer(&copy, "PRAGMA application_id")? != APPLICATION_ID
            {
                return Err(Error::Invalid(
                    "The backup is not supported Starframe SQLite data. Its files were retained."
                        .into(),
                ));
            }
            if version < SCHEMA {
                let tx = copy.transaction_with_behavior(TransactionBehavior::Immediate)?;
                for sql in MIGRATIONS.iter().skip(version as usize) {
                    tx.execute_batch(sql)?;
                }
                tx.execute_batch(&format!("PRAGMA user_version={SCHEMA};"))?;
                tx.commit()?;
            }
            conversion::validate_records(&copy).map_err(|e| Error::Invalid(e.to_string()))?;
            drop(copy);
            OpenOptions::new()
                .write(true)
                .open(staging.join("state.db"))?
                .sync_all()?;
            File::create(staging.join("complete"))?.sync_all()?;
            fs::rename(staging, destination.join("sqlite"))?;
        }
        Self::open_locked(destination, guard)
    }
}

fn copy_database(source: &Path, destination: &Path) -> Result<()> {
    for name in ["state.db", "state.db-wal"] {
        let path = source.join(name);
        if name == "state.db" || path.exists() {
            conversion::regular(&path).map_err(|e| Error::Invalid(e.to_string()))?;
            let mut reader = File::open(path)?;
            let mut writer = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination.join(name))?;
            std::io::copy(&mut reader, &mut writer)?;
            writer.sync_all()?;
        }
    }
    Ok(())
}

fn reference(row: &rusqlite::Row) -> Result<ModReference> {
    let reference = ModReference {
        mod_id: row.get(0)?,
        hash: row.get(1)?,
        origin: Origin::parse(row.get(2)?)?,
        release_id: row.get(3)?,
    };
    validate_reference(&reference)?;
    Ok(reference)
}
fn validate_text(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > 200 || value.contains('\0') {
        return Err(Error::Invalid(format!(
            "{field} must contain 1–200 characters and no null characters."
        )));
    }
    Ok(())
}
pub(crate) fn validate_reference(reference: &ModReference) -> Result<()> {
    validate_text(&reference.mod_id, "Mod ID")?;
    if reference.hash.len() != 64
        || !reference
            .hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Invalid(
            "The artifact hash must be 64 lowercase hexadecimal characters.".into(),
        ));
    }
    match (&reference.origin, &reference.release_id) {
        (Origin::Catalog, Some(id)) => validate_text(id, "Release ID"),
        (Origin::LocalImport, None) => Ok(()),
        _ => Err(Error::Invalid(
            "Catalog references need a release ID; local imports cannot use one.".into(),
        )),
    }
}
fn check_revision(conn: &Connection, expected: i64) -> Result<()> {
    let current = integer(conn, "SELECT revision FROM metadata WHERE id = 1")?;
    if current != expected {
        return Err(Error::Stale { current });
    }
    Ok(())
}
fn bump(conn: &Connection) -> Result<i64> {
    let current = integer(conn, "SELECT revision FROM metadata WHERE id = 1")?;
    let next = current
        .checked_add(1)
        .ok_or_else(|| Error::Invalid("Saved-data revision limit reached.".into()))?;
    conn.execute("UPDATE metadata SET revision = ? WHERE id = 1", [next])?;
    Ok(next)
}

mod conversion;
#[cfg(test)]
mod sqlite_proof;
#[cfg(test)]
mod tests;
