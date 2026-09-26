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

    pub fn set_mod_membership(
        &mut self,
        entries: &[crate::references::Reference],
        expected: i64,
    ) -> Result<i64> {
        if entries.len() > crate::runtime_contract::MAX_MODS {
            return Err(Error::Invalid(
                "This runtime supports at most 256 active mods.".into(),
            ));
        }
        let mut ids = std::collections::HashSet::new();
        for entry in entries {
            entry.validate().map_err(Error::Invalid)?;
            if !ids.insert(entry.runtime_id()) {
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
                write_collection_entry(&tx, &id, position, entry)?;
            }
            bump(&tx)
        })();
        finish(tx, result)
    }

    pub fn uninstall_reference(
        &mut self,
        reference: &crate::references::Reference,
        expected: i64,
        confirmed: bool,
    ) -> Result<i64> {
        reference.validate().map_err(Error::Invalid)?;
        let records = self.load()?;
        if records.revision != expected {
            return Err(Error::Stale {
                current: records.revision,
            });
        }
        let installed = match reference {
            crate::references::Reference::Registry(reference) => self
                .installed_registry_releases()?
                .iter()
                .any(|entry| &entry.reference == reference),
            crate::references::Reference::Local(_) => records
                .library
                .iter()
                .any(|entry| Some(entry.reference.clone()) == reference.local_reference()),
        };
        if !installed {
            return Err(Error::Invalid(
                "This exact package is not in the library.".into(),
            ));
        }
        let affected = records
            .collections
            .iter()
            .filter(|collection| collection.entries.contains(reference))
            .map(|collection| collection.name.as_str())
            .collect::<Vec<_>>();
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
                .find(|collection| Some(&collection.id) == records.active_collection.as_ref())
            {
                let remaining = active
                    .entries
                    .iter()
                    .filter(|entry| *entry != reference)
                    .collect::<Vec<_>>();
                tx.execute(
                    "DELETE FROM collection_entries WHERE collection_id=?",
                    [&active.id],
                )?;
                for (position, entry) in remaining.into_iter().enumerate() {
                    write_collection_entry(&tx, &active.id, position, entry)?;
                }
                tx.execute(
                    "UPDATE collections SET revision=revision+1 WHERE id=?",
                    [&active.id],
                )?;
            }
            match reference {
                crate::references::Reference::Registry(reference) => {
                    tx.execute("UPDATE registry_library SET installation_record=NULL WHERE mod_id=? AND release_id=? AND sha256=?",rusqlite::params![u64::from(reference.mod_id) as i64,reference.release_id.0.to_string(),reference.sha256.as_str()])?;
                }
                crate::references::Reference::Local(reference) => {
                    tx.execute("DELETE FROM library WHERE mod_id=? AND hash=? AND origin='local_import' AND release_id IS NULL",rusqlite::params![reference.mod_id,reference.sha256.as_str()])?;
                    tx.execute(
                        "DELETE FROM local_watches WHERE mod_id=? AND hash=?",
                        rusqlite::params![reference.mod_id, reference.sha256.as_str()],
                    )?;
                    tx.execute(
                        "DELETE FROM local_sources WHERE mod_id=? AND hash=?",
                        rusqlite::params![reference.mod_id, reference.sha256.as_str()],
                    )?;
                }
            }
            if tx.query_row("SELECT (SELECT count(*) FROM library WHERE hash=?1) + (SELECT count(*) FROM registry_library WHERE sha256=?1 AND installation_record IS NOT NULL)",[reference.hash()],|row|row.get::<_,i64>(0))?==0 {
                tx.execute("INSERT OR IGNORE INTO pending_removals(hash) VALUES (?)",[reference.hash()])?;
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
