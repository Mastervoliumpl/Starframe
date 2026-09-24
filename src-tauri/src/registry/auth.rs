#[cfg(windows)]
use super::credential::{CredentialStore, StoredToken};
use super::{ApiResponse, Client, Error, Session, SessionContext};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::watch;
use uuid::Uuid;

#[derive(Debug)]
pub enum AuthError {
    Network(Error),
    InvalidChallenge,
    NoChallenge,
    TooSoon,
    Expired,
    InvalidExchange,
    AlreadySignedIn,
    Storage(&'static str),
}

impl From<Error> for AuthError {
    fn from(error: Error) -> Self {
        Self::Network(error)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeView {
    pub display_code: String,
    pub verification_uri: String,
    pub expires_at: String,
    pub interval_seconds: u8,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Poll {
    Waiting,
    SignedIn,
}

#[derive(Debug)]
pub enum SignOut {
    Revoked,
    LocalOnly(Error),
}

struct Pending {
    id: Uuid,
    verifier: String,
    expires: OffsetDateTime,
    next_poll: Instant,
}

struct Credential {
    token: String,
    expires: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ChallengeData {
    challenge_id: Uuid,
    display_code: String,
    verification_uri: String,
    expires_at: String,
    interval_seconds: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CredentialData {
    token: String,
    token_type: Bearer,
    expires_at: String,
}

#[derive(Deserialize)]
#[serde(try_from = "String")]
struct Bearer;

impl TryFrom<String> for Bearer {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value == "Bearer" {
            Ok(Self)
        } else {
            Err("unexpected token type")
        }
    }
}

pub struct Auth {
    client: Client,
    pending: Option<Pending>,
    credential: Option<Credential>,
    #[cfg(windows)]
    store: Option<CredentialStore>,
}

impl Auth {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            pending: None,
            credential: None,
            #[cfg(windows)]
            store: None,
        }
    }

    #[cfg(windows)]
    pub fn with_store(client: Client, store: CredentialStore) -> Self {
        let mut auth = Self::new(client);
        auth.store = Some(store);
        auth
    }

    #[cfg(windows)]
    fn clear_saved(&mut self) -> Result<(), AuthError> {
        if let Some(store) = &self.store {
            store.delete().map_err(AuthError::Storage)?;
        }
        self.credential = None;
        Ok(())
    }

    #[cfg(not(windows))]
    fn clear_saved(&mut self) -> Result<(), AuthError> {
        self.credential = None;
        Ok(())
    }

    #[cfg(windows)]
    pub async fn restore(
        &mut self,
        cancel: watch::Receiver<bool>,
    ) -> Result<Option<Session>, AuthError> {
        let Some(store) = &self.store else {
            return Ok(None);
        };
        let saved = store.load().map_err(AuthError::Storage)?;
        let Some(saved) = saved else { return Ok(None) };
        let Some(expires) = parse_future(&saved.expires_at) else {
            self.clear_saved()?;
            return Ok(None);
        };
        self.credential = Some(Credential {
            token: saved.token,
            expires,
        });
        self.inspect(cancel).await
    }

    pub async fn inspect(
        &mut self,
        cancel: watch::Receiver<bool>,
    ) -> Result<Option<Session>, AuthError> {
        let Some(token) = self.token() else {
            self.clear_saved()?;
            return Ok(None);
        };
        match self.client.session(token, cancel).await {
            Ok(Some(session))
                if matches!(session.context, SessionContext::Manager)
                    && parse_future(&session.expires_at).is_some() =>
            {
                Ok(Some(session))
            }
            Ok(None) | Ok(Some(_)) | Err(Error::Server(reqwest::StatusCode::UNAUTHORIZED, _)) => {
                self.clear_saved()?;
                Ok(None)
            }
            Err(error) => Err(AuthError::Network(error)),
        }
    }

    pub async fn sign_out(&mut self, cancel: watch::Receiver<bool>) -> Result<SignOut, AuthError> {
        self.pending = None;
        let token = self.token().map(str::to_owned);
        self.clear_saved()?;
        match token {
            Some(token) => match self.client.revoke(&token, cancel).await {
                Ok(()) | Err(Error::Server(reqwest::StatusCode::UNAUTHORIZED, _)) => {
                    Ok(SignOut::Revoked)
                }
                Err(error) => Ok(SignOut::LocalOnly(error)),
            },
            None => Ok(SignOut::Revoked),
        }
    }

    pub fn token(&self) -> Option<&str> {
        self.credential
            .as_ref()
            .filter(|credential| credential.expires > OffsetDateTime::now_utc())
            .map(|credential| credential.token.as_str())
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }

    pub async fn start(
        &mut self,
        cancel: watch::Receiver<bool>,
    ) -> Result<ChallengeView, AuthError> {
        if self.token().is_some() {
            return Err(AuthError::AlreadySignedIn);
        }
        self.clear_saved()?;
        self.pending = None;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|_| AuthError::InvalidChallenge)?;
        let verifier = URL_SAFE_NO_PAD.encode(random);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let (status, response): (StatusCode, ApiResponse<ChallengeData>) = self
            .client
            .post(
                "/auth/manager/start",
                &serde_json::json!({"codeChallenge":challenge}),
                cancel,
            )
            .await?;
        if status != StatusCode::CREATED {
            return Err(AuthError::InvalidChallenge);
        }
        let data = response.data;
        let expires = parse_future(&data.expires_at).ok_or(AuthError::InvalidChallenge)?;
        if expires - OffsetDateTime::now_utc() > time::Duration::minutes(15) {
            return Err(AuthError::InvalidChallenge);
        }
        let url = Url::parse(&data.verification_uri).map_err(|_| AuthError::InvalidChallenge)?;
        let website = self.client.website();
        if data.interval_seconds != 5
            || data.display_code.len() != 8
            || !data
                .display_code
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
            || url.origin() != website.origin()
            || url.path() != "/sign-in"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.query_pairs().count() != 1
            || url.query_pairs().next().is_none_or(|(key, value)| {
                key != "manager" || value != data.challenge_id.to_string()
            })
        {
            return Err(AuthError::InvalidChallenge);
        }
        self.pending = Some(Pending {
            id: data.challenge_id,
            verifier,
            expires,
            next_poll: Instant::now(),
        });
        Ok(ChallengeView {
            display_code: data.display_code,
            verification_uri: data.verification_uri,
            expires_at: data.expires_at,
            interval_seconds: data.interval_seconds,
        })
    }

    pub async fn poll(&mut self, cancel: watch::Receiver<bool>) -> Result<Poll, AuthError> {
        let pending = self.pending.as_mut().ok_or(AuthError::NoChallenge)?;
        if pending.expires <= OffsetDateTime::now_utc() {
            self.pending = None;
            return Err(AuthError::Expired);
        }
        if pending.next_poll > Instant::now() {
            return Err(AuthError::TooSoon);
        }
        pending.next_poll = Instant::now() + Duration::from_secs(5);
        let body = serde_json::json!({"challengeId": pending.id, "codeVerifier": pending.verifier});
        let result: Result<(StatusCode, ApiResponse<Option<CredentialData>>), Error> = self
            .client
            .post("/auth/manager/exchange", &body, cancel)
            .await;
        match result {
            Ok((StatusCode::ACCEPTED, ApiResponse { data: None, .. })) => Ok(Poll::Waiting),
            Ok((
                StatusCode::OK,
                ApiResponse {
                    data: Some(data), ..
                },
            )) => {
                let Bearer = data.token_type;
                let Some(expires) = parse_future(&data.expires_at) else {
                    self.pending = None;
                    return Err(AuthError::InvalidExchange);
                };
                if expires - OffsetDateTime::now_utc() > time::Duration::days(31) {
                    self.pending = None;
                    return Err(AuthError::InvalidExchange);
                }
                if data.token.len() != 43
                    || !data
                        .token
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
                {
                    self.pending = None;
                    return Err(AuthError::InvalidExchange);
                }
                #[cfg(windows)]
                if let Some(store) = &self.store {
                    let saved = store
                        .save(&StoredToken {
                            schema_version: 1,
                            token: data.token.clone(),
                            expires_at: data.expires_at.clone(),
                        })
                        .map_err(AuthError::Storage);
                    if let Err(error) = saved {
                        self.pending = None;
                        return Err(error);
                    }
                }
                self.credential = Some(Credential {
                    token: data.token,
                    expires,
                });
                self.pending = None;
                Ok(Poll::SignedIn)
            }
            Ok(_) => {
                self.pending = None;
                Err(AuthError::InvalidExchange)
            }
            Err(error) => {
                self.pending = None;
                Err(AuthError::Network(error))
            }
        }
    }
}

fn parse_future(value: &str) -> Option<OffsetDateTime> {
    if !value.ends_with('Z') {
        return None;
    }
    let date = OffsetDateTime::parse(value, &Rfc3339).ok()?;
    (date > OffsetDateTime::now_utc()).then_some(date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Config;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::{TcpListener, TcpStream},
    };

    fn read_json(stream: &TcpStream) -> serde_json::Value {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut length = 0;
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        if body.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        }
    }

    fn reply(stream: &mut TcpStream, status: &str, body: serde_json::Value) {
        let bytes = body.to_string();
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bytes}",
            bytes.len()
        )
        .unwrap();
    }

    #[tokio::test]
    async fn pkce_challenge_pending_poll_and_single_token_confirmation() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let website = origin.clone();
        let server = std::thread::spawn(move || {
            let (mut start, _) = listener.accept().unwrap();
            let start_body = read_json(&start);
            let expected = start_body["codeChallenge"].as_str().unwrap().to_owned();
            assert_eq!(expected.len(), 43);
            let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(10))
                .format(&Rfc3339)
                .unwrap();
            reply(
                &mut start,
                "201 Created",
                serde_json::json!({"apiVersion":1,"data":{"challengeId":"11111111-1111-4111-8111-111111111111","displayCode":"A1B2C3D4","verificationUri":format!("{website}/sign-in?manager=11111111-1111-4111-8111-111111111111"),"expiresAt":expires,"intervalSeconds":5}}),
            );
            for status in ["202 Accepted", "200 OK"] {
                let (mut exchange, _) = listener.accept().unwrap();
                let body = read_json(&exchange);
                let verifier = body["codeVerifier"].as_str().unwrap();
                assert_eq!(
                    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
                    expected
                );
                assert_eq!(body["challengeId"], "11111111-1111-4111-8111-111111111111");
                let payload = if status == "202 Accepted" {
                    serde_json::Value::Null
                } else {
                    serde_json::json!({"token":"A".repeat(43),"tokenType":"Bearer","expiresAt":(OffsetDateTime::now_utc() + time::Duration::days(30)).format(&Rfc3339).unwrap()})
                };
                reply(
                    &mut exchange,
                    status,
                    serde_json::json!({"apiVersion":1,"data":payload}),
                );
            }
        });
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let mut auth = Auth::new(client);
        let (sender, cancel) = watch::channel(false);
        let challenge = auth.start(cancel).await.unwrap();
        let view = serde_json::to_string(&challenge).unwrap();
        assert!(view.contains("A1B2C3D4"));
        assert!(!view.contains("codeVerifier"));
        assert_eq!(auth.poll(sender.subscribe()).await.unwrap(), Poll::Waiting);
        assert!(matches!(
            auth.poll(sender.subscribe()).await,
            Err(AuthError::TooSoon)
        ));
        auth.pending.as_mut().unwrap().next_poll = Instant::now() - Duration::from_secs(1);
        assert_eq!(auth.poll(sender.subscribe()).await.unwrap(), Poll::SignedIn);
        assert_eq!(auth.token(), Some("A".repeat(43).as_str()));
        assert!(matches!(
            auth.start(sender.subscribe()).await,
            Err(AuthError::AlreadySignedIn)
        ));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn rejects_foreign_verification_link_without_retaining_verifier() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut start, _) = listener.accept().unwrap();
            let _ = read_json(&start);
            let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(10))
                .format(&Rfc3339)
                .unwrap();
            reply(
                &mut start,
                "201 Created",
                serde_json::json!({"apiVersion":1,"data":{"challengeId":"11111111-1111-4111-8111-111111111111","displayCode":"A1B2C3D4","verificationUri":"https://attacker.invalid/sign-in?manager=11111111-1111-4111-8111-111111111111","expiresAt":expires,"intervalSeconds":5}}),
            );
        });
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let mut auth = Auth::new(client);
        let (_sender, cancel) = watch::channel(false);
        assert!(matches!(
            auth.start(cancel).await,
            Err(AuthError::InvalidChallenge)
        ));
        assert!(auth.pending.is_none());
        server.join().unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn restored_session_and_failed_remote_signout_clear_only_local_token() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut inspect, _) = listener.accept().unwrap();
            assert!(read_json(&inspect).is_null());
            let expires = (OffsetDateTime::now_utc() + time::Duration::days(29))
                .format(&Rfc3339)
                .unwrap();
            reply(
                &mut inspect,
                "200 OK",
                serde_json::json!({"apiVersion":1,"data":{
                    "accountId":"33333333-3333-4333-8333-333333333333",
                    "profile":{"displayName":"Fixture user","avatarUrl":null},
                    "context":"manager","authenticatedAt":"2026-09-20T00:00:00Z",
                    "expiresAt":expires,"capabilities":["download_mod"],"isOwner":false
                }}),
            );
            let (mut revoke, _) = listener.accept().unwrap();
            assert!(read_json(&revoke).is_null());
            reply(
                &mut revoke,
                "503 Service Unavailable",
                serde_json::json!({"apiVersion":1,"error":{
                    "code":"service_unavailable","message":"Retry later","requestId":"11111111-1111-4111-8111-111111111111","retryAfterSeconds":null,"problems":[]
                }}),
            );
        });
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let store = CredentialStore::fixture();
        struct Cleanup<'a>(&'a CredentialStore);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.delete();
            }
        }
        let _cleanup = Cleanup(&store);
        store
            .save(&StoredToken {
                schema_version: 1,
                token: "A".repeat(43),
                expires_at: (OffsetDateTime::now_utc() + time::Duration::days(30))
                    .format(&Rfc3339)
                    .unwrap(),
            })
            .unwrap();
        let mut auth = Auth::with_store(client, store.clone());
        let (_sender, cancel) = watch::channel(false);
        let session = auth.restore(cancel.clone()).await.unwrap().unwrap();
        assert_eq!(session.profile.display_name, "Fixture user");
        assert!(matches!(
            auth.sign_out(cancel).await.unwrap(),
            SignOut::LocalOnly(Error::Http(StatusCode::SERVICE_UNAVAILABLE))
        ));
        assert!(auth.token().is_none());
        assert!(auth.store.as_ref().unwrap().load().unwrap().is_none());
        server.join().unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn revoked_saved_session_is_removed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut inspect, _) = listener.accept().unwrap();
            assert!(read_json(&inspect).is_null());
            reply(
                &mut inspect,
                "401 Unauthorized",
                serde_json::json!({"apiVersion":1,"error":{
                    "code":"unauthenticated","message":"Expired","requestId":"11111111-1111-4111-8111-111111111111","retryAfterSeconds":null,"problems":[]
                }}),
            );
        });
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let store = CredentialStore::fixture();
        struct Cleanup<'a>(&'a CredentialStore);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.delete();
            }
        }
        let _cleanup = Cleanup(&store);
        store
            .save(&StoredToken {
                schema_version: 1,
                token: "A".repeat(43),
                expires_at: (OffsetDateTime::now_utc() + time::Duration::days(30))
                    .format(&Rfc3339)
                    .unwrap(),
            })
            .unwrap();
        let mut auth = Auth::with_store(client, store.clone());
        let (_sender, cancel) = watch::channel(false);
        assert!(auth.restore(cancel).await.unwrap().is_none());
        assert!(auth.store.as_ref().unwrap().load().unwrap().is_none());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn cancellation_and_expiry_remove_the_private_challenge() {
        let client =
            Client::new(Config::new("http://127.0.0.1:1/v1", "http://127.0.0.1:1/", true).unwrap())
                .unwrap();
        let mut auth = Auth::new(client);
        let (_sender, cancel) = watch::channel(false);
        auth.pending = Some(Pending {
            id: Uuid::new_v4(),
            verifier: "A".repeat(43),
            expires: OffsetDateTime::now_utc() + time::Duration::minutes(1),
            next_poll: Instant::now(),
        });
        auth.cancel();
        assert!(matches!(
            auth.poll(cancel.clone()).await,
            Err(AuthError::NoChallenge)
        ));
        auth.pending = Some(Pending {
            id: Uuid::new_v4(),
            verifier: "B".repeat(43),
            expires: OffsetDateTime::now_utc() - time::Duration::seconds(1),
            next_poll: Instant::now(),
        });
        assert!(matches!(auth.poll(cancel).await, Err(AuthError::Expired)));
        assert!(auth.pending.is_none());
    }

    #[tokio::test]
    async fn lost_exchange_response_requires_a_new_challenge() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let website = origin.clone();
        let server = std::thread::spawn(move || {
            for attempt in 0..2 {
                let (mut start, _) = listener.accept().unwrap();
                let _ = read_json(&start);
                let expires = (OffsetDateTime::now_utc() + time::Duration::minutes(10))
                    .format(&Rfc3339)
                    .unwrap();
                reply(
                    &mut start,
                    "201 Created",
                    serde_json::json!({"apiVersion":1,"data":{
                        "challengeId":"11111111-1111-4111-8111-111111111111",
                        "displayCode":"A1B2C3D4",
                        "verificationUri":format!("{website}/sign-in?manager=11111111-1111-4111-8111-111111111111"),
                        "expiresAt":expires,"intervalSeconds":5
                    }}),
                );
                if attempt == 0 {
                    let (exchange, _) = listener.accept().unwrap();
                    let _ = read_json(&exchange);
                    drop(exchange);
                }
            }
        });
        let client =
            Client::new(Config::new(&format!("{origin}/v1"), &format!("{origin}/"), true).unwrap())
                .unwrap();
        let mut auth = Auth::new(client);
        let (_sender, cancel) = watch::channel(false);
        auth.start(cancel.clone()).await.unwrap();
        assert!(matches!(
            auth.poll(cancel.clone()).await,
            Err(AuthError::Network(_))
        ));
        assert!(auth.pending.is_none());
        assert!(matches!(
            auth.poll(cancel.clone()).await,
            Err(AuthError::NoChallenge)
        ));
        auth.start(cancel).await.unwrap();
        server.join().unwrap();
    }
}
