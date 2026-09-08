use super::*;
use crate::sharing::{Import, Portable};

impl Storage {
    pub fn collection_imports(&self) -> Result<Vec<Import>> {
        let mut statement = self
            .conn
            .prepare("SELECT record FROM collection_imports ORDER BY collection_id")?;
        statement
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|row| {
                serde_json::from_str(&row?)
                    .map_err(|e| Error::Invalid(format!("Invalid collection import: {e}")))
            })
            .collect()
    }

    pub(crate) fn create_import(
        &mut self,
        document: &Portable,
        import: &Import,
        expected: i64,
    ) -> Result<()> {
        document.validate().map_err(Error::Invalid)?;
        Uuid::parse_str(&import.collection_id)
            .map_err(|_| Error::Invalid("Invalid import ID.".into()))?;
        let record = serde_json::to_string(import).map_err(|e| Error::Invalid(e.to_string()))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        check_revision(&tx, expected)?;
        tx.execute(
            "INSERT INTO collections (id,name,revision) VALUES (?,?,1)",
            rusqlite::params![import.collection_id, document.name],
        )?;
        for (position, reference) in document.entries.iter().enumerate() {
            tx.execute("INSERT INTO collection_entries (collection_id,position,mod_id,hash,origin,release_id) VALUES (?,?,?,?,?,?)", rusqlite::params![import.collection_id, position as i64, reference.mod_id, reference.hash, reference.origin.as_str(), reference.release_id])?;
        }
        tx.execute(
            "INSERT INTO collection_imports (collection_id,record) VALUES (?,?)",
            rusqlite::params![import.collection_id, record],
        )?;
        bump(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn save_import(&mut self, import: &Import) -> Result<()> {
        let record = serde_json::to_string(import).map_err(|e| Error::Invalid(e.to_string()))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE collection_imports SET record=? WHERE collection_id=?",
            rusqlite::params![record, import.collection_id],
        )? != 1
        {
            return Err(Error::Invalid(
                "This imported collection no longer exists.".into(),
            ));
        }
        bump(&tx)?;
        tx.commit()?;
        Ok(())
    }
}
