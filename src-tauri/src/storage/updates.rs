use super::{Error, Result, Storage};
use crate::updates::Saved;
use rusqlite::OptionalExtension;

impl Storage {
    pub fn flush(&self) -> Result<()> {
        let busy: i64 = self
            .conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
        if busy != 0 {
            return Err(Error::Invalid(
                "Saved data is still busy. Retry the update shortly.".into(),
            ));
        }
        Ok(())
    }
    pub fn update_preferences(&self) -> Result<Saved> {
        let text: Option<String> = self
            .conn
            .query_row("SELECT record FROM app_updates WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        text.map(|s| Saved::read(&s).map_err(Error::Invalid))
            .transpose()
            .map(Option::unwrap_or_default)
    }
    pub fn save_update_preferences(&mut self, saved: &Saved) -> Result<()> {
        let text = serde_json::to_string(saved).map_err(|e| Error::Invalid(e.to_string()))?;
        Saved::read(&text).map_err(Error::Invalid)?;
        self.conn.execute("INSERT INTO app_updates(id,record) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record", [text])?;
        Ok(())
    }
}
