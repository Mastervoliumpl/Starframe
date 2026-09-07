use super::{Catalog, ENDPOINT, MAX_BYTES};
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

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub retry_after: Option<u64>,
    pub bytes: Vec<u8>,
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

async fn fetch(client: &Client, endpoint: &str, cache: &Cache) -> Result<Response, String> {
    let mut request = client
        .get(endpoint)
        .header(header::ACCEPT, "application/json");
    if let Some(etag) = &cache.etag {
        request = request.header(header::IF_NONE_MATCH, etag);
    } else if let Some(modified) = &cache.last_modified {
        request = request.header(header::IF_MODIFIED_SINCE, modified);
    }
    let mut response = request.send().await.map_err(|e| {
        if e.is_timeout() {
            "Catalog request timed out. Starframe will retry automatically.".to_owned()
        } else {
            "Catalog request failed. Check your connection; Starframe will retry automatically."
                .to_owned()
        }
    })?;
    let validator = |name| -> Result<Option<String>, String> {
        response
            .headers()
            .get(name)
            .map(|value: &header::HeaderValue| {
                let value = value
                    .to_str()
                    .map_err(|_| "Invalid catalog HTTP validator.")?;
                if value.len() > 1024 {
                    return Err("Catalog HTTP validator is too long.".into());
                }
                Ok(value.to_owned())
            })
            .transpose()
    };
    let mut result = Response {
        status: response.status().as_u16(),
        etag: validator(header::ETAG)?,
        last_modified: validator(header::LAST_MODIFIED)?,
        retry_after: response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(|seconds| seconds.min(3600)),
        bytes: Vec::new(),
    };
    if result.status != 200 {
        return Ok(result);
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BYTES as u64)
    {
        return Err("Catalog response exceeds 2 MiB. The previous cache was retained.".into());
    }
    while let Some(chunk) = response.chunk().await.map_err(
        |_| "Catalog transfer stopped before completion. Starframe will retry automatically.",
    )? {
        if result.bytes.len() + chunk.len() > MAX_BYTES {
            return Err("Catalog response exceeds 2 MiB. The previous cache was retained.".into());
        }
        result.bytes.extend_from_slice(&chunk);
    }
    Ok(result)
}

type PendingResponse = (
    tokio::task::JoinHandle<()>,
    mpsc::Receiver<Result<Response, String>>,
);

pub struct Refresh {
    pub cache: Cache,
    pub checking: bool,
    pub next_check: u64,
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
        Ok(Self {
            cache: storage
                .catalog_cache()
                .map_err(|e| e.to_string())?
                .unwrap_or_default(),
            checking: false,
            next_check: 0,
            failures: 0,
            pending: None,
        })
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
        let client = match client() {
            Ok(client) => client,
            Err(error) => {
                self.complete(storage, Err(error), now);
                return;
            }
        };
        let cache = self.cache.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        self.checking = true;
        let task = tokio::spawn(async move {
            let _ = sender.send(fetch(&client, ENDPOINT, &cache).await);
        });
        self.pending = Some((task, receiver));
    }

    pub fn complete(
        &mut self,
        storage: &mut Storage,
        response: Result<Response, String>,
        now: u64,
    ) {
        self.checking = false;
        let retry_after = response
            .as_ref()
            .ok()
            .and_then(|r| r.retry_after)
            .unwrap_or(0);
        let mut candidate = self.cache.clone();
        candidate.last_checked = Some(now);
        let result = response.and_then(|response| {
            match response.status {
                200 => {
                    let catalog = Catalog::read(&response.bytes)?;
                    if let Some(previous) = &candidate.catalog {
                        catalog.accepts_after(previous)?;
                    }
                    candidate.catalog = Some(catalog);
                    candidate.etag = response.etag;
                    candidate.last_modified = response.last_modified;
                }
                304 if candidate.catalog.is_some()
                    && (candidate.etag.is_some() || candidate.last_modified.is_some()) =>
                {
                    if response.etag.is_some() {
                        candidate.etag = response.etag;
                    }
                    if response.last_modified.is_some() {
                        candidate.last_modified = response.last_modified;
                    }
                }
                status => {
                    return Err(format!(
                        "Catalog server returned HTTP {status}. Starframe will retry automatically."
                    ));
                }
            }
            candidate.last_success = Some(now);
            candidate.error = None;
            storage
                .save_catalog_cache(&candidate)
                .map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => {
                self.cache = candidate;
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
            (30u64.saturating_mul(1u64 << self.failures.min(7).saturating_sub(1)))
                .min(1800)
                .max(retry_after)
        };
        self.next_check = now.saturating_add(delay);
    }
}

#[cfg(test)]
mod tests;
