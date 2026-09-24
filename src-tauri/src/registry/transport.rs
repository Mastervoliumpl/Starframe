use super::{ApiResponse, ListQuery, ModList, ReleaseId, ReleaseResult};
use reqwest::{StatusCode, Url, header};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::watch;

const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;

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
        Ok(Self { config, http })
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
        mut cancel: watch::Receiver<bool>,
    ) -> Result<T, Error> {
        let mut request = self
            .http
            .get(url)
            .header(header::ACCEPT, "application/json");
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
                serde_json::from_value(value).map_err(|_| Error::Protocol)
            } else if let Ok(error) = serde_json::from_value::<ErrorEnvelope>(value) {
                if error.api_version != 1
                    || error.error.message.is_empty()
                    || error.error.message.len() > 1000
                    || error.error.problems.len() > 20
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
    use std::net::TcpListener;

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
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buffer = [0; 8192];
            let n = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..n]).to_string();
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
