use super::{Connection, Error, Result, Storage, TransactionBehavior};
use crate::catalog::{
    Catalog,
    advisories::{Advisories, MAX_BYTES},
    authentication::VerifiedCatalog,
    refresh::Cache,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogSecurity {
    catalog_sha256: String,
    received_at: u64,
    expires: u64,
    advisories: Advisories,
}

fn catalog_hash(catalog: &Catalog) -> Result<String> {
    let bytes = serde_json::to_vec(catalog).map_err(|e| Error::Invalid(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

impl CatalogSecurity {
    fn read(record: &str) -> Result<Self> {
        if record.len() > MAX_BYTES + 1024 {
            return Err(Error::Invalid(
                "Saved catalog security data is too large.".into(),
            ));
        }
        let value: Self =
            serde_json::from_str(record).map_err(|e| Error::Invalid(e.to_string()))?;
        if value.received_at >= value.expires
            || value.expires > 253_402_300_799
            || value.catalog_sha256.len() != 64
            || !value
                .catalog_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(Error::Invalid(
                "Saved catalog authentication is invalid.".into(),
            ));
        }
        value.advisories.validate().map_err(Error::Invalid)?;
        Ok(value)
    }

    pub fn advisories(&self) -> &Advisories {
        &self.advisories
    }

    pub fn expires(&self) -> u64 {
        self.expires
    }

    pub fn require_fresh(&self, catalog: &Catalog, now: u64) -> Result<()> {
        if now < self.received_at || now >= self.expires {
            return Err(Error::Invalid("Catalog security information has expired or the clock changed. New catalog downloads are paused until a signed refresh succeeds.".into()));
        }
        if catalog_hash(catalog)? != self.catalog_sha256 {
            return Err(Error::Invalid("This catalog does not match the authenticated security information. Wait for a signed refresh.".into()));
        }
        Ok(())
    }
}

fn cache(conn: &Connection) -> Result<Option<Cache>> {
    let record: Option<String> = conn
        .query_row("SELECT record FROM catalog_cache WHERE id=1", [], |r| {
            r.get(0)
        })
        .optional()?;
    record
        .map(|record| Cache::read(record.as_bytes()).map_err(Error::Invalid))
        .transpose()
}

fn security(conn: &Connection) -> Result<Option<CatalogSecurity>> {
    let record: Option<String> = conn
        .query_row("SELECT record FROM catalog_security WHERE id=1", [], |r| {
            r.get(0)
        })
        .optional()?;
    record
        .map(|record| CatalogSecurity::read(&record))
        .transpose()
}

fn write_cache(conn: &Connection, value: &Cache) -> Result<()> {
    let record = serde_json::to_string(value).map_err(|e| Error::Invalid(e.to_string()))?;
    Cache::read(record.as_bytes()).map_err(Error::Invalid)?;
    if let Some(old) = cache(conn)?.and_then(|cache| cache.catalog) {
        value
            .catalog
            .as_ref()
            .ok_or_else(|| Error::Invalid("Cannot discard the validated catalog cache.".into()))?
            .accepts_after(&old)
            .map_err(Error::Invalid)?;
    }
    conn.execute("INSERT INTO catalog_cache (id, record) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET record=excluded.record", [record])?;
    Ok(())
}

impl Storage {
    pub fn catalog_cache(&self) -> Result<Option<Cache>> {
        cache(&self.conn)
    }

    pub fn catalog_security(&self) -> Result<Option<CatalogSecurity>> {
        security(&self.conn)
    }

    pub fn save_catalog_cache(&mut self, value: &Cache) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(security) = security(&tx)? {
            let catalog = value
                .catalog
                .as_ref()
                .ok_or_else(|| Error::Invalid("Cannot discard an authenticated catalog.".into()))?;
            if catalog_hash(catalog)? != security.catalog_sha256 {
                return Err(Error::Invalid(
                    "An authenticated catalog can only be replaced by a signed refresh.".into(),
                ));
            }
        }
        write_cache(&tx, value)?;
        tx.commit()?;
        Ok(())
    }

    pub fn save_verified_catalog(&mut self, verified: &VerifiedCatalog, now: u64) -> Result<Cache> {
        let candidate = CatalogSecurity {
            catalog_sha256: catalog_hash(verified.catalog())?,
            received_at: now,
            expires: verified.expires(),
            advisories: verified.advisories().clone(),
        };
        let record =
            serde_json::to_string(&candidate).map_err(|e| Error::Invalid(e.to_string()))?;
        CatalogSecurity::read(&record)?;
        candidate.require_fresh(verified.catalog(), now)?;
        let value = Cache {
            catalog: Some(verified.catalog().clone()),
            last_checked: Some(now),
            last_success: Some(now),
            ..Default::default()
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(previous) = security(&tx)? {
            if now < previous.received_at {
                return Err(Error::Invalid(
                    "The clock moved backwards since the last authenticated catalog.".into(),
                ));
            }
            candidate
                .advisories
                .accepts_after(&previous.advisories)
                .map_err(Error::Invalid)?;
        }
        write_cache(&tx, &value)?;
        tx.execute("INSERT INTO catalog_security (id, record) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET record=excluded.record", [record])?;
        tx.commit()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
