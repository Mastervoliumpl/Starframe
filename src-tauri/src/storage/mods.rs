use super::*;

impl Storage {
    pub fn delete_collection(&mut self, id: &str, expected: i64) -> Result<i64> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            check_revision(&tx, expected)?;
            tx.execute(
                "UPDATE preferences SET active_collection=NULL WHERE active_collection=?",
                [id],
            )?;
            if tx.execute("DELETE FROM collections WHERE id=?", [id])? != 1 {
                return Err(Error::Invalid("This collection no longer exists.".into()));
            }
            bump(&tx)
        })();
        finish(tx, result)
    }

    pub fn set_mod_membership(&mut self, entries: &[ModReference], expected: i64) -> Result<i64> {
        if entries.len() > crate::runtime_contract::MAX_MODS {
            return Err(Error::Invalid(
                "This runtime supports at most 256 active mods.".into(),
            ));
        }
        let mut ids = std::collections::HashSet::new();
        for entry in entries {
            validate_reference(entry)?;
            if !ids.insert(&entry.mod_id) {
                return Err(Error::Invalid(
                    "Only one version of a mod can be enabled.".into(),
                ));
            }
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            check_revision(&tx, expected)?;
            let selected: Option<String> = tx.query_row(
                "SELECT active_collection FROM preferences WHERE id=1",
                [],
                |r| r.get(0),
            )?;
            let id = selected.unwrap_or_else(|| Uuid::new_v4().to_string());
            tx.execute("INSERT INTO collections (id,name,revision) VALUES (?, 'Default', 1) ON CONFLICT(id) DO UPDATE SET revision=revision+1", [&id])?;
            tx.execute(
                "UPDATE preferences SET active_collection=? WHERE id=1",
                [&id],
            )?;
            tx.execute(
                "DELETE FROM collection_entries WHERE collection_id=?",
                [&id],
            )?;
            for (position, entry) in entries.iter().enumerate() {
                tx.execute("INSERT INTO collection_entries (collection_id,position,mod_id,hash,origin,release_id) VALUES (?,?,?,?,?,?)", rusqlite::params![id, position as i64, entry.mod_id, entry.hash, entry.origin.as_str(), entry.release_id])?;
            }
            bump(&tx)
        })();
        finish(tx, result)
    }

    pub fn uninstall_mod(
        &mut self,
        reference: &ModReference,
        expected: i64,
        confirmed: bool,
    ) -> Result<i64> {
        let records = self.load()?;
        if records.revision != expected {
            return Err(Error::Invalid(
                "The library changed. Retry with its current revision.".into(),
            ));
        }
        let entry = records
            .library
            .iter()
            .find(|e| &e.reference == reference)
            .ok_or_else(|| Error::Invalid("This exact package is not in the library.".into()))?;
        validate_reference(&entry.reference)?;
        let affected: Vec<_> = records
            .collections
            .iter()
            .filter(|c| c.entries.iter().any(|r| r == reference))
            .map(|c| c.name.as_str())
            .collect();
        if !affected.is_empty() && !confirmed {
            return Err(Error::Invalid(format!(
                "Uninstall affects collections: {}. Confirm removal; other collections will retain an unresolved reference.",
                affected.join(", ")
            )));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            check_revision(&tx, expected)?;
            if let Some(active) = records
                .collections
                .iter()
                .find(|c| Some(&c.id) == records.active_collection.as_ref())
            {
                let remaining: Vec<_> = active.entries.iter().filter(|r| *r != reference).collect();
                tx.execute(
                    "DELETE FROM collection_entries WHERE collection_id=?",
                    [&active.id],
                )?;
                for (position, entry) in remaining.iter().enumerate() {
                    tx.execute("INSERT INTO collection_entries (collection_id,position,mod_id,hash,origin,release_id) VALUES (?,?,?,?,?,?)", rusqlite::params![active.id, position as i64, entry.mod_id, entry.hash, entry.origin.as_str(), entry.release_id])?;
                }
                tx.execute(
                    "UPDATE collections SET revision=revision+1 WHERE id=?",
                    [&active.id],
                )?;
            }
            tx.execute(
                "DELETE FROM library WHERE mod_id=? AND hash=? AND origin=? AND release_id IS ?",
                rusqlite::params![
                    reference.mod_id,
                    reference.hash,
                    reference.origin.as_str(),
                    reference.release_id
                ],
            )?;
            if tx.query_row(
                "SELECT count(*) FROM library WHERE hash=?",
                [&reference.hash],
                |r| r.get::<_, i64>(0),
            )? == 0
            {
                tx.execute(
                    "INSERT OR IGNORE INTO pending_removals(hash) VALUES (?)",
                    [&reference.hash],
                )?;
            }
            if reference.origin == Origin::LocalImport {
                tx.execute(
                    "DELETE FROM local_watches WHERE mod_id=? AND hash=?",
                    rusqlite::params![reference.mod_id, reference.hash],
                )?;
                tx.execute(
                    "DELETE FROM local_sources WHERE mod_id=? AND hash=?",
                    rusqlite::params![reference.mod_id, reference.hash],
                )?;
            }
            bump(&tx)
        })();
        finish(tx, result)
    }

    pub(crate) fn pending_removals(&self) -> Result<Vec<(String, String)>> {
        self.conn
            .prepare("SELECT hash,error FROM pending_removals ORDER BY hash")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<_, _>>()
            .map_err(Error::from)
    }

    pub(crate) fn finish_removal(&mut self, hash: &str, error: Option<&str>) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(error) = error {
            tx.execute(
                "UPDATE pending_removals SET error=? WHERE hash=?",
                [error, hash],
            )?;
        } else {
            tx.execute("DELETE FROM prepared_artifacts WHERE hash=?", [hash])?;
            tx.execute("DELETE FROM pending_removals WHERE hash=?", [hash])?;
        }
        tx.commit()?;
        Ok(())
    }
}
