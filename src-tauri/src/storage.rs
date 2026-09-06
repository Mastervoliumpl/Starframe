use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};
use turso::{
    Connection,
    transaction::{Transaction, TransactionBehavior},
};
use uuid::Uuid;

const SCHEMA: i64 = 3;
const APPLICATION_ID: i64 = 0x53544652;
const MIGRATIONS: [&str; 3] = [
    "CREATE TABLE metadata (id INTEGER PRIMARY KEY CHECK(id = 1), engine TEXT NOT NULL CHECK(engine = 'turso'), revision INTEGER NOT NULL CHECK(revision >= 0));
     INSERT INTO metadata VALUES (1, 'turso', 0);
     CREATE TABLE library (mod_id TEXT NOT NULL CHECK(length(mod_id) BETWEEN 1 AND 200), hash TEXT NOT NULL CHECK(length(hash) = 64 AND hash NOT GLOB '*[^0-9a-f]*'), name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200), author TEXT NOT NULL CHECK(length(author) <= 200), version TEXT NOT NULL CHECK(length(version) BETWEEN 1 AND 200), origin TEXT NOT NULL CHECK(origin IN ('catalog', 'local_import')), release_id TEXT, PRIMARY KEY(mod_id, hash), CHECK((origin = 'catalog' AND release_id IS NOT NULL AND length(release_id) BETWEEN 1 AND 200) OR (origin = 'local_import' AND release_id IS NULL)));
     CREATE TABLE collections (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 200), revision INTEGER NOT NULL CHECK(revision > 0));
     CREATE TABLE collection_entries (collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE, position INTEGER NOT NULL CHECK(position >= 0), mod_id TEXT NOT NULL CHECK(length(mod_id) BETWEEN 1 AND 200), hash TEXT NOT NULL CHECK(length(hash) = 64 AND hash NOT GLOB '*[^0-9a-f]*'), origin TEXT NOT NULL CHECK(origin IN ('catalog', 'local_import')), release_id TEXT, PRIMARY KEY(collection_id, position), UNIQUE(collection_id, mod_id), CHECK((origin = 'catalog' AND release_id IS NOT NULL AND length(release_id) BETWEEN 1 AND 200) OR (origin = 'local_import' AND release_id IS NULL)));",
    "CREATE TABLE preferences (id INTEGER PRIMARY KEY CHECK(id = 1), active_collection TEXT REFERENCES collections(id)); INSERT INTO preferences VALUES (1, NULL);",
    "CREATE TABLE game_selection (singleton INTEGER PRIMARY KEY CHECK(singleton = 1), installation_id TEXT NOT NULL, path TEXT NOT NULL CHECK(length(path) BETWEEN 1 AND 32768));",
];

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Database(turso::Error),
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
impl From<turso::Error> for Error {
    fn from(e: turso::Error) -> Self {
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
    _db: turso::Database,
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

async fn integer(conn: &Connection, sql: &str) -> Result<i64> {
    let mut rows = conn.query(sql, ()).await?;
    Ok(rows
        .next()
        .await?
        .ok_or_else(|| Error::Invalid("A required saved-data record is missing.".into()))?
        .get(0)?)
}

async fn finish<T>(tx: Transaction<'_>, result: Result<T>) -> Result<T> {
    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

impl Storage {
    pub async fn selected_game(&self) -> Result<Option<(String, String)>> {
        let mut rows = self
            .conn
            .query(
                "SELECT installation_id, path FROM game_selection WHERE singleton = 1",
                (),
            )
            .await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        let id: String = row.get(0)?;
        let path: String = row.get(1)?;
        validate_game_selection(&id, &path)?;
        Ok(Some((id, path)))
    }

    pub async fn select_game(&mut self, id: &str, path: &str) -> Result<i64> {
        validate_game_selection(id, path)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let result = async {
            tx.execute("INSERT INTO game_selection (singleton, installation_id, path) VALUES (1, ?, ?) ON CONFLICT(singleton) DO UPDATE SET installation_id=excluded.installation_id, path=excluded.path", [id, path]).await?;
            bump(&tx).await
        }.await;
        finish(tx, result).await
    }

    /// Open only from a background worker. File locks, backups and engine work can block.
    pub async fn open(root: &Path) -> Result<Self> {
        let guard = lock(root)?;
        Self::open_locked(root, guard).await
    }

    async fn open_locked(root: &Path, guard: File) -> Result<Self> {
        let path = root.join("state.db");
        let existing = path.exists();
        if existing {
            let mut header = [0u8; 100];
            use std::io::Read;
            File::open(&path)?.read_exact(&mut header)?;
            if &header[..16] != b"SQLite format 3\0" {
                return Err(Error::Invalid(
                    "The saved database header is invalid. It has been retained for recovery."
                        .into(),
                ));
            }
            if i64::from(u32::from_be_bytes(header[60..64].try_into().unwrap())) > SCHEMA {
                return Err(Error::Invalid("Saved data belongs to a newer Starframe version. Use that version; the data has been retained.".into()));
            }
        }
        let db = turso::Builder::new_local(
            path.to_str()
                .ok_or_else(|| Error::Invalid("The data path is not valid Unicode.".into()))?,
        )
        .build()
        .await?;
        let conn = db.connect()?;
        conn.busy_timeout(Duration::from_millis(250))?;
        let mut store = Self {
            conn,
            _db: db,
            root: root.into(),
            _lock: guard,
        };
        let version = integer(&store.conn, "PRAGMA user_version").await?;
        if version > SCHEMA {
            return Err(Error::Invalid(
                "Saved data belongs to a newer Starframe version. It has been retained.".into(),
            ));
        }
        if existing {
            if version == 0
                || integer(&store.conn, "PRAGMA application_id").await? != APPLICATION_ID
            {
                return Err(Error::Invalid(
                    "This database is not a supported Starframe database. It has been retained."
                        .into(),
                ));
            }
            let mut rows = store
                .conn
                .query("SELECT engine FROM metadata WHERE id = 1", ())
                .await?;
            if rows
                .next()
                .await?
                .is_none_or(|row| row.get::<String>(0).ok().as_deref() != Some("turso"))
            {
                return Err(Error::Invalid(
                    "The saved database engine is not supported. It has been retained.".into(),
                ));
            }
        }
        store
            .conn
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")
            .await?;
        store.check_integrity().await?;
        if existing && version < SCHEMA {
            store.backup().await?;
        }
        store.migrate(version, SCHEMA).await?;
        store.load().await?;
        Ok(store)
    }

    async fn check_integrity(&self) -> Result<()> {
        let mut rows = self.conn.query("PRAGMA integrity_check", ()).await?;
        let first = rows.next().await?.ok_or_else(|| {
            Error::Invalid("The database integrity check returned no result.".into())
        })?;
        if first.get::<String>(0)? != "ok" || rows.next().await?.is_some() {
            return Err(Error::Invalid("Saved data failed its integrity check. Retain the database and restore a verified backup.".into()));
        }
        Ok(())
    }

    async fn migrate(&mut self, from: i64, to: i64) -> Result<()> {
        for version in from..to {
            self.apply_migration(version + 1, MIGRATIONS[version as usize])
                .await?;
        }
        Ok(())
    }

    async fn apply_migration(&mut self, version: i64, sql: &str) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let result = async {
            tx.execute_batch(sql).await?;
            tx.execute_batch(format!(
                "PRAGMA application_id = {APPLICATION_ID}; PRAGMA user_version = {version};"
            ))
            .await?;
            Ok(())
        }
        .await;
        finish(tx, result).await
    }

    pub async fn load(&self) -> Result<Records> {
        let orphans = integer(&self.conn, "SELECT (SELECT count(*) FROM collection_entries e LEFT JOIN collections c ON c.id = e.collection_id WHERE c.id IS NULL) + (SELECT count(*) FROM preferences p LEFT JOIN collections c ON c.id = p.active_collection WHERE p.active_collection IS NOT NULL AND c.id IS NULL)").await?;
        if orphans != 0 {
            return Err(Error::Invalid(
                "Saved collection relationships are invalid. Restore a verified backup.".into(),
            ));
        }
        let revision = integer(&self.conn, "SELECT revision FROM metadata WHERE id = 1").await?;
        let mut library = vec![];
        let mut rows = self.conn.query("SELECT mod_id, hash, origin, release_id, name, author, version FROM library ORDER BY mod_id, hash", ()).await?;
        while let Some(row) = rows.next().await? {
            library.push(LibraryEntry {
                reference: reference(&row)?,
                name: row.get(4)?,
                author: row.get(5)?,
                version: row.get(6)?,
            });
        }
        drop(rows);
        let mut collections = vec![];
        let mut rows = self
            .conn
            .query("SELECT id, name, revision FROM collections ORDER BY id", ())
            .await?;
        while let Some(row) = rows.next().await? {
            collections.push(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                revision: row.get(2)?,
                entries: vec![],
            });
        }
        drop(rows);
        for collection in &mut collections {
            let mut entries = self.conn.query("SELECT mod_id, hash, origin, release_id, position FROM collection_entries WHERE collection_id = ? ORDER BY position", [collection.id.as_str()]).await?;
            while let Some(row) = entries.next().await? {
                if row.get::<i64>(4)? != collection.entries.len() as i64 {
                    return Err(Error::Invalid(
                        "A saved collection has an incomplete order. Restore a verified backup."
                            .into(),
                    ));
                }
                collection.entries.push(reference(&row)?);
            }
        }
        let mut rows = self
            .conn
            .query("SELECT active_collection FROM preferences WHERE id = 1", ())
            .await?;
        let active_collection = rows
            .next()
            .await?
            .ok_or_else(|| Error::Invalid("Saved preferences are missing.".into()))?
            .get(0)?;
        Ok(Records {
            revision,
            library,
            collections,
            active_collection,
        })
    }

    pub async fn put_library_entry(&mut self, entry: &LibraryEntry, expected: i64) -> Result<i64> {
        validate_reference(&entry.reference)?;
        validate_text(&entry.name, "Mod name")?;
        validate_text(&entry.version, "Version label")?;
        if entry.author.chars().count() > 200 {
            return Err(Error::Invalid("The author name is too long.".into()));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let result = async {
            check_revision(&tx, expected).await?;
            tx.execute("INSERT INTO library (mod_id, hash, origin, release_id, name, author, version) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(mod_id, hash) DO UPDATE SET name=excluded.name, author=excluded.author, version=excluded.version, origin=excluded.origin, release_id=excluded.release_id", turso::params![entry.reference.mod_id.clone(), entry.reference.hash.clone(), entry.reference.origin.as_str(), entry.reference.release_id.clone(), entry.name.clone(), entry.author.clone(), entry.version.clone()]).await?;
            bump(&tx).await
        }.await;
        finish(tx, result).await
    }

    pub async fn save_collection(
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
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let result = async {
            let mut rows = tx.query("SELECT revision FROM collections WHERE id = ?", [id]).await?;
            let current = match rows.next().await? { Some(row) => row.get::<i64>(0)?, None => 0 };
            drop(rows);
            if current != expected { return Err(Error::Stale { current }); }
            let revision = current.checked_add(1).ok_or_else(|| Error::Invalid("Collection revision limit reached.".into()))?;
            tx.execute("INSERT INTO collections (id, name, revision) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET name=excluded.name, revision=excluded.revision", turso::params![id, name, revision]).await?;
            tx.execute("DELETE FROM collection_entries WHERE collection_id = ?", [id]).await?;
            for (position, entry) in entries.iter().enumerate() {
                tx.execute("INSERT INTO collection_entries (collection_id, position, mod_id, hash, origin, release_id) VALUES (?, ?, ?, ?, ?, ?)", turso::params![id, position as i64, entry.mod_id.clone(), entry.hash.clone(), entry.origin.as_str(), entry.release_id.clone()]).await?;
            }
            bump(&tx).await?;
            Ok(revision)
        }.await;
        finish(tx, result).await
    }

    pub async fn set_active_collection(&mut self, id: Option<&str>, expected: i64) -> Result<i64> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let result = async {
            check_revision(&tx, expected).await?;
            tx.execute(
                "UPDATE preferences SET active_collection = ? WHERE id = 1",
                [id],
            )
            .await?;
            bump(&tx).await
        }
        .await;
        finish(tx, result).await
    }

    pub fn artifact_directory(&self, reference: &ModReference) -> Result<PathBuf> {
        validate_reference(reference)?;
        Ok(self.root.join("artifacts").join(&reference.hash))
    }

    pub async fn backup(&self) -> Result<PathBuf> {
        let mut rows = self
            .conn
            .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
            .await?;
        if rows
            .next()
            .await?
            .ok_or_else(|| Error::Invalid("The database checkpoint returned no result.".into()))?
            .get::<i64>(0)?
            != 0
        {
            return Err(Error::Invalid(
                "The database is busy. A backup was not created.".into(),
            ));
        }
        drop(rows);
        let folder = self.root.join("backups").join(Uuid::new_v4().to_string());
        fs::create_dir_all(&folder)?;
        copy_database(&self.root, &folder)?;
        File::create(folder.join("complete"))?.sync_all()?;
        Ok(folder)
    }

    /// Restore into a new directory; the damaged database and original backup remain untouched.
    pub async fn restore_into(backup: &Path, destination: &Path) -> Result<Self> {
        if !backup.join("complete").is_file() {
            return Err(Error::Invalid("The backup is incomplete.".into()));
        }
        let guard = lock(destination)?;
        if destination.join("state.db").exists() || destination.join("state.db-wal").exists() {
            return Err(Error::Invalid(
                "Restore requires an empty destination. Existing saved data was retained.".into(),
            ));
        }
        copy_database(backup, destination)?;
        Self::open_locked(destination, guard).await
    }
}

fn copy_database(source: &Path, destination: &Path) -> Result<()> {
    for name in ["state.db", "state.db-wal"] {
        let path = source.join(name);
        if name == "state.db" || path.exists() {
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

fn reference(row: &turso::Row) -> Result<ModReference> {
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
async fn check_revision(conn: &Connection, expected: i64) -> Result<()> {
    let current = integer(conn, "SELECT revision FROM metadata WHERE id = 1").await?;
    if current != expected {
        return Err(Error::Stale { current });
    }
    Ok(())
}
async fn bump(conn: &Connection) -> Result<i64> {
    let current = integer(conn, "SELECT revision FROM metadata WHERE id = 1").await?;
    let next = current
        .checked_add(1)
        .ok_or_else(|| Error::Invalid("Saved-data revision limit reached.".into()))?;
    conn.execute("UPDATE metadata SET revision = ? WHERE id = 1", [next])
        .await?;
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

    #[tokio::test(flavor = "current_thread")]
    async fn game_selection_survives_migration_and_restart() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).await.unwrap();
        store.put_library_entry(&entry(), 0).await.unwrap();
        store
            .conn
            .execute_batch("DROP TABLE game_selection; PRAGMA user_version = 2;")
            .await
            .unwrap();
        drop(store);
        let mut store = Storage::open(root.path()).await.unwrap();
        assert!(store.selected_game().await.unwrap().is_none());
        let id = Uuid::new_v4().to_string();
        let path = root.path().join("game").to_str().unwrap().to_owned();
        assert_eq!(store.select_game(&id, &path).await.unwrap(), 2);
        assert!(store.select_game("invalid", "relative/path").await.is_err());
        drop(store);
        let store = Storage::open(root.path()).await.unwrap();
        assert_eq!(store.selected_game().await.unwrap(), Some((id, path)));
        assert_eq!(store.load().await.unwrap().library, vec![entry()]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn records_survive_restart_with_order_origin_and_revisions() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).await.unwrap();
        assert_eq!(
            integer(&store.conn, "PRAGMA foreign_keys").await.unwrap(),
            1
        );
        assert_eq!(integer(&store.conn, "PRAGMA synchronous").await.unwrap(), 2);
        let id = Uuid::new_v4().to_string();
        assert_eq!(store.put_library_entry(&entry(), 0).await.unwrap(), 1);
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
                .await
                .unwrap(),
            1
        );
        store.set_active_collection(Some(&id), 2).await.unwrap();
        let before = store.load().await.unwrap();
        assert_eq!(before.revision, 3);
        assert!(
            !store
                .artifact_directory(&local("fixture"))
                .unwrap()
                .exists()
        );
        drop(store);
        let mut store = Storage::open(root.path()).await.unwrap();
        assert_eq!(store.load().await.unwrap(), before);
        assert!(matches!(
            store.save_collection(&id, "Old draft", &[], 0).await,
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
                .await
                .is_err()
        );
        assert_eq!(store.load().await.unwrap(), before);
        assert!(
            store
                .set_active_collection(Some("missing"), 3)
                .await
                .is_err()
        );
        assert_eq!(store.load().await.unwrap(), before);
        assert!(
            store
                .conn
                .execute("INSERT INTO collections VALUES ('invalid', '', 0)", ())
                .await
                .is_err()
        );
        store
            .save_collection(&id, "Reordered", &[local("fixture"), catalog], 1)
            .await
            .unwrap();
        assert_eq!(
            store.load().await.unwrap().collections[0].entries[0],
            local("fixture")
        );
        let mut invalid = entry();
        invalid.reference.hash = "../outside".into();
        assert!(store.put_library_entry(&invalid, 4).await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn migrations_back_up_and_failed_migrations_roll_back() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).await.unwrap();
        store.put_library_entry(&entry(), 0).await.unwrap();
        store
            .conn
            .execute_batch(
                "DROP TABLE preferences; DROP TABLE game_selection; PRAGMA user_version = 1;",
            )
            .await
            .unwrap();
        let backup = store.backup().await.unwrap();
        assert!(
            store
                .apply_migration(
                    2,
                    "CREATE TABLE partial (id INTEGER); INSERT INTO missing_table VALUES (1);"
                )
                .await
                .is_err()
        );
        assert_eq!(
            integer(&store.conn, "PRAGMA user_version").await.unwrap(),
            1
        );
        assert_eq!(
            integer(
                &store.conn,
                "SELECT count(*) FROM sqlite_schema WHERE name = 'partial'"
            )
            .await
            .unwrap(),
            0
        );
        drop(store);
        let store = Storage::open(root.path()).await.unwrap();
        assert_eq!(store.load().await.unwrap().library, vec![entry()]);
        assert_eq!(
            integer(&store.conn, "PRAGMA user_version").await.unwrap(),
            SCHEMA
        );
        assert_eq!(
            fs::read_dir(root.path().join("backups")).unwrap().count(),
            2
        );
        let restored = tempfile::tempdir().unwrap();
        let recovered = Storage::restore_into(&backup, restored.path())
            .await
            .unwrap();
        assert_eq!(recovered.load().await.unwrap(), store.load().await.unwrap());
        assert!(Storage::restore_into(&backup, root.path()).await.is_err());
        assert!(
            Storage::restore_into(root.path(), tempfile::tempdir().unwrap().path())
                .await
                .is_err()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn corrupt_newer_and_foreign_databases_are_retained() {
        let root = tempfile::tempdir().unwrap();
        let corrupt = vec![0x5a; 8192];
        fs::write(root.path().join("state.db"), &corrupt).unwrap();
        assert!(Storage::open(root.path()).await.is_err());
        assert_eq!(fs::read(root.path().join("state.db")).unwrap(), corrupt);
        let newer = tempfile::tempdir().unwrap();
        let store = Storage::open(newer.path()).await.unwrap();
        store
            .conn
            .execute("PRAGMA user_version = 99", ())
            .await
            .unwrap();
        store.backup().await.unwrap();
        drop(store);
        let before = fs::read(newer.path().join("state.db")).unwrap();
        assert!(Storage::open(newer.path()).await.is_err());
        assert_eq!(fs::read(newer.path().join("state.db")).unwrap(), before);
        let foreign = tempfile::tempdir().unwrap();
        let db = turso::Builder::new_local(foreign.path().join("state.db").to_str().unwrap())
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        conn.execute("CREATE TABLE other_data (value TEXT)", ())
            .await
            .unwrap();
        conn.execute("INSERT INTO other_data VALUES ('retain')", ())
            .await
            .unwrap();
        drop(conn);
        drop(db);
        assert!(Storage::open(foreign.path()).await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn concurrent_owners_and_busy_writes_fail_without_losing_state() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Storage::open(root.path()).await.unwrap();
        assert!(Storage::open(root.path()).await.is_err());
        let second = store._db.connect().unwrap();
        second.busy_timeout(Duration::from_millis(50)).unwrap();
        store.conn.execute("BEGIN IMMEDIATE", ()).await.unwrap();
        let start = Instant::now();
        assert!(second.execute("BEGIN IMMEDIATE", ()).await.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        store.conn.execute("ROLLBACK", ()).await.unwrap();
        store.put_library_entry(&entry(), 0).await.unwrap();
        assert_eq!(store.load().await.unwrap().revision, 1);
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
                let result = tauri::async_runtime::block_on(Storage::open(root.path()));
                assert!(result.is_err());
            }
            child.kill().unwrap();
            child.wait().unwrap();
            let recovered = tauri::async_runtime::block_on(Storage::open(root.path())).unwrap();
            let records = tauri::async_runtime::block_on(recovered.load()).unwrap();
            assert_eq!(records.library, vec![entry()]);
            assert_eq!(records.revision, 1);
        }
    }

    #[test]
    #[ignore = "spawned by the process-interruption test"]
    fn crash_worker() {
        let root = PathBuf::from(std::env::var_os("STARFRAME_TEST_ROOT").unwrap());
        let mode = std::env::var("STARFRAME_TEST_MODE").unwrap();
        tauri::async_runtime::block_on(async {
            let mut store = Storage::open(&root).await.unwrap();
            store.put_library_entry(&entry(), 0).await.unwrap();
            if mode == "migration" {
                store
                    .conn
                    .execute_batch("DROP TABLE preferences; DROP TABLE game_selection; PRAGMA user_version = 1;")
                    .await
                    .unwrap();
                store.backup().await.unwrap();
                store.conn.execute("BEGIN IMMEDIATE", ()).await.unwrap();
                store.conn.execute_batch(MIGRATIONS[1]).await.unwrap();
                store
                    .conn
                    .execute("PRAGMA user_version = 2", ())
                    .await
                    .unwrap();
            } else if mode == "transaction" {
                store.conn.execute("BEGIN IMMEDIATE", ()).await.unwrap();
                store.conn.execute("DELETE FROM library", ()).await.unwrap();
                store
                    .conn
                    .execute("UPDATE metadata SET revision = 2", ())
                    .await
                    .unwrap();
            }
            store.conn.cacheflush().unwrap();
            let mut ready = File::create(root.join("ready")).unwrap();
            ready
                .write_all(b"committed base; unfinished work ready")
                .unwrap();
            ready.sync_all().unwrap();
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    }
}
