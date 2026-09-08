use crate::{
    app::{Shared, start_worker},
    model::{CommandError, Snapshot},
};
use tauri::{Manager, State, ipc::Channel};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

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
    action: starframe::sharing::Action,
) -> Result<starframe::sharing::Reply, CommandError> {
    service.sharing(action).await
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
        _ => Err(CommandError::new(
            "invalid_link",
            "This external page is not available.",
        )),
    }
}

#[tauri::command]
pub async fn open_external(
    app: tauri::AppHandle,
    service: State<'_, crate::game_service::GameService>,
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
    let url = if let Some(id) = page.strip_prefix("mod:") {
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
