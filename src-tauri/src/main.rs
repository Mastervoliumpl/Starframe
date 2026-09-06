#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod model;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::Manager;

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
