use super::{
    ApiResponse, ExactReference, ListQuery, ModList, ReleaseId, ReleaseResult, Session,
    SessionContext,
};
use reqwest::{StatusCode, Url, header};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::watch,
};

const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Config {
    api: Url,
    website: Url,
}

impl Config {
    pub fn new(api: &str, website: &str, test_loopback: bool) -> Result<Self, Error> {
        let api = Url::parse(api).map_err(|_| Error::Configuration)?;
        let website = Url::parse(website).map_err(|_| Error::Configuration)?;
        let allowed = |url: &Url, production_host: &str, path: &str| {
            let production = url.scheme() == "https"
                && url.host_str() == Some(production_host)
                && url.port_or_known_default() == Some(443);
            let loopback =
                test_loopback && url.scheme() == "http" && url.host_str() == Some("127.0.0.1");
            (production || loopback)
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && url.path() == path
        };
        if !allowed(&api, "api.starframemanager.com", "/v1")
            || !allowed(&website, "starframemanager.com", "/")
        {
            return Err(Error::Configuration);
        }
        Ok(Self { api, website })
    }

    pub fn website(&self) -> &Url {
        &self.website
    }
}

#[derive(Debug)]
pub enum Error {
    Configuration,
    InvalidPath,
    InvalidQuery,
    InvalidToken,
    Transport,
    Cancelled,
    TooLarge,
    Protocol,
    Integrity,
    Server(StatusCode, ResponseError),
    Http(StatusCode),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthenticated,
    Forbidden,
    NotFound,
    ValidationFailed,
    Conflict,
    RevisionConflict,
    ReleaseUnavailable,
    ReleaseBlocked,
    DependencyUnavailable,
    SecurityStale,
    RateLimited,
    ServiceUnavailable,
}

impl ErrorCode {
    fn matches_status(&self, status: StatusCode) -> bool {
        match self {
            Self::ValidationFailed => status == StatusCode::BAD_REQUEST,
            Self::Unauthenticated => status == StatusCode::UNAUTHORIZED,
            Self::Forbidden | Self::ReleaseBlocked => status == StatusCode::FORBIDDEN,
            Self::NotFound => status == StatusCode::NOT_FOUND,
            Self::Conflict
            | Self::RevisionConflict
            | Self::DependencyUnavailable
            | Self::SecurityStale => status == StatusCode::CONFLICT,
            Self::ReleaseUnavailable => status == StatusCode::GONE,
            Self::RateLimited => status == StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable => status == StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldProblem {
    pub field: String,
    pub code: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResponseError {
    pub code: ErrorCode,
    pub message: String,
    pub request_id: uuid::Uuid,
    pub retry_after_seconds: Option<u64>,
    pub problems: Vec<FieldProblem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ErrorEnvelope {
    pub(super) api_version: u8,
    pub(super) error: ResponseError,
}

pub struct Client {
    config: Config,
    http: reqwest::Client,
    content_http: reqwest::Client,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadGrant {
    pub download_id: uuid::Uuid,
    pub reference: ExactReference,
    pub bytes: u64,
    pub metadata_revision: u64,
    pub security_revision: u64,
    pub content_path: String,
    pub expires_at: String,
    #[serde(skip)]
    account_id: uuid::Uuid,
}

pub struct VerifiedArchive {
    grant: DownloadGrant,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptClaim {
    pub(crate) account_id: uuid::Uuid,
    pub(crate) download_id: uuid::Uuid,
    pub(crate) release_id: ReleaseId,
    pub(crate) sha256: super::Sha256,
    pub(crate) bytes: u64,
}

impl VerifiedArchive {
    pub(crate) fn receipt_claim(&self) -> ReceiptClaim {
        ReceiptClaim {
            account_id: self.grant.account_id,
            download_id: self.grant.download_id,
            release_id: self.grant.reference.release_id,
            sha256: self.grant.reference.sha256.clone(),
            bytes: self.grant.bytes,
        }
    }
}

impl ReceiptClaim {
    pub(crate) fn valid(&self) -> bool {
        (1..=MAX_ARCHIVE_BYTES).contains(&self.bytes)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DownloadReceipt {
    pub release_id: ReleaseId,
    pub counted: bool,
}

fn can_download(session: &Session) -> bool {
    matches!(session.context, SessionContext::Manager)
        && session
            .capabilities
            .iter()
            .any(|capability| matches!(capability, super::wire::Capability::DownloadMod))
        && OffsetDateTime::parse(&session.expires_at, &Rfc3339)
            .is_ok_and(|expires| expires > OffsetDateTime::now_utc())
}

impl Client {
    pub fn new(config: Config) -> Result<Self, Error> {
        let http = reqwest::Client::builder()
            .user_agent("Starframe registry/1")
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| Error::Configuration)?;
        let content_http = reqwest::Client::builder()
            .user_agent("Starframe registry/1")
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| Error::Configuration)?;
        Ok(Self {
            config,
            http,
            content_http,
        })
    }

    pub(super) fn website(&self) -> &Url {
        self.config.website()
    }

    pub(super) async fn session(
        &self,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<Option<Session>, Error> {
        let result: ApiResponse<Option<Session>> =
            self.get("/session", Some(bearer), cancel).await?;
        Ok(result.data)
    }

    pub(super) async fn revoke(
        &self,
        bearer: &str,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<(), Error> {
        if bearer.len() != 43
            || !bearer
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(Error::InvalidToken);
        }
        let mut url = self.config.api.clone();
        url.set_path("/v1/session");
        if *cancel.borrow() {
            return Err(Error::Cancelled);
        }
        let exchange = async {
            let response = self
                .http
                .delete(url)
                .bearer_auth(bearer)
                .send()
                .await
                .map_err(|_| Error::Transport)?;
            if response.status() != StatusCode::NO_CONTENT {
                return Err(Error::Http(response.status()));
            }
            if response.content_length().is_some_and(|size| size != 0) {
                return Err(Error::Protocol);
            }
            if !response
                .bytes()
                .await
                .map_err(|_| Error::Transport)?
                .is_empty()
            {
                return Err(Error::Protocol);
            }
            Ok(())
        };
        tokio::select! {
            result = exchange => result,
            _ = cancel.changed() => Err(Error::Cancelled),
        }
    }

    pub async fn list_mods(
        &self,
        query: &ListQuery,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<ModList, Error> {
        query.validate().map_err(|_| Error::InvalidQuery)?;
        let mut url = self.config.api.clone();
        url.set_path("/v1/registry/mods");
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("page", &query.page.to_string());
            pairs.append_pair("pageSize", &query.page_size.to_string());
            pairs.append_pair("query", &query.query);
            for tag in &query.include_tags {
                pairs.append_pair("includeTags", tag);
            }
            for tag in &query.exclude_tags {
                pairs.append_pair("excludeTags", tag);
            }
            pairs.append_pair("sort", query.sort.as_str());
            pairs.append_pair("period", query.period.as_str());
            pairs.append_pair("maintenance", query.maintenance.as_str());
            for build in &query.game_builds {
                pairs.append_pair("gameBuilds", build);
            }
        }
        let result: ModList = self.exchange(url, Some(bearer), cancel).await?;
        if !result.valid() {
            return Err(Error::Protocol);
        }
        Ok(result)
    }

    pub async fn release(
        &self,
        release_id: ReleaseId,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<ApiResponse<ReleaseResult>, Error> {
        let result: ApiResponse<ReleaseResult> = self
            .get(
                &format!("/registry/releases/{}", release_id.0),
                Some(bearer),
                cancel,
            )
            .await?;
        let returned_id = match &result.data {
            ReleaseResult::Release(release) => release.release_id,
            ReleaseResult::Tombstone(tombstone) => tombstone.release_id,
        };
        if returned_id != release_id || !result.data.valid() {
            return Err(Error::Protocol);
        }
        Ok(result)
    }

    pub async fn registry_keys(
        &self,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<Vec<u8>, Error> {
        self.signed("/registry/keys", bearer, cancel).await
    }

    pub async fn registry_security(
        &self,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<Vec<u8>, Error> {
        self.signed("/registry/security", bearer, cancel).await
    }

    pub async fn registry_manifest(
        &self,
        release_id: ReleaseId,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<Vec<u8>, Error> {
        self.signed(
            &format!("/registry/releases/{}/manifest", release_id.0),
            bearer,
            cancel,
        )
        .await
    }

    async fn signed(
        &self,
        path: &str,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<Vec<u8>, Error> {
        let value: serde_json::Value = self.get(path, Some(bearer), cancel).await?;
        serde_json::to_vec(&value).map_err(|_| Error::Protocol)
    }

    pub async fn download_grant(
        &self,
        identity: &super::trust::DownloadIdentity,
        session: &Session,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<DownloadGrant, Error> {
        if !can_download(session) {
            return Err(Error::InvalidToken);
        }
        let (status, response): (StatusCode, ApiResponse<DownloadGrant>) = self
            .post_with_bearer(
                &format!(
                    "/registry/releases/{}/downloads",
                    identity.reference.release_id.0
                ),
                &serde_json::json!({
                    "sha256": identity.reference.sha256.as_str(),
                    "metadataRevision": identity.metadata_revision,
                    "securityRevision": identity.security_revision,
                }),
                Some(bearer),
                cancel,
            )
            .await?;
        let mut grant = response.data;
        let expires =
            OffsetDateTime::parse(&grant.expires_at, &Rfc3339).map_err(|_| Error::Protocol)?;
        let now = OffsetDateTime::now_utc();
        if status != StatusCode::CREATED
            || grant.reference != identity.reference
            || grant.bytes != identity.bytes
            || grant.bytes > MAX_ARCHIVE_BYTES
            || grant.metadata_revision != identity.metadata_revision
            || grant.security_revision != identity.security_revision
            || grant.content_path != format!("/v1/downloads/{}/content", grant.download_id)
            || expires <= now
            || expires > now + time::Duration::minutes(16)
        {
            return Err(Error::Protocol);
        }
        grant.account_id = session.account_id;
        Ok(grant)
    }

    pub async fn download_content(
        &self,
        grant: &DownloadGrant,
        bearer: &str,
        file: &mut tokio::fs::File,
        start: u64,
        progress: &AtomicU64,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<VerifiedArchive, Error> {
        if bearer.is_empty()
            || bearer.len() > 4096
            || grant.bytes == 0
            || grant.bytes > MAX_ARCHIVE_BYTES
            || start >= grant.bytes
            || grant.content_path != format!("/v1/downloads/{}/content", grant.download_id)
            || file.metadata().await.map_err(|_| Error::Integrity)?.len() != start
            || OffsetDateTime::parse(&grant.expires_at, &Rfc3339).map_err(|_| Error::Protocol)?
                <= OffsetDateTime::now_utc()
        {
            return Err(Error::Protocol);
        }
        if *cancel.borrow() {
            return Err(Error::Cancelled);
        }
        let mut url = self.config.api.clone();
        url.set_path(&grant.content_path);
        let etag = format!("\"sha256-{}\"", grant.reference.sha256.as_str());
        let mut request = self
            .content_http
            .get(url)
            .bearer_auth(bearer)
            .header(header::ACCEPT_ENCODING, "identity");
        if start > 0 {
            request = request
                .header(header::RANGE, format!("bytes={start}-"))
                .header(header::IF_RANGE, &etag);
        }
        let exchange = async {
            let mut response = request.send().await.map_err(|_| Error::Transport)?;
            let status = response.status();
            if status != StatusCode::OK && status != StatusCode::PARTIAL_CONTENT {
                return Err(Error::Http(status));
            }
            let offset = if status == StatusCode::OK { 0 } else { start };
            let expected_length = grant.bytes - offset;
            let expected_range = format!("bytes {offset}-{}/{}", grant.bytes - 1, grant.bytes);
            if response
                .headers()
                .get(header::ETAG)
                .and_then(|v| v.to_str().ok())
                != Some(etag.as_str())
                || response
                    .headers()
                    .get(header::ACCEPT_RANGES)
                    .and_then(|v| v.to_str().ok())
                    != Some("bytes")
                || response.content_length() != Some(expected_length)
                || response
                    .headers()
                    .get(header::CONTENT_ENCODING)
                    .is_some_and(|v| v != "identity")
                || (status == StatusCode::PARTIAL_CONTENT
                    && response
                        .headers()
                        .get(header::CONTENT_RANGE)
                        .and_then(|v| v.to_str().ok())
                        != Some(expected_range.as_str()))
                || (status == StatusCode::OK
                    && response.headers().contains_key(header::CONTENT_RANGE))
            {
                return Err(Error::Protocol);
            }
            if offset == 0 {
                file.set_len(0).await.map_err(|_| Error::Integrity)?;
            }
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|_| Error::Integrity)?;
            progress.store(offset, Ordering::SeqCst);
            let mut received = offset;
            while let Some(chunk) = response.chunk().await.map_err(|_| Error::Transport)? {
                received = received
                    .checked_add(chunk.len() as u64)
                    .ok_or(Error::TooLarge)?;
                if received > grant.bytes {
                    return Err(Error::TooLarge);
                }
                file.write_all(&chunk).await.map_err(|_| Error::Integrity)?;
                progress.store(received, Ordering::SeqCst);
            }
            if received != grant.bytes {
                return Err(Error::Transport);
            }
            file.flush().await.map_err(|_| Error::Integrity)?;
            file.seek(std::io::SeekFrom::Start(0))
                .await
                .map_err(|_| Error::Integrity)?;
            let mut digest = Sha256::new();
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = file.read(&mut buffer).await.map_err(|_| Error::Integrity)?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
            if format!("{:x}", digest.finalize()) != grant.reference.sha256.as_str() {
                return Err(Error::Integrity);
            }
            file.sync_all().await.map_err(|_| Error::Integrity)?;
            Ok(VerifiedArchive {
                grant: grant.clone(),
            })
        };
        tokio::select! {
            result = exchange => result,
            _ = cancel.changed() => Err(Error::Cancelled),
        }
    }

    pub async fn download_with_renewal(
        &self,
        identity: &super::trust::DownloadIdentity,
        session: &Session,
        bearer: &str,
        file: &mut tokio::fs::File,
        progress: &AtomicU64,
        cancel: watch::Receiver<bool>,
    ) -> Result<(DownloadGrant, VerifiedArchive), Error> {
        if !can_download(session) {
            return Err(Error::InvalidToken);
        }
        file.set_len(0).await.map_err(|_| Error::Integrity)?;
        progress.store(0, Ordering::SeqCst);
        for attempt in 0..3 {
            let grant = self
                .download_grant(identity, session, bearer, cancel.clone())
                .await?;
            let mut start = file.metadata().await.map_err(|_| Error::Integrity)?.len();
            if start >= grant.bytes {
                file.set_len(0).await.map_err(|_| Error::Integrity)?;
                start = 0;
            }
            let mut restarted = false;
            loop {
                match self
                    .download_content(&grant, bearer, file, start, progress, cancel.clone())
                    .await
                {
                    Ok(verified) => return Ok((grant, verified)),
                    Err(Error::Http(StatusCode::CONFLICT | StatusCode::RANGE_NOT_SATISFIABLE))
                        if start > 0 && !restarted =>
                    {
                        file.set_len(0).await.map_err(|_| Error::Integrity)?;
                        progress.store(0, Ordering::SeqCst);
                        start = 0;
                        restarted = true;
                    }
                    Err(Error::Transport | Error::Http(StatusCode::GONE)) if attempt < 2 => break,
                    Err(error) => return Err(error),
                }
            }
        }
        Err(Error::Transport)
    }

    pub async fn confirm_download_session(
        &self,
        original: &Session,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<Session, Error> {
        let current = self
            .session(bearer, cancel)
            .await?
            .ok_or(Error::InvalidToken)?;
        if current.account_id != original.account_id || !can_download(&current) {
            return Err(Error::InvalidToken);
        }
        Ok(current)
    }

    pub async fn download_receipt(
        &self,
        verified: &VerifiedArchive,
        session: &Session,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<DownloadReceipt, Error> {
        self.retry_receipt(&verified.receipt_claim(), session, bearer, cancel)
            .await
    }

    pub async fn retry_receipt(
        &self,
        claim: &ReceiptClaim,
        session: &Session,
        bearer: &str,
        cancel: watch::Receiver<bool>,
    ) -> Result<DownloadReceipt, Error> {
        if !can_download(session) || session.account_id != claim.account_id {
            return Err(Error::InvalidToken);
        }
        let (status, response): (StatusCode, ApiResponse<DownloadReceipt>) = self
            .post_with_bearer(
                &format!("/registry/releases/{}/receipts", claim.release_id.0),
                &serde_json::json!({
                    "downloadId": claim.download_id,
                    "sha256": claim.sha256.as_str(),
                    "bytes": claim.bytes,
                }),
                Some(bearer),
                cancel,
            )
            .await?;
        if status != StatusCode::OK || response.data.release_id != claim.release_id {
            return Err(Error::Protocol);
        }
        Ok(response.data)
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        bearer: Option<&str>,
        cancel: watch::Receiver<bool>,
    ) -> Result<T, Error> {
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.contains(['?', '#', '\\'])
            || path.split('/').any(|part| part == ".." || part == ".")
        {
            return Err(Error::InvalidPath);
        }
        let mut url = self.config.api.clone();
        url.set_path(&format!("/v1{path}"));
        self.exchange(url, bearer, cancel).await
    }

    async fn exchange<T: DeserializeOwned>(
        &self,
        url: Url,
        bearer: Option<&str>,
        cancel: watch::Receiver<bool>,
    ) -> Result<T, Error> {
        self.exchange_request(self.http.get(url), bearer, cancel)
            .await
            .map(|(_, value)| value)
    }

    pub(super) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        cancel: watch::Receiver<bool>,
    ) -> Result<(StatusCode, T), Error> {
        self.post_with_bearer(path, body, None, cancel).await
    }

    async fn post_with_bearer<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        bearer: Option<&str>,
        cancel: watch::Receiver<bool>,
    ) -> Result<(StatusCode, T), Error> {
        let bytes = serde_json::to_vec(body).map_err(|_| Error::Protocol)?;
        if bytes.len() > 64 * 1024 {
            return Err(Error::TooLarge);
        }
        let mut url = self.config.api.clone();
        url.set_path(&format!("/v1{path}"));
        self.exchange_request(
            self.http
                .post(url)
                .header(header::CONTENT_TYPE, "application/json")
                .body(bytes),
            bearer,
            cancel,
        )
        .await
    }

    async fn exchange_request<T: DeserializeOwned>(
        &self,
        mut request: reqwest::RequestBuilder,
        bearer: Option<&str>,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<(StatusCode, T), Error> {
        request = request.header(header::ACCEPT, "application/json");
        if let Some(token) = bearer {
            if token.is_empty() || token.len() > 4096 {
                return Err(Error::InvalidToken);
            }
            request = request.bearer_auth(token);
        }
        if *cancel.borrow() {
            return Err(Error::Cancelled);
        }
        let exchange = async {
            let mut response = request.send().await.map_err(|_| Error::Transport)?;
            let status = response.status();
            if status.is_redirection() {
                return Err(Error::Http(status));
            }
            let retry_after = response.headers().get(header::RETRY_AFTER).cloned();
            if response
                .content_length()
                .is_some_and(|size| size > MAX_JSON_BYTES as u64)
            {
                return Err(Error::TooLarge);
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| Error::Transport)? {
                if bytes.len().saturating_add(chunk.len()) > MAX_JSON_BYTES {
                    return Err(Error::TooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            let value =
                crate::runtime_contract::unique_json(&bytes).map_err(|_| Error::Protocol)?;
            if status.is_success() {
                serde_json::from_value(value)
                    .map(|value| (status, value))
                    .map_err(|_| Error::Protocol)
            } else if let Ok(error) = serde_json::from_value::<ErrorEnvelope>(value) {
                if error.api_version != 1
                    || !error.error.code.matches_status(status)
                    || error.error.message.is_empty()
                    || error.error.message.len() > 1000
                    || error.error.problems.len() > 20
                    || error.error.problems.iter().any(|problem| {
                        !(1..=100).contains(&problem.field.len())
                            || !(1..=100).contains(&problem.code.len())
                    })
                    || error
                        .error
                        .retry_after_seconds
                        .is_some_and(|n| !(1..=86400).contains(&n))
                {
                    return Err(Error::Protocol);
                }
                if let Some(header) = retry_after {
                    let value = header.to_str().ok().and_then(|s| s.parse::<u64>().ok());
                    if value != error.error.retry_after_seconds {
                        return Err(Error::Protocol);
                    }
                }
                Err(Error::Server(status, error.error))
            } else {
                Err(Error::Protocol)
            }
        };
        tokio::select! {
            result = exchange => result,
            _ = cancel.changed() => Err(Error::Cancelled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{ApiResponse, ReleaseResult};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::AtomicU64;

    fn fixture(name: &str) -> String {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/registry-v1.json")).unwrap();
        serde_json::to_string(
            &fixtures
                .as_array()
                .unwrap()
                .iter()
                .find(|value| value["name"] == name)
                .unwrap()["body"],
        )
        .unwrap()
    }

    fn manager_session() -> Session {
        let mut value: serde_json::Value = serde_json::from_str(&fixture("session")).unwrap();
        value["data"]["expiresAt"] = (OffsetDateTime::now_utc() + time::Duration::hours(1))
            .format(&Rfc3339)
            .unwrap()
            .into();
        serde_json::from_value(value["data"].clone()).unwrap()
    }

    #[tokio::test]
    async fn completion_session_keeps_the_original_account_and_download_capability() {
        let original = manager_session();
        for (current, accepted) in [
            (original.clone(), true),
            (
                Session {
                    account_id: uuid::Uuid::new_v4(),
                    ..original.clone()
                },
                false,
            ),
            (
                Session {
                    capabilities: Vec::new(),
                    ..original.clone()
                },
                false,
            ),
        ] {
            let body = serde_json::json!({"apiVersion": 1, "data": current}).to_string();
            let (origin, thread) = serve("200 OK", &body, "");
            let client = Client::new(
                Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap(),
            )
            .unwrap();
            let (_sender, cancel) = watch::channel(false);
            let result = client
                .confirm_download_session(&original, "fixture-token", cancel)
                .await;
            assert_eq!(result.is_ok(), accepted);
            let request = thread.join().unwrap();
            assert!(request.starts_with("GET /v1/session HTTP/1.1"));
        }
    }

    fn read_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut buffer = [0; 8192];
            let n = stream.read(&mut buffer).unwrap();
            assert!(n > 0);
            request.extend_from_slice(&buffer[..n]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&request).to_string()
    }

    fn serve(
        status: &str,
        body: &str,
        extra_headers: &str,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let body = body.to_owned();
        let status = status.to_owned();
        let extra_headers = extra_headers.to_owned();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            request
        });
        (origin, thread)
    }

    #[test]
    fn origin_policy_is_exact() {
        assert!(
            Config::new(
                "https://api.starframemanager.com/v1",
                "https://starframemanager.com/",
                false
            )
            .is_ok()
        );
        assert!(Config::new("http://127.0.0.1:3000/v1", "http://127.0.0.1:3001/", true).is_ok());
        for api in [
            "http://api.starframemanager.com/v1",
            "https://api.starframemanager.com.evil/v1",
            "https://api.starframemanager.com/v1/extra",
            "https://user@api.starframemanager.com/v1",
            "http://localhost:3000/v1",
        ] {
            assert!(Config::new(api, "https://starframemanager.com/", true).is_err());
        }
    }

    #[tokio::test]
    async fn signed_registry_routes_keep_manager_bearer_on_the_api_origin() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/registry-keys-v1.json"
        ))
        .unwrap();
        let release_id =
            ReleaseId(uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        for (route, body) in [
            ("keys", &fixture["envelope"]),
            ("security", &fixture["securityEnvelope"]),
            ("manifest", &fixture["releaseEnvelope"]),
        ] {
            let (origin, thread) = serve("200 OK", &body.to_string(), "");
            let client = Client::new(
                Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap(),
            )
            .unwrap();
            let (_sender, cancel) = watch::channel(false);
            let response = match route {
                "keys" => client.registry_keys("fixture-token", cancel).await,
                "security" => client.registry_security("fixture-token", cancel).await,
                _ => {
                    client
                        .registry_manifest(release_id, "fixture-token", cancel)
                        .await
                }
            }
            .unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&response).unwrap(),
                *body
            );
            let request = thread.join().unwrap();
            let path = match route {
                "manifest" => format!("/v1/registry/releases/{}/manifest", release_id.0),
                _ => format!("/v1/registry/{route}"),
            };
            assert!(request.starts_with(&format!("GET {path} HTTP/1.1")));
            assert!(request.contains("fixture-token"));
        }
    }

    #[tokio::test]
    async fn download_grant_matches_exact_signed_identity_and_relative_content_path() {
        let release_id =
            ReleaseId(uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        let identity = super::super::trust::DownloadIdentity {
            reference: ExactReference {
                mod_id: super::super::ModId::try_from(1).unwrap(),
                release_id,
                sha256: super::super::Sha256::try_from("b".repeat(64)).unwrap(),
            },
            bytes: 2_147_483_648,
            metadata_revision: 1,
            security_revision: 1,
        };
        let download_id = uuid::Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
        let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(5))
            .format(&Rfc3339)
            .unwrap();
        let body = serde_json::json!({"apiVersion":1,"data":{
            "downloadId":download_id,"reference":identity.reference,
            "bytes":identity.bytes,"metadataRevision":identity.metadata_revision,
            "securityRevision":identity.security_revision,
            "contentPath":format!("/v1/downloads/{download_id}/content"),"expiresAt":expires
        }});
        let (origin, thread) = serve("201 Created", &body.to_string(), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        let session = manager_session();
        let grant = client
            .download_grant(&identity, &session, "fixture-token", cancel)
            .await
            .unwrap();
        assert_eq!(grant.download_id, download_id);
        let request = thread.join().unwrap();
        assert!(request.starts_with(&format!(
            "POST /v1/registry/releases/{}/downloads HTTP/1.1",
            release_id.0
        )));
        assert!(request.contains("fixture-token"));
        assert!(request.contains(&format!("\"sha256\":\"{}\"", "b".repeat(64))));
        assert!(request.contains("\"metadataRevision\":1"));
        assert!(request.contains("\"securityRevision\":1"));

        let mut changed = body;
        changed["data"]["contentPath"] = "https://example.invalid/content".into();
        let (origin, thread) = serve("201 Created", &changed.to_string(), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client
                .download_grant(&identity, &session, "fixture-token", cancel)
                .await,
            Err(Error::Protocol)
        ));
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn content_stream_restarts_resumes_and_rejects_changed_bytes() {
        let archive = "PK\u{0003}\u{0004}fixture archive bytes";
        let hash = format!("{:x}", Sha256::digest(archive.as_bytes()));
        let download_id = uuid::Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
        let grant = DownloadGrant {
            download_id,
            reference: ExactReference {
                mod_id: super::super::ModId::try_from(1).unwrap(),
                release_id: ReleaseId(
                    uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap(),
                ),
                sha256: super::super::Sha256::try_from(hash.clone()).unwrap(),
            },
            bytes: archive.len() as u64,
            metadata_revision: 1,
            security_revision: 1,
            content_path: format!("/v1/downloads/{download_id}/content"),
            expires_at: (OffsetDateTime::now_utc() + time::Duration::minutes(5))
                .format(&Rfc3339)
                .unwrap(),
            account_id: manager_session().account_id,
        };
        let headers = format!("ETag: \"sha256-{hash}\"\r\nAccept-Ranges: bytes\r\n");
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("part.zip");
        let progress = AtomicU64::new(0);
        let (origin, thread) = serve("200 OK", archive, &headers);
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .await
            .unwrap();
        let (_sender, cancel) = watch::channel(false);
        let verified = client
            .download_content(&grant, "fixture-token", &mut file, 0, &progress, cancel)
            .await
            .unwrap();
        assert_eq!(tokio::fs::read(&path).await.unwrap(), archive.as_bytes());
        assert_eq!(progress.load(Ordering::SeqCst), grant.bytes);
        let request = thread.join().unwrap();
        assert!(request.starts_with(&format!("GET {} HTTP/1.1", grant.content_path)));
        assert!(!request.contains("Range: bytes="));

        let data = root.path().join("receipt-data");
        let mut store = crate::storage::Storage::open(&data).unwrap();
        let claim = store.record_registry_receipt(&verified).unwrap();
        assert_eq!(claim.download_id, grant.download_id);
        drop(store);
        let mut store = crate::storage::Storage::open(&data).unwrap();
        let session = manager_session();
        let mut other = session.clone();
        other.account_id = uuid::Uuid::new_v4();
        assert!(
            store
                .pending_registry_receipts(other.account_id)
                .unwrap()
                .is_empty()
        );
        let pending = store.pending_registry_receipts(session.account_id).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].download_id, grant.download_id);

        let (origin, thread) = serve("503 Service Unavailable", "", "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(
            client
                .retry_receipt(&pending[0], &session, "fixture-token", cancel)
                .await
                .is_err()
        );
        thread.join().unwrap();
        assert_eq!(
            store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .len(),
            1
        );

        let receipt_body = serde_json::json!({"apiVersion":1,"data":{
            "releaseId":grant.reference.release_id,"counted":true
        }});
        let (origin, thread) = serve("200 OK", &receipt_body.to_string(), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(
            client
                .retry_receipt(&pending[0], &session, "fixture-token", cancel)
                .await
                .unwrap()
                .counted
        );
        store.clear_registry_receipt(&pending[0]).unwrap();
        assert!(
            store
                .pending_registry_receipts(session.account_id)
                .unwrap()
                .is_empty()
        );
        let request = thread.join().unwrap();
        assert!(request.starts_with(&format!(
            "POST /v1/registry/releases/{}/receipts HTTP/1.1",
            grant.reference.release_id.0
        )));
        assert!(request.contains(&grant.download_id.to_string()));
        assert!(request.contains(&hash));
        let duplicate = serde_json::json!({"apiVersion":1,"data":{
            "releaseId":grant.reference.release_id,"counted":false
        }});
        let (origin, thread) = serve("200 OK", &duplicate.to_string(), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(
            !client
                .download_receipt(&verified, &session, "fixture-token", cancel)
                .await
                .unwrap()
                .counted
        );
        thread.join().unwrap();
        let mut other = manager_session();
        other.account_id = uuid::Uuid::new_v4();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client
                .download_receipt(&verified, &other, "fixture-token", cancel)
                .await,
            Err(Error::InvalidToken)
        ));

        let prefix = 7;
        tokio::fs::write(&path, &archive.as_bytes()[..prefix])
            .await
            .unwrap();
        let range_headers = format!(
            "{headers}Content-Range: bytes {prefix}-{}/{}\r\n",
            grant.bytes - 1,
            grant.bytes
        );
        let (origin, thread) = serve("206 Partial Content", &archive[prefix..], &range_headers);
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        client
            .download_content(
                &grant,
                "fixture-token",
                &mut file,
                prefix as u64,
                &progress,
                cancel,
            )
            .await
            .unwrap();
        assert_eq!(tokio::fs::read(&path).await.unwrap(), archive.as_bytes());
        let request = thread.join().unwrap();
        assert!(request.contains(&format!("bytes={prefix}-")));
        assert!(request.contains(&format!("\"sha256-{hash}\"")));

        tokio::fs::write(&path, b"old").await.unwrap();
        let (origin, thread) = serve("200 OK", archive, &headers);
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        client
            .download_content(&grant, "fixture-token", &mut file, 3, &progress, cancel)
            .await
            .unwrap();
        assert_eq!(tokio::fs::read(&path).await.unwrap(), archive.as_bytes());
        thread.join().unwrap();

        tokio::fs::write(&path, b"").await.unwrap();
        let changed = "X".repeat(archive.len());
        let (origin, thread) = serve("200 OK", &changed, &headers);
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client
                .download_content(&grant, "fixture-token", &mut file, 0, &progress, cancel)
                .await,
            Err(Error::Integrity)
        ));
        thread.join().unwrap();

        tokio::fs::write(&path, b"").await.unwrap();
        let (origin, thread) = serve(
            "416 Range Not Satisfiable",
            "",
            &format!("Content-Range: bytes */{}\r\n", grant.bytes),
        );
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client
                .download_content(&grant, "fixture-token", &mut file, 0, &progress, cancel)
                .await,
            Err(Error::Http(StatusCode::RANGE_NOT_SATISFIABLE))
        ));
        thread.join().unwrap();
        assert!(tokio::fs::read(&path).await.unwrap().is_empty());

        let (sender, cancel) = watch::channel(false);
        sender.send_replace(true);
        assert!(matches!(
            client
                .download_content(&grant, "fixture-token", &mut file, 0, &progress, cancel)
                .await,
            Err(Error::Cancelled)
        ));
    }

    #[tokio::test]
    async fn representative_stream_uses_bounded_chunks_and_reaches_signed_size() {
        const CHUNK: usize = 64 * 1024;
        const CHUNKS: usize = 512;
        let bytes = CHUNK * CHUNKS;
        let block = [0x5au8; CHUNK];
        let mut digest = Sha256::new();
        for _ in 0..CHUNKS {
            digest.update(block);
        }
        let hash = format!("{:x}", digest.finalize());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let response_hash = hash.clone();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0u8; 2048];
            assert!(stream.read(&mut request).unwrap() > 0);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {bytes}\r\nETag: \"sha256-{response_hash}\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            for _ in 0..CHUNKS {
                stream.write_all(&block).unwrap();
            }
        });
        let download_id = uuid::Uuid::new_v4();
        let grant = DownloadGrant {
            download_id,
            reference: ExactReference {
                mod_id: super::super::ModId::try_from(1).unwrap(),
                release_id: ReleaseId(uuid::Uuid::new_v4()),
                sha256: super::super::Sha256::try_from(hash).unwrap(),
            },
            bytes: bytes as u64,
            metadata_revision: 1,
            security_revision: 1,
            content_path: format!("/v1/downloads/{download_id}/content"),
            expires_at: (OffsetDateTime::now_utc() + time::Duration::minutes(5))
                .format(&Rfc3339)
                .unwrap(),
            account_id: manager_session().account_id,
        };
        let root = tempfile::tempdir().unwrap();
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(root.path().join("large.part"))
            .await
            .unwrap();
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let progress = AtomicU64::new(0);
        let (_sender, cancel) = watch::channel(false);
        client
            .download_content(&grant, "fixture-token", &mut file, 0, &progress, cancel)
            .await
            .unwrap();
        thread.join().unwrap();
        assert_eq!(file.metadata().await.unwrap().len(), bytes as u64);
        assert_eq!(progress.load(Ordering::SeqCst), bytes as u64);
    }

    #[tokio::test]
    async fn interrupted_transfer_renews_grant_and_restarts_after_unrecorded_prefix() {
        let archive = "PK\u{0003}\u{0004}resume this complete archive";
        let prefix = 9;
        let hash = format!("{:x}", Sha256::digest(archive.as_bytes()));
        let release_id =
            ReleaseId(uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        let identity = super::super::trust::DownloadIdentity {
            reference: ExactReference {
                mod_id: super::super::ModId::try_from(1).unwrap(),
                release_id,
                sha256: super::super::Sha256::try_from(hash.clone()).unwrap(),
            },
            bytes: archive.len() as u64,
            metadata_revision: 1,
            security_revision: 1,
        };
        let ids = [
            uuid::Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
            uuid::Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
        ];
        let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(5))
            .format(&Rfc3339)
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let reference = identity.reference.clone();
        let expected_archive = archive.to_string();
        let expected_second_id = ids[1];
        let archive = archive.to_string();
        let thread = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for step in 0..5 {
                let (mut stream, _) = listener.accept().unwrap();
                requests.push(read_request(&mut stream));
                match step {
                    0 | 2 => {
                        let id = ids[step / 2];
                        let body = serde_json::json!({"apiVersion":1,"data":{
                            "downloadId":id,"reference":reference,"bytes":archive.len(),
                            "metadataRevision":1,"securityRevision":1,
                            "contentPath":format!("/v1/downloads/{id}/content"),
                            "expiresAt":expires
                        }})
                        .to_string();
                        write!(stream, "HTTP/1.1 201 Created\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                    }
                    1 => {
                        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"sha256-{hash}\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n", archive.len()).unwrap();
                        stream.write_all(&archive.as_bytes()[..prefix]).unwrap();
                    }
                    3 => {
                        write!(stream, "HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    }
                    _ => {
                        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"sha256-{hash}\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n{archive}", archive.len()).unwrap();
                    }
                }
            }
            requests
        });
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("renewed.part");
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .await
            .unwrap();
        let progress = AtomicU64::new(0);
        let (_sender, cancel) = watch::channel(false);
        let (grant, _) = client
            .download_with_renewal(
                &identity,
                &manager_session(),
                "fixture-token",
                &mut file,
                &progress,
                cancel,
            )
            .await
            .unwrap();
        assert_eq!(grant.download_id, expected_second_id);
        assert_eq!(
            tokio::fs::read(path).await.unwrap(),
            expected_archive.as_bytes()
        );
        let requests = thread.join().unwrap();
        assert_eq!(requests.len(), 5);
        assert!(requests[3].contains(&format!("bytes={prefix}-")));
        assert!(!requests[4].contains("range:"));
    }

    #[tokio::test]
    async fn interrupted_transfer_resumes_a_server_recorded_prefix() {
        let archive = b"PK\x03\x04resume a recorded prefix";
        let prefix = 9;
        let hash = format!("{:x}", Sha256::digest(archive));
        let release_id =
            ReleaseId(uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        let identity = super::super::trust::DownloadIdentity {
            reference: ExactReference {
                mod_id: super::super::ModId::try_from(1).unwrap(),
                release_id,
                sha256: super::super::Sha256::try_from(hash.clone()).unwrap(),
            },
            bytes: archive.len() as u64,
            metadata_revision: 1,
            security_revision: 1,
        };
        let ids = [
            uuid::Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
            uuid::Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
        ];
        let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(5))
            .format(&Rfc3339)
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let reference = identity.reference.clone();
        let thread = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for step in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                requests.push(read_request(&mut stream));
                match step {
                    0 | 2 => {
                        let id = ids[step / 2];
                        let body = serde_json::json!({"apiVersion":1,"data":{
                            "downloadId":id,"reference":reference,"bytes":archive.len(),
                            "metadataRevision":1,"securityRevision":1,
                            "contentPath":format!("/v1/downloads/{id}/content"),
                            "expiresAt":expires
                        }})
                        .to_string();
                        write!(stream, "HTTP/1.1 201 Created\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                    }
                    1 => {
                        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"sha256-{hash}\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n", archive.len()).unwrap();
                        stream.write_all(&archive[..prefix]).unwrap();
                    }
                    _ => {
                        write!(stream, "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {prefix}-{}/{}\r\nETag: \"sha256-{hash}\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n", archive.len() - prefix, archive.len() - 1, archive.len()).unwrap();
                        stream.write_all(&archive[prefix..]).unwrap();
                    }
                }
            }
            requests
        });
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("resumed.part");
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .await
            .unwrap();
        let progress = AtomicU64::new(0);
        let (_sender, cancel) = watch::channel(false);
        let (grant, _) = client
            .download_with_renewal(
                &identity,
                &manager_session(),
                "fixture-token",
                &mut file,
                &progress,
                cancel,
            )
            .await
            .unwrap();
        assert_eq!(grant.download_id, ids[1]);
        assert_eq!(tokio::fs::read(path).await.unwrap(), archive);
        let requests = thread.join().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(requests[3].contains(&format!("bytes={prefix}-")));
    }

    #[tokio::test]
    async fn reads_pinned_release_without_leaking_token_to_redirects() {
        let (origin, thread) = serve("200 OK", &fixture("approved"), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        let result = client
            .release(
                ReleaseId(uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()),
                "fixture-token",
                cancel,
            )
            .await
            .unwrap();
        assert!(matches!(result.data, ReleaseResult::Release(_)));
        let request = thread.join().unwrap();
        assert!(request.starts_with(
            "GET /v1/registry/releases/11111111-1111-4111-8111-111111111111 HTTP/1.1"
        ));
        assert!(
            request.contains("authorization: Bearer fixture-token")
                || request.contains("Authorization: Bearer fixture-token")
        );

        let (origin, thread) = serve(
            "302 Found",
            "",
            "Location: https://example.invalid/steal\r\n",
        );
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client
                .get::<ApiResponse<ReleaseResult>>(
                    "/registry/releases/11111111-1111-4111-8111-111111111111",
                    Some("fixture-token"),
                    cancel
                )
                .await,
            Err(Error::Http(StatusCode::FOUND))
        ));
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn list_query_uses_bounded_repeated_keys() {
        let (origin, thread) = serve("200 OK", &fixture("list"), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let query = ListQuery {
            query: "Lua & maps".into(),
            include_tags: vec!["lua".into(), "maps".into()],
            ..ListQuery::default()
        };
        let (_sender, cancel) = watch::channel(false);
        let result = client
            .list_mods(&query, "fixture-token", cancel)
            .await
            .unwrap();
        assert_eq!(result.items.len(), 1);
        let request = thread.join().unwrap();
        assert!(request.starts_with("GET /v1/registry/mods?"));
        assert!(request.contains("query=Lua+%26+maps"));
        assert!(request.contains("includeTags=lua&includeTags=maps"));

        let invalid = ListQuery {
            page_size: 25,
            ..ListQuery::default()
        };
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client.list_mods(&invalid, "fixture-token", cancel).await,
            Err(Error::InvalidQuery)
        ));
    }

    #[tokio::test]
    async fn errors_and_malformed_json_are_distinct() {
        let (origin, thread) = serve("401 Unauthorized", &fixture("unauthorised"), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client
                .get::<ApiResponse<ReleaseResult>>(
                    "/registry/releases/11111111-1111-4111-8111-111111111111",
                    None,
                    cancel
                )
                .await,
            Err(Error::Server(
                StatusCode::UNAUTHORIZED,
                ResponseError {
                    code: ErrorCode::Unauthenticated,
                    ..
                }
            ))
        ));
        thread.join().unwrap();

        let (origin, thread) = serve("200 OK", "{\"apiVersion\":1,\"apiVersion\":1}", "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client
                .get::<ApiResponse<ReleaseResult>>(
                    "/registry/releases/11111111-1111-4111-8111-111111111111",
                    None,
                    cancel
                )
                .await,
            Err(Error::Protocol)
        ));
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn exact_release_lookup_rejects_mismatched_and_unsafe_response() {
        let mut body: serde_json::Value = serde_json::from_str(&fixture("approved")).unwrap();
        body["data"]["releaseId"] = serde_json::json!("22222222-2222-4222-8222-222222222222");
        let (origin, thread) = serve("200 OK", &body.to_string(), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        let id = ReleaseId(uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        assert!(matches!(
            client.release(id, "fixture-token", cancel).await,
            Err(Error::Protocol)
        ));
        thread.join().unwrap();

        body["data"]["releaseId"] = serde_json::json!("11111111-1111-4111-8111-111111111111");
        body["data"]["artifact"]["bytes"] = serde_json::json!(9_007_199_254_740_992u64);
        let (origin, thread) = serve("200 OK", &body.to_string(), "");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client.release(id, "fixture-token", cancel).await,
            Err(Error::Protocol)
        ));
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn retry_after_requires_matching_structured_error() {
        let body = serde_json::json!({"apiVersion":1,"error":{"code":"rate_limited","message":"Try again later.","requestId":"44444444-4444-4444-8444-444444444444","retryAfterSeconds":7,"problems":[]}}).to_string();
        let id = ReleaseId(uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap());
        let (origin, thread) = serve("429 Too Many Requests", &body, "Retry-After: 7\r\n");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client.release(id, "fixture-token", cancel).await,
            Err(Error::Server(
                StatusCode::TOO_MANY_REQUESTS,
                ResponseError {
                    retry_after_seconds: Some(7),
                    ..
                }
            ))
        ));
        thread.join().unwrap();

        let (origin, thread) = serve("429 Too Many Requests", &body, "Retry-After: 8\r\n");
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client.release(id, "fixture-token", cancel).await,
            Err(Error::Protocol)
        ));
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn cancellation_and_size_limit_stop_the_exchange() {
        let client =
            Client::new(Config::new("http://127.0.0.1:1/v1", "http://127.0.0.1:1/", true).unwrap())
                .unwrap();
        let (sender, cancel) = watch::channel(false);
        sender.send(true).unwrap();
        assert!(matches!(
            client
                .get::<ApiResponse<ReleaseResult>>(
                    "/registry/releases/11111111-1111-4111-8111-111111111111",
                    None,
                    cancel
                )
                .await,
            Err(Error::Cancelled)
        ));

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            assert!(stream.read(&mut request).unwrap() > 0);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_JSON_BYTES + 1
            )
            .unwrap();
        });
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            client
                .get::<ApiResponse<ReleaseResult>>(
                    "/registry/releases/11111111-1111-4111-8111-111111111111",
                    None,
                    cancel
                )
                .await,
            Err(Error::TooLarge)
        ));
        thread.join().unwrap();
    }
}
