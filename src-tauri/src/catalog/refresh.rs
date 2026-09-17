use super::{
    Catalog, MAX_BYTES,
    authentication::{self, VerifiedCatalog},
};
use crate::storage::Storage;
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::{sync::mpsc, time::Duration};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cache {
    pub catalog: Option<Catalog>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_checked: Option<u64>,
    pub last_success: Option<u64>,
    pub error: Option<String>,
}

impl Cache {
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES + 8192 {
            return Err("Catalog cache is too large.".into());
        }
        let cache: Self = serde_json::from_slice(bytes)
            .map_err(|e| format!("Could not read catalog cache: {e}"))?;
        if let Some(catalog) = &cache.catalog {
            catalog.validate()?;
        }
        for value in [&cache.etag, &cache.last_modified].into_iter().flatten() {
            if value.len() > 1024 || header::HeaderValue::from_str(value).is_err() {
                return Err("Invalid cached HTTP validator.".into());
            }
        }
        if cache.error.as_ref().is_some_and(|e| e.len() > 4096) {
            return Err("Invalid cached catalog error.".into());
        }
        if cache.catalog.is_none()
            && (cache.etag.is_some()
                || cache.last_modified.is_some()
                || cache.last_success.is_some())
        {
            return Err("Catalog cache has validators without a catalog.".into());
        }
        Ok(cache)
    }
}

pub fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent("Starframe catalog/1")
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

type PendingResponse = (
    tokio::task::JoinHandle<()>,
    mpsc::Receiver<Result<VerifiedCatalog, String>>,
);

pub struct Refresh {
    pub cache: Cache,
    pub checking: bool,
    pub next_check: u64,
    verified_window: Option<(u64, u64)>,
    failures: u32,
    pending: Option<PendingResponse>,
}

impl Drop for Refresh {
    fn drop(&mut self) {
        if let Some((task, _)) = self.pending.take() {
            task.abort();
        }
    }
}

impl Refresh {
    pub fn load(storage: &Storage) -> Result<Self, String> {
        let cache = storage
            .catalog_cache()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let security = storage.catalog_security().map_err(|e| e.to_string())?;
        let verified_window = match (&cache.catalog, security) {
            (Some(catalog), Some(security))
                if security
                    .matches_catalog(catalog)
                    .map_err(|e| e.to_string())? =>
            {
                Some((security.received_at(), security.expires()))
            }
            _ => None,
        };
        Ok(Self {
            cache,
            checking: false,
            next_check: 0,
            verified_window,
            failures: 0,
            pending: None,
        })
    }

    pub fn expires(&self) -> Option<u64> {
        self.verified_window.map(|(_, expires)| expires)
    }

    pub fn fresh(&self, now: u64) -> bool {
        self.verified_window
            .is_some_and(|(received, expires)| now >= received && now < expires)
    }

    pub fn tick(&mut self, storage: &mut Storage, now: u64, shell_ready: bool, stopped: bool) {
        if stopped {
            if let Some((task, _)) = self.pending.take() {
                task.abort();
            }
            self.checking = false;
            return;
        }
        if let Some((_, receiver)) = &self.pending {
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(_) => Err("Catalog worker stopped before returning a result.".into()),
            };
            self.pending.take();
            self.complete(storage, result, now);
            return;
        }
        if !shell_ready || now < self.next_check {
            return;
        }
        let datastore = match storage.catalog_trust_directory() {
            Ok(path) => path,
            Err(error) => {
                self.complete(storage, Err(error.to_string()), now);
                return;
            }
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        self.checking = true;
        let task = tokio::spawn(async move {
            let _ = sender.send(
                authentication::fetch(
                    authentication::ROOT,
                    &datastore,
                    authentication::METADATA
                        .parse()
                        .expect("fixed metadata URL"),
                    authentication::TARGETS.parse().expect("fixed targets URL"),
                )
                .await,
            );
        });
        self.pending = Some((task, receiver));
    }

    pub fn complete(
        &mut self,
        storage: &mut Storage,
        response: Result<VerifiedCatalog, String>,
        now: u64,
    ) {
        self.checking = false;
        let result = response.and_then(|verified| {
            let cache = storage
                .save_verified_catalog(&verified, now)
                .map_err(|e| e.to_string())?;
            Ok((cache, verified.expires()))
        });
        match result {
            Ok((cache, expires)) => {
                self.cache = cache;
                self.verified_window = Some((now, expires));
                self.failures = 0;
            }
            Err(error) => {
                self.failures = self.failures.saturating_add(1);
                self.cache.last_checked = Some(now);
                self.cache.error = Some(error.chars().take(1000).collect());
                if let Err(error) = storage.save_catalog_cache(&self.cache) {
                    self.cache.error = Some(format!("Catalog cache could not be saved: {error}"));
                }
            }
        }
        let delay = if self.failures == 0 {
            300
        } else {
            (30u64.saturating_mul(1u64 << self.failures.min(7).saturating_sub(1))).min(1800)
        };
        self.next_check = now.saturating_add(delay);
    }
}

#[cfg(test)]
mod tests;
