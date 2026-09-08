fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "watch_state",
            "start_diagnostic",
            "cancel_operation",
            "open_external",
            "game_action",
            "package_action",
            "mod_action",
            "sharing_action",
            "save_collection_file",
        ]),
    ))
    .expect("Tauri build configuration");
}
