use super::*;
use crate::local_import::LocalSource;

impl Storage {
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
