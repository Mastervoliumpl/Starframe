#[test]
fn desktop_configuration_is_accepted_by_tauri() {
    serde_json::from_str::<tauri::Config>(include_str!("../tauri.conf.json"))
        .expect("the desktop configuration must match Tauri's schema");
}

#[test]
fn malformed_window_fixture_is_rejected() {
    let fixture = include_str!("fixtures/invalid-window.json");
    assert!(serde_json::from_str::<tauri::Config>(fixture).is_err());
}
