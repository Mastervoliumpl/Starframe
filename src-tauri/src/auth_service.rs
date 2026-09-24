use crate::model::CommandError;
use serde::Serialize;
use starframe::packages::RegistryRequest;
use starframe::registry::{
    Auth, AuthError, ChallengeView, Client, Config, CredentialStore, Error, ModId, Poll, ReleaseId,
    Session, SignOut,
};
use std::sync::Mutex;
use tauri::State;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::{Mutex as AsyncMutex, watch};

pub struct AuthService {
    auth: AsyncMutex<Auth>,
    client: Client,
    cancel: Mutex<watch::Sender<bool>>,
}

const PRODUCTION_REGISTRY_ROOT: Option<[u8; 32]> = None;

impl AuthService {
    pub fn new() -> Result<Self, String> {
        let config = Config::new(
            "https://api.starframemanager.com/v1",
            "https://starframemanager.com/",
            false,
        )
        .map_err(|_| "The website connection is not configured correctly.")?;
        let client = Client::new(config).map_err(|_| "The website client could not start.")?;
        let (cancel, _) = watch::channel(false);
        let store = {
            #[cfg(debug_assertions)]
            if let Some(root) = std::env::var_os("STARFRAME_TEST_DATA_DIR") {
                CredentialStore::isolated(std::path::Path::new(&root))
            } else {
                CredentialStore::production()
            }
            #[cfg(not(debug_assertions))]
            CredentialStore::production()
        };
        Ok(Self {
            auth: AsyncMutex::new(Auth::with_store(client.clone(), store)),
            client,
            cancel: Mutex::new(cancel),
        })
    }

    pub async fn registry_request(
        &self,
        mod_id: ModId,
        release_id: ReleaseId,
    ) -> Result<RegistryRequest, CommandError> {
        let root_public = PRODUCTION_REGISTRY_ROOT.ok_or_else(|| {
            CommandError::new(
                "registry_trust_unavailable",
                "This Starframe build has no independently provisioned registry root. Registry downloads are unavailable; local mods remain usable.",
            )
        })?;
        let cancel = self.receiver();
        let mut auth = self.auth.lock().await;
        let session = auth
            .inspect(cancel.clone())
            .await
            .map_err(failure)?
            .ok_or_else(|| {
                CommandError::new("auth_required", "Sign in to download this release.")
            })?;
        let bearer = auth
            .token()
            .ok_or_else(|| CommandError::new("auth_required", "Sign in to download this release."))?
            .to_owned();
        Ok(RegistryRequest {
            mod_id,
            release_id,
            root_public,
            client: self.client.clone(),
            session,
            bearer,
            auth_cancel: cancel,
        })
    }

    fn receiver(&self) -> watch::Receiver<bool> {
        let mut sender = self.cancel.lock().expect("auth cancellation state");
        if *sender.borrow() {
            *sender = watch::channel(false).0;
        }
        sender.subscribe()
    }

    fn stop(&self) {
        self.cancel
            .lock()
            .expect("auth cancellation state")
            .send_replace(true);
    }
}

fn failure(error: AuthError) -> CommandError {
    let (code, message) = match error {
        AuthError::Network(Error::Cancelled) => ("auth_cancelled", "Sign-in was cancelled."),
        AuthError::Network(Error::Transport) => (
            "auth_network",
            "The website could not be reached. Local mods remain available.",
        ),
        AuthError::Network(Error::Server(status, _)) if status.as_u16() == 403 => (
            "auth_rejected",
            "The sign-in request was declined or is no longer available. Start again.",
        ),
        AuthError::Network(Error::Server(status, _)) if status.as_u16() == 429 => (
            "auth_rate_limit",
            "The website asked Starframe to wait. Try again later.",
        ),
        AuthError::Network(_) => (
            "auth_response",
            "The website could not confirm this request. Try again.",
        ),
        AuthError::Storage(message) => ("auth_storage", message),
        AuthError::InvalidChallenge => (
            "auth_challenge",
            "The website sent an invalid sign-in link. Try again later.",
        ),
        AuthError::NoChallenge => ("auth_challenge", "Start sign-in first."),
        AuthError::TooSoon => ("auth_wait", "Wait for the next sign-in check."),
        AuthError::Expired => ("auth_expired", "The sign-in code expired. Start again."),
        AuthError::InvalidExchange => (
            "auth_exchange",
            "The sign-in response could not be used. Start again.",
        ),
        AuthError::AlreadySignedIn => ("auth_signed_in", "Sign out before changing accounts."),
    };
    CommandError::new(code, message)
}

#[tauri::command]
pub async fn auth_restore(
    service: State<'_, AuthService>,
) -> Result<Option<Session>, CommandError> {
    service
        .auth
        .lock()
        .await
        .restore(service.receiver())
        .await
        .map_err(failure)
}

#[tauri::command]
pub async fn auth_inspect(
    service: State<'_, AuthService>,
) -> Result<Option<Session>, CommandError> {
    service
        .auth
        .lock()
        .await
        .inspect(service.receiver())
        .await
        .map_err(failure)
}

#[tauri::command]
pub async fn auth_start(
    app: tauri::AppHandle,
    service: State<'_, AuthService>,
) -> Result<ChallengeView, CommandError> {
    let mut auth = service.auth.lock().await;
    let challenge = auth.start(service.receiver()).await.map_err(failure)?;
    if app
        .opener()
        .open_url(&challenge.verification_uri, None::<&str>)
        .is_err()
    {
        auth.cancel();
        return Err(CommandError::new(
            "auth_browser",
            "The browser could not open. Try sign-in again.",
        ));
    }
    Ok(challenge)
}

#[tauri::command]
pub async fn auth_poll(service: State<'_, AuthService>) -> Result<Poll, CommandError> {
    service
        .auth
        .lock()
        .await
        .poll(service.receiver())
        .await
        .map_err(failure)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignOutResult {
    server_revoked: bool,
}

#[tauri::command]
pub async fn auth_sign_out(service: State<'_, AuthService>) -> Result<SignOutResult, CommandError> {
    service.stop();
    let mut auth = service.auth.lock().await;
    let result = auth.sign_out(service.receiver()).await.map_err(failure)?;
    Ok(SignOutResult {
        server_revoked: matches!(result, SignOut::Revoked),
    })
}

#[tauri::command]
pub async fn auth_cancel(service: State<'_, AuthService>) -> Result<(), CommandError> {
    service.stop();
    service.auth.lock().await.cancel();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn production_download_refuses_an_unprovisioned_registry_root() {
        let service = AuthService::new().unwrap();
        let error = service
            .registry_request(ModId::try_from(1).unwrap(), ReleaseId(uuid::Uuid::new_v4()))
            .await
            .err()
            .unwrap();
        assert_eq!(error.code, "registry_trust_unavailable");
        assert!(error.message.contains("local mods remain usable"));
    }
}
