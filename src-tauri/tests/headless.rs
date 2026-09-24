#![cfg(windows)]

use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn command(data: &Path, name: &str, args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_starframe_headless"))
        .arg(data)
        .arg(name)
        .args(args)
        .output()
        .unwrap();
    let bytes = if output.status.success() {
        output.stdout
    } else {
        output.stderr
    };
    let last = bytes
        .rsplit(|byte| *byte == b'\n')
        .find(|line| !line.is_empty())
        .unwrap();
    (
        output.status.success(),
        serde_json::from_slice(last).unwrap(),
    )
}

#[test]
fn headless_and_gui_share_local_operations_and_storage_ownership() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-results/0.7.0-headless");
    fs::create_dir_all(&fixtures).unwrap();
    let root = tempfile::tempdir_in(fixtures.canonicalize().unwrap()).unwrap();
    let data = root.path().join("data");
    let source = root.path().join("source");
    fs::create_dir_all(source.join("LJ/lua")).unwrap();
    fs::write(source.join("LJ/lua/local.lua"), b"return 'fixture'").unwrap();
    fs::write(
        source.join("starframe.local.json"),
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "modId": "fixture.headless",
            "name": "Headless fixture",
            "author": "Test fixture",
            "version": "dev.1",
            "layout": {"kind": "starframe_lua_zip"}
        }))
        .unwrap(),
    )
    .unwrap();

    let (ok, import) = command(&data, "import", &[source.to_str().unwrap()]);
    assert!(ok, "{import}");
    assert_eq!(import["data"]["status"], "completed");
    let (ok, created) = command(&data, "collection-create", &["Headless collection"]);
    assert!(ok, "{created}");
    let id = created["data"]["collections"][0]["id"].as_str().unwrap();
    let (ok, selected) = command(&data, "collection-select", &[id]);
    assert!(ok, "{selected}");
    assert_eq!(selected["data"]["activeCollection"], id);
    let (ok, restarted) = command(&data, "status", &[]);
    assert!(ok, "{restarted}");
    assert_eq!(
        restarted["data"]["mods"]["library"][0]["reference"]["modId"],
        "fixture.headless"
    );
    assert_eq!(restarted["data"]["mods"]["activeCollection"], id);

    let owner = starframe::storage::Storage::open(&data).unwrap();
    let (ok, blocked) = command(&data, "status", &[]);
    assert!(!ok);
    assert!(
        blocked["error"]["message"]
            .as_str()
            .unwrap()
            .contains("another process")
    );
    drop(owner);
}
