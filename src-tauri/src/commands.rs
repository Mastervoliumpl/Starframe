use crate::{
    app::{Shared, start_worker},
    model::{CommandError, Snapshot},
};
use tauri::{State, ipc::Channel};
use tauri_plugin_opener::OpenerExt;

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
pub fn open_external(app: tauri::AppHandle, page: String) -> Result<(), CommandError> {
    app.opener()
        .open_url(external_url(&page)?, None::<&str>)
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
