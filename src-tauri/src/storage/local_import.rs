use super::*;
use crate::local_import::{LocalSource, LocalWatch, WatchState};

impl Storage {
    pub fn local_watches(&self) -> Result<Vec<LocalWatch>> {
        self.conn.prepare("SELECT s.record,w.state,w.message,w.mod_id,w.hash FROM local_watches w JOIN local_sources s ON s.mod_id=w.mod_id AND s.hash=w.hash JOIN library l ON l.mod_id=w.mod_id AND l.hash=w.hash AND l.origin='local_import' ORDER BY w.mod_id")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?)))?
            .map(|row| {
                let (record, state, message, id, hash) = row?;
                let source: LocalSource = serde_json::from_str(&record).map_err(|e| Error::Invalid(e.to_string()))?;
                source.validate().map_err(Error::Invalid)?;
                if source.reference.mod_id != id || source.reference.hash != hash {
                    return Err(Error::Invalid("Saved local source identity differs from its watch key.".into()));
                }
                let state = match state.as_str() {
                    "watching" => WatchState::Watching, "settling" => WatchState::Settling, "error" => WatchState::Error,
                    _ => return Err(Error::Invalid("Invalid local watching state.".into())),
                };
                Ok(LocalWatch { source, state, message })
            }).collect()
    }

    pub(crate) fn watch_status(
        &mut self,
        source: &LocalSource,
        state: WatchState,
        message: &str,
    ) -> Result<()> {
        if !self.local_watches()?.iter().any(|w| &w.source == source) {
            return Ok(());
        }
        let state = match state {
            WatchState::Watching => "watching",
            WatchState::Settling => "settling",
            WatchState::Error => "error",
        };
        self.conn.execute(
            "UPDATE local_watches SET state=?,message=? WHERE mod_id=? AND hash=?",
            rusqlite::params![
                state,
                message.chars().take(1000).collect::<String>(),
                source.reference.mod_id,
                source.reference.hash
            ],
        )?;
        Ok(())
    }

    pub(crate) fn complete_watch(
        &mut self,
        previous: &LocalSource,
        local: &LocalSource,
        prepared: &crate::packages::Prepared,
    ) -> Result<bool> {
        if !self.local_watches()?.iter().any(|w| &w.source == previous) {
            return Ok(false);
        }
        if local.path != previous.path || local.reference.mod_id != previous.reference.mod_id {
            return Err(Error::Invalid("The source now identifies a different mod. Import it explicitly; the previous build was kept.".into()));
        }
        if local.reference == previous.reference {
            return Ok(false);
        }
        let records = self.load()?;
        let active = records
            .collections
            .iter()
            .find(|c| Some(&c.id) == records.active_collection.as_ref());
        let advance = active.filter(|c| c.entries.contains(&previous.reference));
        let advance = match advance {
            Some(collection)
                if self.conn.query_row(
                    "SELECT count(*) FROM collection_imports WHERE collection_id=?",
                    [&collection.id],
                    |r| r.get::<_, i64>(0),
                )? == 0 =>
            {
                Some(collection)
            }
            _ => None,
        };
        if let Some(collection) = advance {
            let entries = collection
                .entries
                .iter()
                .map(|r| {
                    if r == &previous.reference {
                        local.reference.clone()
                    } else {
                        r.clone()
                    }
                })
                .collect::<Vec<_>>();
            let catalog = self.catalog_cache()?.and_then(|c| c.catalog);
            let mut locals = self.local_sources()?;
            locals.push(local.clone());
            crate::ordering::resolve_with_locals(catalog.as_ref(), &locals, &entries)
                .map_err(Error::Invalid)?;
            let mut total = 0;
            for reference in entries {
                if reference == local.reference {
                    total += prepared.files.iter().map(|f| f.size_bytes).sum::<u64>();
                } else {
                    let files = self.prepared_artifact(&reference.hash)?.ok_or_else(|| {
                        Error::Invalid("A required managed copy is missing.".into())
                    })?;
                    total += files.files.iter().map(|f| f.size_bytes).sum::<u64>();
                }
            }
            if total > crate::runtime_contract::MAX_ACTIVATION_BYTES {
                return Err(Error::Invalid(
                    "The rebuilt collection exceeds 256 MiB. The previous build was kept.".into(),
                ));
            }
        }
        let size = prepared
            .files
            .iter()
            .map(|f| f.size_bytes)
            .sum::<u64>()
            .max(1);
        let mut operation = crate::packages::Operation {
            id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            release_id: "local-import".into(),
            hash: prepared.hash.clone(),
            status: crate::packages::Status::Preparing,
            message: "Saving the verified local rebuild.".into(),
            received_bytes: size,
            total_bytes: size,
        };
        self.save_package(&operation)?;
        operation.status = crate::packages::Status::Completed;
        operation.message = format!(
            "Verified rebuild of {} saved. Game files change only when the game is closed.",
            local.manifest.name
        );
        let result = self.commit_import(
            &operation,
            &local.entry(),
            prepared,
            Some(local),
            advance.map(|c| (c.id.as_str(), &previous.reference)),
        );
        if let Err(error) = result {
            operation.status = crate::packages::Status::Failed;
            operation.message = "The rebuild could not be saved. The previous build was kept; source watching will retry.".into();
            self.save_package(&operation)?;
            return Err(error);
        }
        Ok(true)
    }

    pub fn local_sources(&self) -> Result<Vec<LocalSource>> {
        self.conn.prepare("SELECT s.mod_id, s.hash, s.record FROM local_sources s JOIN library l ON l.mod_id=s.mod_id AND l.hash=s.hash AND l.origin='local_import' ORDER BY s.mod_id,s.hash")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
            .map(|row| {
                let (id, hash, record) = row?;
                let source: LocalSource = serde_json::from_str(&record).map_err(|e| Error::Invalid(e.to_string()))?;
                source.validate().map_err(Error::Invalid)?;
                if source.reference.mod_id != id || source.reference.hash != hash {
                    return Err(Error::Invalid("Saved local source identity differs from its key.".into()));
                }
                Ok(source)
            }).collect()
    }
}
