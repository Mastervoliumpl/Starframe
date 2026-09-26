use super::*;
use std::{
    process::{Child, Command, Stdio},
    time::Instant,
};

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "startup/backup worker terminated by parent test"]
fn production_worker() {
    let root = PathBuf::from(std::env::var_os("STARFRAME_PROOF_SOURCE").unwrap());
    let store = Storage::open(&root).unwrap();
    if std::env::var("STARFRAME_PROOF_MODE")
        .unwrap()
        .starts_with("backup-")
    {
        store.backup().unwrap();
    }
}

#[test]
fn interrupted_current_startup_and_backups_retry_without_replacing_records_or_sources() {
    for phase in [
        "before-switch",
        "after-switch",
        "backup-written",
        "backup-complete",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("data");
        fs::create_dir(&root).unwrap();
        let source = root.join("original.lua");
        fs::write(&source, b"original source").unwrap();
        let expected = if phase.starts_with("backup-") {
            let mut store = Storage::open(&root).unwrap();
            let entry = LibraryEntry {
                reference: ModReference {
                    mod_id: "fixture".into(),
                    hash: "a".repeat(64),
                    origin: Origin::LocalImport,
                    release_id: None,
                },
                name: "Fixture".into(),
                author: "Fixture".into(),
                version: "1".into(),
            };
            store.put_library_entry(&entry, 0).unwrap();
            store.load().unwrap()
        } else {
            Records {
                revision: 0,
                library: vec![],
                collections: vec![],
                active_collection: None,
            }
        };
        let mut child = Worker(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "storage::sqlite_proof::production_worker",
                    "--ignored",
                    "--nocapture",
                ])
                .env("STARFRAME_PROOF_SOURCE", &root)
                .env("STARFRAME_PROOF_MODE", phase)
                .env("STARFRAME_STORAGE_STOP", phase)
                .env("STARFRAME_STORAGE_SIGNAL", root.join("ready"))
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        let until = Instant::now() + Duration::from_secs(30);
        while !root.join("ready").exists() && Instant::now() < until {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "{phase}: worker exited before signal"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(root.join("ready").exists(), "{phase}: missing signal");
        drop(child);
        let store = Storage::open(&root).unwrap();
        assert_eq!(store.load().unwrap(), expected, "{phase}");
        assert_eq!(fs::read(&source).unwrap(), b"original source");
        if phase.starts_with("backup-") {
            let backup = fs::read_dir(root.join("backups"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let result = Storage::restore_into(&backup, &temp.path().join("restored"));
            assert_eq!(result.is_ok(), phase == "backup-complete");
            if let Ok(restored) = result {
                assert_eq!(restored.load().unwrap(), expected);
            }
        }
    }
}

#[test]
fn incomplete_active_destination_and_obsolete_database_are_retained() {
    for incomplete in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("state.db"), b"obsolete database").unwrap();
        fs::write(root.join("state.db-wal"), b"retained wal").unwrap();
        if incomplete {
            fs::create_dir(root.join("sqlite")).unwrap();
            fs::write(root.join("sqlite/state.db"), b"incomplete current database").unwrap();
        }
        assert!(Storage::open(root).is_err());
        assert_eq!(
            fs::read(root.join("state.db")).unwrap(),
            b"obsolete database"
        );
        assert_eq!(
            fs::read(root.join("state.db-wal")).unwrap(),
            b"retained wal"
        );
        assert!(!root.join("sqlite/complete").exists());
        if incomplete {
            assert_eq!(
                fs::read(root.join("sqlite/state.db")).unwrap(),
                b"incomplete current database"
            );
        }
    }
}
