use super::*;
use crate::packages::{Operation, Prepared, Status};
use rusqlite::OptionalExtension;

fn operation_record(operation: &Operation) -> Result<String> {
    if Uuid::parse_str(&operation.id).is_err()
        || Uuid::parse_str(&operation.request_id).is_err()
        || operation.release_id.is_empty()
        || operation.release_id.len() > 128
        || operation.message.len() > 8000
        || operation.received_bytes > operation.total_bytes
        || !(1..=2_147_483_648).contains(&operation.total_bytes)
    {
        return Err(Error::Invalid("Invalid saved package operation.".into()));
    }
    validate_reference(&ModReference {
        mod_id: operation.release_id.clone(),
        hash: operation.hash.clone(),
        origin: Origin::Catalog,
        release_id: Some(operation.release_id.clone()),
    })?;
    serde_json::to_string(operation).map_err(|e| Error::Invalid(e.to_string()))
}

fn prepared_record(prepared: &Prepared) -> Result<String> {
    validate_reference(&ModReference {
        mod_id: "artifact".into(),
        hash: prepared.hash.clone(),
        origin: Origin::LocalImport,
        release_id: None,
    })?;
    let mut paths = std::collections::BTreeSet::new();
    let mut total = 0u64;
    if prepared.files.is_empty() || prepared.files.len() > 4096 {
        return Err(Error::Invalid("Invalid prepared file count.".into()));
    }
    for file in &prepared.files {
        let path = crate::runtime_contract::relative_path(&file.path).map_err(Error::Invalid)?;
        if !paths.insert(path) || file.size_bytes > 512 * 1024 * 1024 {
            return Err(Error::Invalid("Invalid prepared file path or size.".into()));
        }
        total += file.size_bytes;
        validate_reference(&ModReference {
            mod_id: "file".into(),
            hash: file.sha256.clone(),
            origin: Origin::LocalImport,
            release_id: None,
        })?;
    }
    if total > 2 * 1024 * 1024 * 1024 {
        return Err(Error::Invalid("Prepared package exceeds 2 GiB.".into()));
    }
    serde_json::to_string(prepared).map_err(|e| Error::Invalid(e.to_string()))
}

impl Storage {
    pub(crate) fn package_root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn package_request(&self, request_id: &str) -> Result<Option<Operation>> {
        let record: Option<String> = self
            .conn
            .query_row(
                "SELECT record FROM package_operations WHERE request_id=?",
                [request_id],
                |r| r.get(0),
            )
            .optional()?;
        record
            .map(|record| {
                let operation = serde_json::from_str(&record)
                    .map_err(|e| Error::Invalid(format!("Invalid package history: {e}")))?;
                operation_record(&operation)?;
                Ok(operation)
            })
            .transpose()
    }

    pub fn package_operations(&self) -> Result<Vec<Operation>> {
        let mut query = self
            .conn
            .prepare("SELECT record FROM package_operations ORDER BY rowid DESC LIMIT 100")?;
        query
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|record| {
                let operation = serde_json::from_str(&record?)
                    .map_err(|e| Error::Invalid(format!("Invalid package history: {e}")))?;
                operation_record(&operation)?;
                Ok(operation)
            })
            .collect()
    }

    pub fn prepared_artifact(&self, hash: &str) -> Result<Option<Prepared>> {
        let record: Option<String> = self
            .conn
            .query_row(
                "SELECT record FROM prepared_artifacts WHERE hash=?",
                [hash],
                |r| r.get(0),
            )
            .optional()?;
        record
            .map(|record| {
                let prepared: Prepared = serde_json::from_str(&record)
                    .map_err(|e| Error::Invalid(format!("Invalid prepared artifact: {e}")))?;
                prepared_record(&prepared)?;
                if prepared.hash != hash {
                    return Err(Error::Invalid(
                        "Prepared artifact identity mismatch.".into(),
                    ));
                }
                Ok(prepared)
            })
            .transpose()
    }

    pub(crate) fn save_package(&mut self, operation: &Operation) -> Result<()> {
        let record = operation_record(operation)?;
        self.conn.execute("INSERT INTO package_operations (id, request_id, record) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET record=excluded.record", rusqlite::params![operation.id, operation.request_id, record])?;
        Ok(())
    }

    pub(crate) fn recover_packages(&mut self) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE package_operations SET record=json_set(record, '$.status', 'failed', '$.message', 'Starframe closed before package preparation was committed. Retry the import or release. Partial staging was retained; no game files changed.') WHERE json_extract(record, '$.status') IN ('preparing', 'cancelling')", [])?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn complete_package(
        &mut self,
        operation: &Operation,
        entry: &LibraryEntry,
        prepared: &Prepared,
    ) -> Result<()> {
        self.complete_import(operation, entry, prepared, None)
    }

    pub(crate) fn complete_import(
        &mut self,
        operation: &Operation,
        entry: &LibraryEntry,
        prepared: &Prepared,
        local: Option<&crate::local_import::LocalSource>,
    ) -> Result<()> {
        self.commit_import(operation, entry, prepared, local, None)
    }

    pub(super) fn commit_import(
        &mut self,
        operation: &Operation,
        entry: &LibraryEntry,
        prepared: &Prepared,
        local: Option<&crate::local_import::LocalSource>,
        advance: Option<(&str, &ModReference)>,
    ) -> Result<()> {
        let record = operation_record(operation)?;
        let manifest = prepared_record(prepared)?;
        crate::packages::supported_files(&prepared.files).map_err(Error::Invalid)?;
        validate_reference(&entry.reference)?;
        validate_text(&entry.name, "Mod name")?;
        validate_text(&entry.version, "Version label")?;
        if operation.status != Status::Completed
            || operation.hash != prepared.hash
            || entry.reference.hash != prepared.hash
            || match local {
                Some(source) => source.entry() != *entry || operation.release_id != "local-import",
                None => {
                    entry.reference.origin != Origin::Catalog
                        || entry.reference.release_id.as_ref() != Some(&operation.release_id)
                }
            }
            || entry.author.chars().count() > 200
        {
            return Err(Error::Invalid(
                "Prepared package does not match its operation.".into(),
            ));
        }
        if let Some(source) = local {
            source.validate().map_err(Error::Invalid)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT record FROM prepared_artifacts WHERE hash=?",
                [&prepared.hash],
                |r| r.get(0),
            )
            .optional()?;
        if previous.is_some_and(|value| value != manifest) {
            return Err(Error::Invalid(
                "Prepared content identity changed. Retain it for repair.".into(),
            ));
        }
        tx.execute("INSERT INTO prepared_artifacts (hash, record) VALUES (?, ?) ON CONFLICT(hash) DO NOTHING", rusqlite::params![prepared.hash, manifest])?;
        tx.execute("INSERT INTO library (mod_id, hash, origin, release_id, name, author, version) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT DO NOTHING", rusqlite::params![entry.reference.mod_id, prepared.hash, entry.reference.origin.as_str(), entry.reference.release_id, entry.name, entry.author, entry.version])?;
        if let Some(source) = local {
            let record =
                serde_json::to_string(source).map_err(|e| Error::Invalid(e.to_string()))?;
            tx.execute("INSERT INTO local_sources (mod_id,hash,record) VALUES (?,?,?) ON CONFLICT(mod_id,hash) DO UPDATE SET record=excluded.record", rusqlite::params![entry.reference.mod_id, prepared.hash, record])?;
            tx.execute("INSERT INTO local_watches(mod_id,hash) VALUES (?,?) ON CONFLICT(mod_id) DO UPDATE SET hash=excluded.hash,state='watching',message='Verified build saved. Active local collections apply when the game is closed.'", rusqlite::params![entry.reference.mod_id, prepared.hash])?;
        }
        if let Some((collection, previous)) = advance {
            if tx.execute("UPDATE collection_entries SET hash=? WHERE collection_id=? AND mod_id=? AND hash=? AND origin='local_import' AND release_id IS NULL", rusqlite::params![prepared.hash, collection, previous.mod_id, previous.hash])? != 1 {
                return Err(Error::Invalid("The active local build changed before it could be saved.".into()));
            }
            tx.execute(
                "UPDATE collections SET revision=revision+1 WHERE id=?",
                [collection],
            )?;
        }
        if tx.execute(
            "UPDATE package_operations SET record=? WHERE id=? AND request_id=?",
            rusqlite::params![record, operation.id, operation.request_id],
        )? != 1
        {
            return Err(Error::Invalid(
                "Prepared package operation is missing.".into(),
            ));
        }
        bump(&tx)?;
        tx.commit()?;
        Ok(())
    }
}
