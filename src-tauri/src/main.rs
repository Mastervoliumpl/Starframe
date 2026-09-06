#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod game_service;
mod model;

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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state: app::Shared = Arc::new(Mutex::new(app::Core::default()));
            app.manage(state.clone());
            app.manage(game_service::start(app.handle().clone(), state.clone()));
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
            commands::open_external,
            commands::game_action
        ])
        .run(tauri::generate_context!())
        .expect("Starframe could not start");
}
