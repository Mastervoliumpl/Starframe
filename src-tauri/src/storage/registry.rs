use super::{Error, Result, Storage};
use crate::registry::{ExactReference, ModId, ReleaseId, Sha256};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl Storage {
    pub fn save_registry_reference(&mut self, reference: &ExactReference) -> Result<()> {
        let mod_id = u64::from(reference.mod_id) as i64;
        let release_id = reference.release_id.0.to_string();
        let hash = reference.sha256.as_str();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO registry_library (mod_id, release_id, sha256) VALUES (?, ?, ?)",
            params![mod_id, release_id, hash],
        )?;
        let saved: String = tx.query_row(
            "SELECT sha256 FROM registry_library WHERE mod_id=? AND release_id=?",
            params![mod_id, release_id],
            |row| row.get(0),
        )?;
        if saved != hash {
            return Err(Error::Invalid(
                "A saved registry release has a different SHA-256. Keep the existing exact reference."
                    .into(),
            ));
        }
        tx.commit()?;
        Ok(())
    }

    pub fn registry_references(&self) -> Result<Vec<ExactReference>> {
        self.conn
            .prepare("SELECT mod_id, release_id, sha256 FROM registry_library ORDER BY mod_id, release_id")?
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })?
            .map(|row| {
                let (mod_id, release_id, hash) = row?;
                Ok(ExactReference {
                    mod_id: ModId::try_from(mod_id as u64)
                        .map_err(|error| Error::Invalid(error.into()))?,
                    release_id: ReleaseId(Uuid::parse_str(&release_id).map_err(|_| {
                        Error::Invalid("Saved registry ReleaseID is invalid.".into())
                    })?),
                    sha256: Sha256::try_from(hash)
                        .map_err(|error| Error::Invalid(error.into()))?,
                })
            })
            .collect()
    }

    pub fn has_registry_reference(&self, reference: &ExactReference) -> Result<bool> {
        let saved: Option<String> = self
            .conn
            .query_row(
                "SELECT sha256 FROM registry_library WHERE mod_id=? AND release_id=?",
                params![
                    u64::from(reference.mod_id) as i64,
                    reference.release_id.0.to_string()
                ],
                |row| row.get(0),
            )
            .optional()?;
        Ok(saved.is_some_and(|hash| hash == reference.sha256.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_registry_reference_survives_restart_and_cannot_change_hash() {
        let root = tempfile::tempdir().unwrap();
        let reference = ExactReference {
            mod_id: ModId::try_from(9_007_199_254_740_991).unwrap(),
            release_id: ReleaseId(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()),
            sha256: Sha256::try_from("a".repeat(64)).unwrap(),
        };
        {
            let mut store = Storage::open(root.path()).unwrap();
            store.save_registry_reference(&reference).unwrap();
            store.save_registry_reference(&reference).unwrap();
            let mut conflicting = reference.clone();
            conflicting.sha256 = Sha256::try_from("b".repeat(64)).unwrap();
            assert!(store.save_registry_reference(&conflicting).is_err());
            assert_eq!(
                store.registry_references().unwrap(),
                vec![reference.clone()]
            );
        }
        let store = Storage::open(root.path()).unwrap();
        assert!(store.has_registry_reference(&reference).unwrap());
        assert_eq!(store.registry_references().unwrap(), vec![reference]);
    }
}
