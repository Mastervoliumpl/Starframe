use super::{Catalog, MAX_BYTES};
use futures_util::stream;
use reqwest::{Client, Url};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tough::async_trait;
use tough::{
    IntoVec, RepositoryLoader, TargetName, Transport, TransportError, TransportErrorKind,
    TransportStream,
};

#[derive(Debug)]
pub struct VerifiedCatalog {
    catalog: Catalog,
    advisories: super::advisories::Advisories,
    expires: u64,
}

impl VerifiedCatalog {
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn expires(&self) -> u64 {
        self.expires
    }
    pub fn advisories(&self) -> &super::advisories::Advisories {
        &self.advisories
    }
}

#[derive(Clone, Debug)]
struct Http {
    client: Client,
    bases: [Url; 2],
    requests: Arc<AtomicUsize>,
}

#[async_trait]
impl Transport for Http {
    async fn fetch(&self, url: Url) -> Result<TransportStream, TransportError> {
        let error = || TransportError::new(TransportErrorKind::Other, url.as_str());
        if !self
            .bases
            .iter()
            .any(|base| url.origin() == base.origin() && url.path().starts_with(base.path()))
            || self.requests.fetch_add(1, Ordering::Relaxed) >= 40
        {
            return Err(error());
        }
        let mut response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|_| error())?;
        if response.status() == 404 {
            return Err(TransportError::new(
                TransportErrorKind::FileNotFound,
                url.as_str(),
            ));
        }
        if response.status() != 200
            || response
                .content_length()
                .is_some_and(|n| n > MAX_BYTES as u64)
        {
            return Err(error());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| error())? {
            if bytes.len() + chunk.len() > MAX_BYTES {
                return Err(error());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(Box::pin(stream::iter([Ok(bytes.into())])))
    }
}

pub async fn fetch(
    root: &[u8],
    datastore: &Path,
    metadata: Url,
    targets: Url,
) -> Result<VerifiedCatalog, String> {
    for base in [&metadata, &targets] {
        if base.scheme() != "https"
            || !base.path().ends_with('/')
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.username().is_empty()
            || base.password().is_some()
        {
            return Err(
                "Catalog trust endpoints require HTTPS directory URLs without credentials.".into(),
            );
        }
    }
    let transport = Http {
        client: super::refresh::client()?,
        bases: [metadata.clone(), targets.clone()],
        requests: Arc::new(AtomicUsize::new(0)),
    };
    tokio::time::timeout(
        Duration::from_secs(60),
        load(root, datastore, metadata, targets, transport),
    )
    .await
    .map_err(|_| {
        "Signed catalog refresh timed out; previous information was retained.".to_owned()
    })?
}

async fn load<T: Transport + 'static>(
    root: &[u8],
    datastore: &Path,
    metadata: Url,
    targets: Url,
    transport: T,
) -> Result<VerifiedCatalog, String> {
    if root.len() > 65_536 {
        return Err("Catalog trust root exceeds its size limit.".into());
    }
    let _directory = crate::filesystem::pin(datastore)?;
    for entry in std::fs::read_dir(datastore).map_err(|e| e.to_string())? {
        crate::filesystem::regular_metadata(&entry.map_err(|e| e.to_string())?.path(), false)?;
    }
    let mut trusted_root = root.to_vec();
    let cached_root = datastore.join("root.json");
    if cached_root.try_exists().map_err(|e| e.to_string())? {
        use std::io::Read;
        let file = crate::filesystem::read_file(&cached_root)?;
        let mut bytes = Vec::new();
        file.take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 65_536 {
            return Err("Cached trust root exceeds its size limit.".into());
        }
        let embedded: tough::schema::Signed<tough::schema::Root> =
            serde_json::from_slice(root).map_err(|e| e.to_string())?;
        let retained: tough::schema::Signed<tough::schema::Root> =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if retained.signed.version >= embedded.signed.version {
            trusted_root = bytes;
        }
    }
    let repository = RepositoryLoader::new(&trusted_root, metadata, targets)
        .datastore(datastore)
        .transport(transport)
        .limits(tough::Limits {
            max_root_size: 65_536,
            max_timestamp_size: 65_536,
            max_snapshot_size: 262_144,
            max_targets_size: 262_144,
            max_root_updates: 32,
        })
        .load()
        .await
        .map_err(|e| format!("Catalog authentication failed: {e}"))?;
    let expires = [
        repository.root().signed.expires,
        repository.timestamp().signed.expires,
        repository.snapshot().signed.expires,
        repository.targets().signed.expires,
    ]
    .into_iter()
    .map(|time| time.as_second())
    .min()
    .unwrap();
    let catalog = Catalog::read(&target_bytes(&repository, "catalog.json", MAX_BYTES).await?)?;
    let advisories = super::advisories::Advisories::read(
        &target_bytes(&repository, "advisories.json", super::advisories::MAX_BYTES).await?,
    )?;
    Ok(VerifiedCatalog {
        catalog,
        advisories,
        expires: expires.try_into().map_err(|_| "Invalid catalog expiry.")?,
    })
}

async fn target_bytes(
    repository: &tough::Repository,
    filename: &str,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let name = TargetName::new(filename).map_err(|e| e.to_string())?;
    let target = repository
        .targets()
        .signed
        .targets
        .get(&name)
        .ok_or_else(|| format!("Signed target {filename} is missing."))?;
    if target.length > limit as u64 {
        return Err(format!("Signed target {filename} exceeds its size limit."));
    }
    repository
        .read_target(&name)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Signed target {filename} is missing."))?
        .into_vec()
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) mod tests;
