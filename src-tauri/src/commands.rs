use crate::{
    app::{Shared, start_worker},
    model::{CommandError, Snapshot},
};
use tauri::{Manager, State, ipc::Channel};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn update_action(
    app: tauri::AppHandle,
    action: starframe::updates::Action,
) -> Result<(), CommandError> {
    app.state::<crate::game_service::GameService>()
        .update(action)
        .await
}

#[tauri::command]
pub async fn pick_local_source(
    app: tauri::AppHandle,
    folder: bool,
) -> Result<Option<String>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app.dialog().file().set_title("Import local mod");
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.set_parent(&window);
        }
        let selected = if folder {
            dialog.blocking_pick_folder()
        } else {
            dialog
                .add_filter("Managed DLL", &["dll"])
                .blocking_pick_file()
        };
        selected
            .map(|file| {
                file.into_path()
                    .map_err(|e| CommandError::new("local_source", &e.to_string()))
                    .and_then(|path| {
                        path.to_str().map(str::to_owned).ok_or_else(|| {
                            CommandError::new(
                                "local_source",
                                "The source path must use Unicode names.",
                            )
                        })
                    })
            })
            .transpose()
    })
    .await
    .map_err(|_| CommandError::new("local_source", "The source picker stopped unexpectedly."))?
}

#[tauri::command]
pub async fn save_collection_file(
    app: tauri::AppHandle,
    text: String,
) -> Result<bool, CommandError> {
    let document = starframe::sharing::Portable::read(&text)
        .map_err(|e| CommandError::new("invalid_collection", &e))?;
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|e| CommandError::new("invalid_collection", &e.to_string()))?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .set_title("Save collection file")
            .set_file_name("collection.starframe-collection.json")
            .add_filter("Starframe collection", &["json"])
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = file
            .into_path()
            .map_err(|e| CommandError::new("save_failed", &e.to_string()))?;
        std::fs::write(path, bytes).map_err(|e| {
            CommandError::new(
                "save_failed",
                &format!("Could not save the collection file: {e}"),
            )
        })?;
        Ok(true)
    })
    .await
    .map_err(|_| CommandError::new("save_failed", "The save dialog stopped unexpectedly."))?
}

#[tauri::command]
pub async fn sharing_action(
    service: State<'_, crate::game_service::GameService>,
    auth: State<'_, crate::auth_service::AuthService>,
    action: starframe::sharing::Action,
) -> Result<starframe::sharing::Reply, CommandError> {
    let online = matches!(
        action,
        starframe::sharing::Action::Accept { .. } | starframe::sharing::Action::Retry { .. }
    );
    let reply = service.sharing(action).await?;
    if online && let Some(id) = reply.collection_id.clone() {
        use starframe::sharing::{OnlineAction, OnlineReply};
        if let OnlineReply::Plan(missing) = service
            .shared_registry(OnlineAction::Plan { id: id.clone() })
            .await?
            && let Some(first) = missing.first()
        {
            let request = auth.registry_request(first.mod_id, first.release_id).await;
            match request {
                Ok(request) => {
                    let service = service.inner().clone();
                    tauri::async_runtime::spawn(async move {
                        for reference in missing {
                            let mut request = request.clone();
                            request.mod_id = reference.mod_id;
                            request.release_id = reference.release_id;
                            let outcome = async {
                                let approval =
                                    starframe::packages::RegistryApproval::fetch(&request).await?;
                                loop {
                                    let reply = service
                                        .shared_registry(OnlineAction::Install {
                                            id: id.clone(),
                                            reference: reference.clone(),
                                            request: Box::new(request.clone()),
                                            approval: Box::new(approval.clone()),
                                        })
                                        .await
                                        .map_err(|error| error.message)?;
                                    if matches!(reply, OnlineReply::Install(Some(_))) {
                                        break;
                                    }
                                    if *request.auth_cancel.borrow() {
                                        return Err(
                                            "Sign-in changed. Retry import after signing in."
                                                .into(),
                                        );
                                    }
                                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                                }
                                Ok::<_, String>(())
                            }
                            .await;
                            if let Err(message) = outcome {
                                let _ = service
                                    .shared_registry(OnlineAction::Fail {
                                        id: id.clone(),
                                        reference,
                                        message,
                                    })
                                    .await;
                            }
                        }
                    });
                }
                Err(error) => {
                    for reference in missing {
                        service
                            .shared_registry(OnlineAction::Fail {
                                id: id.clone(),
                                reference,
                                message: error.message.clone(),
                            })
                            .await?;
                    }
                }
            }
        }
    }
    Ok(reply)
}

#[tauri::command]
pub async fn mod_action(
    service: State<'_, crate::game_service::GameService>,
    action: starframe::mods::Action,
) -> Result<starframe::mods::View, CommandError> {
    service.mods(action).await
}

#[tauri::command]
pub async fn package_action(
    service: State<'_, crate::game_service::GameService>,
    action: starframe::packages::Action,
) -> Result<Vec<starframe::packages::Operation>, CommandError> {
    service.package(action).await
}

#[tauri::command]
pub async fn registry_download(
    service: State<'_, crate::game_service::GameService>,
    auth: State<'_, crate::auth_service::AuthService>,
    request_id: String,
    mod_id: u64,
    release_id: String,
) -> Result<Vec<starframe::packages::Operation>, CommandError> {
    uuid::Uuid::parse_str(&request_id)
        .map_err(|_| CommandError::new("registry_request", "Invalid download request ID."))?;
    let mod_id = starframe::registry::ModId::try_from(mod_id)
        .map_err(|_| CommandError::new("registry_mod", "Invalid registry ModID."))?;
    let release_id = uuid::Uuid::parse_str(&release_id)
        .map(starframe::registry::ReleaseId)
        .map_err(|_| CommandError::new("registry_release", "Invalid registry ReleaseID."))?;
    let request = auth.registry_request(mod_id, release_id).await?;
    service.registry_package(request_id, request).await
}

fn registry_failure(error: starframe::registry::Error) -> CommandError {
    use starframe::registry::Error;
    let status = match &error {
        Error::Server(status, _) | Error::Http(status) => Some(status.as_u16()),
        _ => None,
    };
    let (code, message) = match status {
        Some(401) => (
            "auth_required",
            "The manager session ended. Sign in again for online mods.",
        ),
        Some(403) => (
            "registry_permission",
            "This account cannot access this registry request. Installed and local mods remain available.",
        ),
        Some(404 | 410) => (
            "registry_unavailable",
            "This mod or release is no longer available.",
        ),
        Some(429) => (
            "registry_wait",
            "The website asked Starframe to wait. Retry later.",
        ),
        _ if matches!(error, Error::Cancelled) => (
            "registry_cancelled",
            "The registry request stopped because sign-in changed.",
        ),
        _ if matches!(error, Error::Protocol | Error::InvalidQuery) => (
            "registry_response",
            "The registry response or filters could not be used. Retry or reset filters.",
        ),
        _ => (
            "registry_network",
            "The registry could not be reached. Retry later; installed and local mods remain available.",
        ),
    };
    CommandError::new(code, message)
}

#[tauri::command]
pub async fn registry_list(
    auth: State<'_, crate::auth_service::AuthService>,
    query: starframe::registry::ListQuery,
) -> Result<starframe::registry::ModList, CommandError> {
    let connection = auth.receipt_request().await?;
    connection
        .client
        .list_mods(&query, &connection.bearer, connection.auth_cancel)
        .await
        .map_err(registry_failure)
}

#[tauri::command]
pub async fn registry_options(
    auth: State<'_, crate::auth_service::AuthService>,
) -> Result<starframe::registry::Options, CommandError> {
    let connection = auth.receipt_request().await?;
    connection
        .client
        .discovery_options(&connection.bearer, connection.auth_cancel)
        .await
        .map_err(registry_failure)
}

#[tauri::command]
pub async fn registry_detail(
    auth: State<'_, crate::auth_service::AuthService>,
    mod_id: starframe::registry::ModId,
) -> Result<starframe::registry::ModResult, CommandError> {
    let connection = auth.receipt_request().await?;
    connection
        .client
        .mod_detail(mod_id, &connection.bearer, connection.auth_cancel)
        .await
        .map(|response| response.data)
        .map_err(registry_failure)
}

#[tauri::command]
pub async fn registry_release(
    auth: State<'_, crate::auth_service::AuthService>,
    release_id: starframe::registry::ReleaseId,
) -> Result<starframe::registry::ReleaseResult, CommandError> {
    let connection = auth.receipt_request().await?;
    connection
        .client
        .release(release_id, &connection.bearer, connection.auth_cancel)
        .await
        .map(|response| response.data)
        .map_err(registry_failure)
}

#[tauri::command]
pub async fn registry_history(
    auth: State<'_, crate::auth_service::AuthService>,
    mod_id: starframe::registry::ModId,
    page: u64,
    page_size: u8,
) -> Result<starframe::registry::ReleaseHistory, CommandError> {
    let connection = auth.receipt_request().await?;
    connection
        .client
        .release_history(
            mod_id,
            page,
            page_size,
            &connection.bearer,
            connection.auth_cancel,
        )
        .await
        .map_err(registry_failure)
}

#[tauri::command]
pub async fn registry_install(
    service: State<'_, crate::game_service::GameService>,
    auth: State<'_, crate::auth_service::AuthService>,
    request_id: String,
    reference: starframe::registry::ExactReference,
) -> Result<Vec<starframe::packages::Operation>, CommandError> {
    uuid::Uuid::parse_str(&request_id)
        .map_err(|_| CommandError::new("registry_request", "Invalid install request ID."))?;
    starframe::references::Reference::Registry(reference.clone())
        .validate()
        .map_err(|message| CommandError::new("registry_reference", &message))?;
    let request = auth
        .registry_request(reference.mod_id, reference.release_id)
        .await?;
    let approval = starframe::packages::RegistryApproval::fetch(&request)
        .await
        .map_err(|message| CommandError::new("registry_approval", &message))?;
    service
        .registry_install(request_id, reference, request, approval)
        .await
}

#[tauri::command]
pub async fn registry_retry_receipts(
    service: State<'_, crate::game_service::GameService>,
    auth: State<'_, crate::auth_service::AuthService>,
) -> Result<usize, CommandError> {
    service
        .registry_receipts(auth.receipt_request().await?)
        .await
}

#[tauri::command]
pub fn game_action(
    app: tauri::AppHandle,
    service: State<'_, crate::game_service::GameService>,
    action: crate::model::GameAction,
) -> Result<(), CommandError> {
    service.request(app, action)
}

#[tauri::command]
pub fn watch_state(
    state: State<'_, Shared>,
    channel: Channel<Snapshot>,
) -> Result<(), CommandError> {
    state.lock().expect("state lock").watch(channel)
}

#[tauri::command]
pub fn start_diagnostic(
    state: State<'_, Shared>,
    request_id: String,
    fail: bool,
) -> Result<Vec<String>, CommandError> {
    let (ids, start) = state.lock().expect("state lock").start(&request_id)?;
    if start {
        start_worker(state.inner().clone(), ids.clone(), fail);
    }
    Ok(ids)
}

#[tauri::command]
pub fn cancel_operation(
    state: State<'_, Shared>,
    operation_id: String,
) -> Result<(), CommandError> {
    state.lock().expect("state lock").cancel(&operation_id)
}

pub fn external_url(page: &str) -> Result<&'static str, CommandError> {
    match page {
        "repository" => Ok("https://github.com/Mastervoliumpl/Starframe"),
        "releases" => Ok("https://github.com/Mastervoliumpl/Starframe/releases"),
        "report" => {
            Ok("https://github.com/Mastervoliumpl/Starframe/issues/new?template=mod-report.yml")
        }
        "security" => Ok("https://github.com/Mastervoliumpl/Starframe/security/advisories/new"),
        _ => Err(CommandError::new(
            "invalid_link",
            "This external page is not available.",
        )),
    }
}

fn advisory_url(
    advisories: &starframe::catalog::advisories::Advisories,
    identity: &str,
) -> Result<String, CommandError> {
    let resolve = || {
        let (id, indexes) = identity.split_once(':')?;
        let (history, evidence) = indexes.split_once(':')?;
        advisories
            .advisories
            .iter()
            .find(|a| a.id == id)?
            .history
            .get(history.parse::<usize>().ok()?)?
            .evidence
            .get(evidence.parse::<usize>().ok()?)
            .cloned()
    };
    resolve().ok_or_else(|| {
        CommandError::new(
            "invalid_link",
            "This evidence link is not in the retained security advisory.",
        )
    })
}

#[tauri::command]
pub async fn open_external(
    app: tauri::AppHandle,
    service: State<'_, crate::game_service::GameService>,
    auth: State<'_, crate::auth_service::AuthService>,
    page: String,
) -> Result<(), CommandError> {
    if let Some(identity) = page.strip_prefix("local:") {
        let (id, hash) = identity
            .split_once(':')
            .ok_or_else(|| CommandError::new("local_source", "Invalid local reference."))?;
        let source = service
            .mods(starframe::mods::Action::List)
            .await?
            .local_sources
            .into_iter()
            .find(|s| s.reference.mod_id == id && s.reference.hash == hash)
            .ok_or_else(|| {
                CommandError::new(
                    "local_source",
                    "This local source is no longer in the library.",
                )
            })?;
        let path = std::path::PathBuf::from(source.path);
        let folder = if path.is_dir() {
            path.as_path()
        } else {
            path.parent().ok_or_else(|| {
                CommandError::new("local_source", "The source has no parent folder.")
            })?
        };
        return app
            .opener()
            .open_path(folder.to_string_lossy(), None::<&str>)
            .map_err(|_| {
                CommandError::new(
                    "local_source",
                    "Could not open the source folder. Check that it still exists.",
                )
            });
    }
    let url = if let Some(identity) = page.strip_prefix("registry:") {
        let mod_id = identity
            .parse::<u64>()
            .ok()
            .and_then(|id| starframe::registry::ModId::try_from(id).ok())
            .ok_or_else(|| CommandError::new("invalid_link", "Invalid registry mod reference."))?;
        let connection = auth.receipt_request().await?;
        let detail = connection
            .client
            .mod_detail(mod_id, &connection.bearer, connection.auth_cancel)
            .await
            .map_err(registry_failure)?;
        let starframe::registry::ModResult::Mod(detail) = detail.data else {
            return Err(CommandError::new(
                "invalid_link",
                "This mod is no longer available.",
            ));
        };
        detail.source_repository.ok_or_else(|| {
            CommandError::new("invalid_link", "This mod has no source repository.")
        })?
    } else if let Some(identity) = page.strip_prefix("advisory:") {
        let advisories = service
            .mods(starframe::mods::Action::List)
            .await?
            .advisories
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_link",
                    "No verified security advisories are available.",
                )
            })?;
        advisory_url(&advisories, identity)?
    } else if let Some(id) = page.strip_prefix("mod:") {
        service
            .mods(starframe::mods::Action::List)
            .await?
            .catalog
            .and_then(|catalog| catalog.mods.into_iter().find(|m| m.id == id))
            .map(|m| m.source_url)
            .ok_or_else(|| {
                CommandError::new(
                    "invalid_link",
                    "This mod source is not in the cached catalog.",
                )
            })?
    } else {
        external_url(&page)?.to_owned()
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| CommandError::new("open_failed", "Could not open the browser. Try again."))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evidence_links_resolve_only_retained_advisory_entries() {
        let advisories = starframe::catalog::advisories::Advisories::read(br#"{
          "schemaVersion":1,"revision":"1","advisories":[{
            "id":"fixture.finding","title":"Synthetic finding",
            "affected":[{"releaseId":"fixture.1","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","payloadSha256":[]}],
            "history":[{"recordedAt":1,"state":"confirmed","explanation":"Synthetic evidence","evidence":["https://example.invalid/evidence"],"recommendedAction":"Disable fixture"}]
          }]}
        "#).unwrap();
        assert_eq!(
            advisory_url(&advisories, "fixture.finding:0:0").unwrap(),
            "https://example.invalid/evidence"
        );
        for identity in [
            "missing:0:0",
            "fixture.finding:1:0",
            "fixture.finding:0:1",
            "fixture.finding:-1:0",
            "fixture.finding:0:0:1",
            "https://example.invalid/evidence",
            "file:///C:/secret",
        ] {
            assert!(advisory_url(&advisories, identity).is_err());
        }
    }
    #[test]
    fn only_known_external_pages_can_be_opened() {
        assert!(
            external_url("repository")
                .unwrap()
                .starts_with("https://github.com/")
        );
        for page in [
            "file:///C:/secret",
            "javascript:alert(1)",
            "https://other.example",
            "../",
        ] {
            assert!(external_url(page).is_err());
        }
    }
}
