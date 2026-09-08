use super::*;
use std::{cell::Cell, process::Command};
use tempfile::TempDir;

fn fixture() -> (TempDir, Storage, Engine) {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("engine")).unwrap();
    let storage = Storage::open(&temp.path().join("data")).unwrap();
    let engine = Engine::open(&temp.path().join("engine"), &mut || Ok(())).unwrap();
    (temp, storage, engine)
}
fn files(version: u8) -> Vec<(String, Vec<u8>)> {
    vec![
        ("winhttp.dll".into(), vec![version; 100]),
        ("BepInEx/core/runtime.dll".into(), vec![version; 40]),
    ]
}
fn verify(store: &Storage, root: &Path) {
    let record = load(store, root).unwrap();
    assert!(record.pending.is_none());
    for (path, expected) in &record.owned {
        assert_eq!(&hash(&read(&root.join(path)).unwrap().unwrap()), expected);
    }
}
#[test]
fn install_update_restore_backup_and_remove_preserve_unowned_files() {
    let (temp, mut store, mut engine) = fixture();
    fs::write(engine.root.join("game-original.txt"), b"original").unwrap();
    apply(
        &mut store,
        &mut engine,
        files(1),
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    apply(
        &mut store,
        &mut engine,
        files(2),
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        store.deployment_blob(&hash(&[1; 100])).unwrap(),
        vec![1; 100]
    );
    let backup = store.backup().unwrap();
    let restored = Storage::restore_into(&backup, &temp.path().join("restored")).unwrap();
    verify(&restored, &engine.root);
    assert_eq!(
        restored.deployment_blob(&hash(&[1; 100])).unwrap(),
        vec![1; 100]
    );
    apply(
        &mut store,
        &mut engine,
        Vec::new(),
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        fs::read(engine.root.join("game-original.txt")).unwrap(),
        b"original"
    );
    assert!(!engine.root.join("winhttp.dll").exists());
    verify(&store, &engine.root);
}
#[test]
fn matching_external_files_are_borrowed_never_adopted() {
    let (_temp, mut store, mut engine) = fixture();
    fs::write(engine.root.join("winhttp.dll"), vec![1; 100]).unwrap();
    assert_eq!(
        apply(
            &mut store,
            &mut engine,
            files(1),
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .unwrap(),
        1
    );
    apply(
        &mut store,
        &mut engine,
        Vec::new(),
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        fs::read(engine.root.join("winhttp.dll")).unwrap(),
        vec![1; 100]
    );
}
#[test]
fn conflicts_fail_before_mutation_and_changed_owned_files_are_retained() {
    let (_temp, mut store, mut engine) = fixture();
    fs::write(engine.root.join("winhttp.dll"), b"foreign loader").unwrap();
    assert!(
        apply(
            &mut store,
            &mut engine,
            files(1),
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .unwrap_err()
        .contains("Unowned")
    );
    assert!(!engine.root.join("BepInEx").exists());
    fs::remove_file(engine.root.join("winhttp.dll")).unwrap();
    apply(
        &mut store,
        &mut engine,
        files(1),
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    fs::write(engine.root.join("winhttp.dll"), b"user edit").unwrap();
    assert!(
        apply(
            &mut store,
            &mut engine,
            Vec::new(),
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .is_err()
    );
    assert_eq!(
        fs::read(engine.root.join("winhttp.dll")).unwrap(),
        b"user edit"
    );
}
#[test]
fn game_start_defers_rollback_then_closed_game_can_recover() {
    let (_temp, mut store, mut engine) = fixture();
    let running = Cell::new(false);
    let mut guard = || {
        if running.get() {
            Err("Game running or unknown".into())
        } else {
            Ok(())
        }
    };
    let mut event = |name: &str| {
        if name == "file-changed" {
            running.set(true);
        }
        Ok(())
    };
    assert!(apply(&mut store, &mut engine, files(1), &mut guard, &mut event).is_err());
    let mut record = load(&store, &engine.root).unwrap();
    assert!(record.pending.is_some());
    let before = read(&engine.root.join("BepInEx/core/runtime.dll")).unwrap();
    assert!(
        rollback(
            &mut store,
            &mut engine,
            &mut record,
            &mut guard,
            &mut |_| Ok(())
        )
        .is_err()
    );
    assert_eq!(
        read(&engine.root.join("BepInEx/core/runtime.dll")).unwrap(),
        before
    );
    running.set(false);
    rollback(
        &mut store,
        &mut engine,
        &mut record,
        &mut guard,
        &mut |_| Ok(()),
    )
    .unwrap();
    verify(&store, &engine.root);
    assert!(!engine.root.join("BepInEx/core/runtime.dll").exists());
}
#[test]
fn recovery_preserves_external_changes_and_can_retry_after_manual_resolution() {
    let (_temp, mut store, mut engine) = fixture();
    let running = Cell::new(false);
    let mut guard = || {
        if running.get() {
            Err("deferred".into())
        } else {
            Ok(())
        }
    };
    assert!(
        apply(&mut store, &mut engine, files(1), &mut guard, &mut |name| {
            if name == "file-changed" {
                running.set(true);
            }
            Ok(())
        })
        .is_err()
    );
    running.set(false);
    let target = engine.root.join("BepInEx/core/runtime.dll");
    fs::write(&target, b"external edit").unwrap();
    let mut record = load(&store, &engine.root).unwrap();
    assert!(
        rollback(
            &mut store,
            &mut engine,
            &mut record,
            &mut guard,
            &mut |_| Ok(())
        )
        .unwrap_err()
        .contains("unknown content")
    );
    assert_eq!(fs::read(&target).unwrap(), b"external edit");
    fs::write(&target, vec![1; 40]).unwrap();
    rollback(
        &mut store,
        &mut engine,
        &mut record,
        &mut guard,
        &mut |_| Ok(()),
    )
    .unwrap();
    verify(&store, &engine.root);
}
#[test]
fn installation_lock_and_unknown_game_state_prevent_new_writes() {
    let (_temp, _store, engine) = fixture();
    assert!(Engine::open(&engine.root, &mut || Ok(())).is_err());
    let root = engine.root.clone();
    drop(engine);
    assert!(Engine::open(&root, &mut || Err("unknown process".into())).is_err());
    assert!(Engine::open(&root, &mut || Ok(())).is_ok());
}
#[test]
fn invalid_paths_and_case_aliases_never_become_a_plan() {
    let (_temp, mut store, mut engine) = fixture();
    for name in [
        "../outside.dll",
        "CON.dll",
        "C:/outside.dll",
        "a\\b.dll",
        "BepInEx/core/../outside.dll",
    ] {
        assert!(
            apply(
                &mut store,
                &mut engine,
                vec![(name.into(), vec![1])],
                &mut || Ok(()),
                &mut |_| Ok(())
            )
            .is_err()
        );
    }
    assert!(
        apply(
            &mut store,
            &mut engine,
            vec![("a.dll".into(), vec![1]), ("A.dll".into(), vec![2])],
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .is_err()
    );
    verify(&store, &engine.root);
}
#[test]
fn external_plugins_require_retaining_the_shared_loader() {
    let (_temp, _store, engine) = fixture();
    let plugins = engine.root.join("BepInEx/plugins");
    fs::create_dir_all(&plugins).unwrap();
    fs::write(plugins.join("Other.DLL"), b"external").unwrap();
    assert!(external_dll(&plugins, &engine.root, &Files::new(), 0, &mut 0).unwrap());
}
#[cfg(windows)]
#[test]
fn locked_destination_failure_restores_the_previous_deployment() {
    use std::os::windows::fs::OpenOptionsExt;
    let (_temp, mut store, mut engine) = fixture();
    apply(
        &mut store,
        &mut engine,
        files(1),
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    let held = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(engine.root.join("winhttp.dll"))
        .unwrap();
    assert!(
        apply(
            &mut store,
            &mut engine,
            files(2),
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .is_err()
    );
    drop(held);
    verify(&store, &engine.root);
    assert_eq!(
        fs::read(engine.root.join("BepInEx/core/runtime.dll")).unwrap(),
        vec![1; 40]
    );
}
#[cfg(windows)]
#[test]
fn junction_parent_is_rejected_without_touching_the_destination() {
    use std::os::windows::process::CommandExt;
    let (temp, mut store, mut engine) = fixture();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let output = Command::new("cmd.exe")
        .args(["/c", "mklink", "/J"])
        .arg(engine.root.join("BepInEx"))
        .arg(&outside)
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        apply(
            &mut store,
            &mut engine,
            files(1),
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}
#[cfg(windows)]
#[test]
fn public_recovery_revalidates_game_and_does_nothing_without_a_journal() {
    let (temp, mut store, mut engine) = fixture();
    crate::game::tests::fixture(temp.path(), "engine");
    let game = game::inspect(temp.path()).unwrap();
    let running = Cell::new(false);
    assert!(
        apply(
            &mut store,
            &mut engine,
            files(1),
            &mut || if running.get() {
                Err("deferred".into())
            } else {
                Ok(())
            },
            &mut |name| {
                if name == "file-changed" {
                    running.set(true);
                }
                Ok(())
            }
        )
        .is_err()
    );
    let root = engine.root.clone();
    drop(engine);
    assert!(recover(&mut store, &game).unwrap());
    verify(&store, &root);
    assert!(!recover(&mut store, &game).unwrap());
}

fn child(root: &Path, action: &str, checkpoint: usize) {
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "deployment::tests::crash_worker", "--ignored"])
        .env("STARFRAME_DEPLOY_TEST_ROOT", root)
        .env("STARFRAME_DEPLOY_TEST_ACTION", action)
        .env("STARFRAME_DEPLOY_TEST_STOP", checkpoint.to_string())
        .output()
        .unwrap();
    assert_eq!(
        status.status.code(),
        Some(86),
        "{} {}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
}
#[test]
fn process_exit_at_each_persisted_boundary_recovers_install_update_and_remove() {
    for action in ["install", "update", "remove"] {
        for checkpoint in 1..=if action == "remove" { 5 } else { 7 } {
            let (temp, mut store, mut engine) = fixture();
            if action != "install" {
                apply(
                    &mut store,
                    &mut engine,
                    files(1),
                    &mut || Ok(()),
                    &mut |_| Ok(()),
                )
                .unwrap();
            }
            let root = engine.root.clone();
            drop(engine);
            drop(store);
            child(temp.path(), action, checkpoint);
            let mut store = Storage::open(&temp.path().join("data")).unwrap();
            let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
            let mut record = load(&store, &root).unwrap();
            rollback(
                &mut store,
                &mut engine,
                &mut record,
                &mut || Ok(()),
                &mut |_| Ok(()),
            )
            .unwrap();
            verify(&store, &root);
            let committed = checkpoint == if action == "remove" { 5 } else { 7 };
            let expected = match (action, committed) {
                ("install", false) | ("remove", true) => None,
                ("update", true) => Some(vec![2; 100]),
                _ => Some(vec![1; 100]),
            };
            assert_eq!(
                read(&root.join("winhttp.dll")).unwrap(),
                expected,
                "{action} checkpoint {checkpoint}"
            );
        }
    }
}
#[test]
fn process_exit_during_rollback_can_recover_again() {
    for checkpoint in 1..=3 {
        let (temp, mut store, mut engine) = fixture();
        apply(
            &mut store,
            &mut engine,
            files(1),
            &mut || Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
        let root = engine.root.clone();
        drop(engine);
        drop(store);
        child(temp.path(), "update", 3);
        child(temp.path(), "recover", checkpoint);
        let mut store = Storage::open(&temp.path().join("data")).unwrap();
        let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
        let mut record = load(&store, &root).unwrap();
        rollback(
            &mut store,
            &mut engine,
            &mut record,
            &mut || Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
        verify(&store, &root);
        assert_eq!(
            fs::read(root.join("BepInEx/core/runtime.dll")).unwrap(),
            vec![1; 40]
        );
    }
}
#[test]
#[ignore = "worker exits at durable deployment boundaries; invoked by parent tests"]
fn crash_worker() {
    let root = PathBuf::from(std::env::var_os("STARFRAME_DEPLOY_TEST_ROOT").unwrap());
    let action = std::env::var("STARFRAME_DEPLOY_TEST_ACTION").unwrap();
    let stop: usize = std::env::var("STARFRAME_DEPLOY_TEST_STOP")
        .unwrap()
        .parse()
        .unwrap();
    let mut count = 0;
    let mut event = |_: &str| {
        count += 1;
        if count == stop {
            std::process::exit(86);
        }
        Ok(())
    };
    let mut store = Storage::open(&root.join("data")).unwrap();
    let mut engine = Engine::open(&root.join("engine"), &mut || Ok(())).unwrap();
    if action == "recover" {
        let mut record = load(&store, &engine.root).unwrap();
        rollback(
            &mut store,
            &mut engine,
            &mut record,
            &mut || Ok(()),
            &mut event,
        )
        .unwrap();
    } else {
        apply(
            &mut store,
            &mut engine,
            if action == "remove" {
                Vec::new()
            } else {
                files(if action == "update" { 2 } else { 1 })
            },
            &mut || Ok(()),
            &mut event,
        )
        .unwrap();
    }
}

#[test]
fn prepared_runtime_inventory_rejects_unlisted_content_and_hash_changes() {
    let root = std::env::temp_dir().join(format!("starframe-runtime-package-{}", Uuid::new_v4()));
    let plugin = root.join("BepInEx/plugins/Starframe");
    fs::create_dir_all(&plugin).unwrap();
    fs::create_dir_all(root.join("Starframe")).unwrap();
    let mut inventory = Vec::new();
    for name in ["Starframe.Bootstrap.dll", "Starframe.Runtime.dll"] {
        let path = format!("BepInEx/plugins/Starframe/{name}");
        fs::write(root.join(&path), b"fixture").unwrap();
        inventory.push(serde_json::json!({"path":path,"sha256":hash(b"fixture")}));
    }
    let manifest = include_bytes!("../../../contracts/fixtures/activation-empty.json");
    fs::write(root.join("Starframe/activation.json"), manifest).unwrap();
    inventory.push(serde_json::json!({"path":"Starframe/activation.json","sha256":hash(manifest)}));
    let index = root.join("runtime-package.json");
    fs::write(&index, serde_json::to_vec(&inventory).unwrap()).unwrap();
    assert_eq!(runtime_payload(&root).unwrap().len(), 3);
    fs::write(plugin.join("Starframe.Runtime.dll"), b"changed").unwrap();
    assert!(runtime_payload(&root).unwrap_err().contains("hash/path"));
    fs::write(plugin.join("Starframe.Runtime.dll"), b"fixture").unwrap();
    fs::create_dir_all(root.join("Starframe/mods/disabled")).unwrap();
    fs::write(root.join("Starframe/mods/disabled/extra.dll"), b"fixture").unwrap();
    inventory.push(
        serde_json::json!({"path":"Starframe/mods/disabled/extra.dll","sha256":hash(b"fixture")}),
    );
    fs::write(&index, serde_json::to_vec(&inventory).unwrap()).unwrap();
    assert!(runtime_payload(&root).unwrap_err().contains("Unlisted"));
    fs::remove_dir_all(root).unwrap();
}
