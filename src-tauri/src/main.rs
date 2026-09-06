#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod model;

use starframe::storage::Storage;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::Manager;

fn data_directory(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("STARFRAME_TEST_DATA_DIR") {
        return Ok(path.into());
    }
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state: app::Shared = Arc::new(Mutex::new(app::Core::default()));
            app.manage(state.clone());
            app.manage(Mutex::new(None::<Storage>));
            let handle = app.handle().clone();
            let storage_state = state.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let result = tauri::async_runtime::block_on(async {
                    let root = data_directory(&handle)?;
                    let storage = Storage::open(&root)
                        .await
                        .map_err(|error| format!("{error} Data folder: {}", root.display()))?;
                    let records = storage.load().await.map_err(|error| error.to_string())?;
                    let status = model::SavedData::Ready {
                        active_collection_name: records
                            .collections
                            .iter()
                            .find(|collection| {
                                Some(&collection.id) == records.active_collection.as_ref()
                            })
                            .map(|collection| collection.name.clone()),
                        revision: records.revision.to_string(),
                        library_count: records
                            .library
                            .len()
                            .try_into()
                            .map_err(|_| "Too many library records.")?,
                        collection_count: records
                            .collections
                            .len()
                            .try_into()
                            .map_err(|_| "Too many collections.")?,
                    };
                    *handle
                        .state::<Mutex<Option<Storage>>>()
                        .lock()
                        .expect("storage lock") = Some(storage);
                    Ok::<_, String>(status)
                });
                storage_state.lock().expect("state lock").saved_data(
                    result.unwrap_or_else(|message| model::SavedData::Unavailable { message }),
                );
            });
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(2));
                    let mut core = state.lock().expect("state lock");
                    if core.stopped {
                        break;
                    }
                    core.publish();
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window
                    .state::<app::Shared>()
                    .lock()
                    .expect("state lock")
                    .stopped = true;
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::watch_state,
            commands::start_diagnostic,
            commands::cancel_operation,
            commands::open_external
        ])
        .run(tauri::generate_context!())
        .expect("Starframe could not start");
}
