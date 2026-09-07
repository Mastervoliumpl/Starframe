use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

mod packages;

const SCHEMA: i64 = 7;
const APPLICATION_ID: i64 = 0x53544652;
const MIGRATIONS: [&str; 7] = [
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

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModReference {
    pub mod_id: String,
    pub hash: String,
    pub origin: Origin,
    pub release_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub reference: ModReference,
    pub name: String,
    pub author: String,
    pub version: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
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
        let mut statement = self.conn.prepare("SELECT mod_id, hash, origin, release_id, name, author, version FROM library ORDER BY mod_id, hash")?;
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
            tx.execute("INSERT INTO library (mod_id, hash, origin, release_id, name, author, version) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(mod_id, hash) DO UPDATE SET name=excluded.name, author=excluded.author, version=excluded.version, origin=excluded.origin, release_id=excluded.release_id", rusqlite::params![entry.reference.mod_id.clone(), entry.reference.hash.clone(), entry.reference.origin.as_str(), entry.reference.release_id.clone(), entry.name.clone(), entry.author.clone(), entry.version.clone()])?;
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
fn validate_reference(reference: &ModReference) -> Result<()> {
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

#[cfg(test)]
mod tests {
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
            .execute_batch("DROP TABLE package_operations; DROP TABLE prepared_artifacts; DROP TABLE catalog_cache; PRAGMA user_version=5;")
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
            .execute_batch("DROP TABLE game_selection; DROP TABLE deployments; DROP TABLE deployment_blobs; DROP TABLE catalog_cache; DROP TABLE package_operations; DROP TABLE prepared_artifacts; PRAGMA user_version = 2;")
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
                "DROP TABLE preferences; DROP TABLE game_selection; DROP TABLE deployments; DROP TABLE deployment_blobs; DROP TABLE catalog_cache; DROP TABLE package_operations; DROP TABLE prepared_artifacts; PRAGMA user_version = 1;",
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
                    .execute_batch("DROP TABLE preferences; DROP TABLE game_selection; DROP TABLE deployments; DROP TABLE deployment_blobs; DROP TABLE catalog_cache; DROP TABLE package_operations; DROP TABLE prepared_artifacts; PRAGMA user_version = 1;")
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
}

mod conversion {
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
    pub(super) fn convert_locked(
        source: &Path,
        destination: &Path,
        stop: Option<&str>,
    ) -> Check<()> {
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
                        Json::Number(n) => {
                            n.as_i64().map(SqlValue::Integer).ok_or("Invalid integer")
                        }
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
}
#[cfg(test)]
mod sqlite_proof;
