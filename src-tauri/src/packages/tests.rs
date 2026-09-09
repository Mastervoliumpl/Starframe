use super::*;
use crate::catalog::refresh::Cache;
use std::{io::Cursor, net::TcpListener, time::Instant};
use zip::{ZipWriter, write::SimpleFileOptions};

mod fuzz;

#[tokio::test]
async fn new_downloads_require_signed_freshness_at_start_and_completion() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(root.path()).unwrap();
    let bytes = zip(&[("package/Core.dll", b"inert fixture")]);
    let approved = catalog(artifact(&bytes));
    storage
        .save_catalog_cache(&Cache {
            catalog: Some(approved.clone()),
            ..Default::default()
        })
        .unwrap();
    let mut queue = Packages::open(&mut storage).unwrap();
    let start = |queue: &mut Packages, storage: &mut Storage| {
        queue.start(storage, &Uuid::new_v4().to_string(), "fixture.core.1")
    };
    assert!(
        start(&mut queue, &mut storage)
            .unwrap_err()
            .contains("not been verified")
    );
    assert!(!queue.busy());
    let advisories = crate::catalog::advisories::Advisories {
        schema_version: 1,
        revision: "1".into(),
        advisories: vec![],
    };
    let verified =
        crate::catalog::authentication::tests::verified_catalog(&approved, &advisories).await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    storage.save_verified_catalog(&verified, now).unwrap();
    let operation = start(&mut queue, &mut storage).unwrap();
    let prepared = extract_fixture(&bytes).unwrap();
    queue.active.get_mut(&operation.id).unwrap().ready = Some(Ok(PreparedImport {
        prepared,
        local: None,
    }));
    // Advance only the fixture's saved expiry to exercise completion without a clock bypass in the app.
    let db = rusqlite::Connection::open(root.path().join("sqlite/state.db")).unwrap();
    db.execute(
        "UPDATE catalog_security SET record=json_set(record, '$.receivedAt', ?1, '$.expires', ?2)",
        rusqlite::params![(now - 2) as i64, (now - 1) as i64],
    )
    .unwrap();
    queue.poll(&mut storage).unwrap();
    let failed = storage
        .package_request(&operation.request_id)
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, Status::Failed);
    assert!(failed.message.contains("expired"));
    assert!(storage.load().unwrap().library.is_empty());
    assert!(
        start(&mut queue, &mut storage)
            .unwrap_err()
            .contains("expired")
    );
    assert!(!queue.busy());
}

fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn artifact(bytes: &[u8]) -> Artifact {
    Artifact {
        url: "https://example.invalid/core.zip".into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        size_bytes: bytes.len() as u64,
        layout: Layout::StarframeManagedZip {
            root: "package".into(),
            entry_assembly: "Core.dll".into(),
            entry_type: "Fixture.Core".into(),
        },
    }
}
fn catalog(artifact: Artifact) -> Catalog {
    Catalog {
        schema_version: 1,
        catalog_revision: "1".into(),
        mods: vec![crate::catalog::Mod {
            id: "fixture.core".into(),
            name: "Core".into(),
            author: "Test fixture".into(),
            source_url: "https://example.invalid/source".into(),
            description: String::new(),
            unmaintained: false,
            releases: vec![crate::catalog::Release {
                load_before: vec![],
                load_after: vec![],
                prefer_before: vec![],
                prefer_after: vec![],
                id: "fixture.core.1".into(),
                version: "1".into(),
                withdrawn: false,
                withdrawal_reason: None,
                compatibility_problems: vec![],
                artifact,
                requires: vec![],
                tested_game_builds: vec![],
            }],
        }],
    }
}
fn extract_fixture(bytes: &[u8]) -> Result<Prepared> {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("download.zip");
    let content = root.path().join("content");
    fs::write(&archive, bytes).unwrap();
    fs::create_dir(&content).unwrap();
    extract(&archive, &content, &artifact(bytes), &Cancel::default())
}

#[test]
fn runtime_file_limit_counts_files_not_zip_directories_for_managed_and_lua() {
    for lua in [false, true] {
        for count in [1024, 1025] {
            let root = tempfile::tempdir().unwrap();
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            writer
                .add_directory("empty/", SimpleFileOptions::default())
                .unwrap();
            for index in 0..count {
                let path = if lua {
                    format!("LJ/lua/file{index}.lua")
                } else if index == 0 {
                    "package/Core.dll".into()
                } else {
                    format!("package/file{index}.txt")
                };
                writer
                    .start_file(path, SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(b"fixture").unwrap();
            }
            let bytes = writer.finish().unwrap().into_inner();
            let mut metadata = artifact(&bytes);
            if lua {
                metadata.layout = Layout::StarframeLuaZip {};
            }
            fs::write(root.path().join("download.zip"), bytes).unwrap();
            fs::create_dir(root.path().join("content")).unwrap();
            let result = extract(
                &root.path().join("download.zip"),
                &root.path().join("content"),
                &metadata,
                &Cancel::default(),
            );
            if count == 1024 {
                assert_eq!(result.unwrap().files.len(), count);
            } else {
                assert!(result.unwrap_err().contains("1,024"));
            }
        }
    }
}

#[test]
fn runtime_size_limits_are_checked_before_ready_and_cached_reuse() {
    let file = |path: &str, size_bytes| PreparedFile {
        path: path.into(),
        sha256: "ab".repeat(32),
        size_bytes,
    };
    assert!(supported_files(&[file("package/Core.dll", 16 * 1024 * 1024)]).is_ok());
    assert!(supported_files(&[file("package/Core.dll", 16 * 1024 * 1024 + 1)]).is_err());
    assert!(supported_files(&[file("data.bin", 64 * 1024 * 1024)]).is_ok());
    assert!(supported_files(&[file("data.bin", 64 * 1024 * 1024 + 1)]).is_err());
    let mut files: Vec<_> = (0..4)
        .map(|i| file(&format!("data{i}.bin"), 64 * 1024 * 1024))
        .collect();
    assert!(supported_files(&files).is_ok());
    files.push(file("extra.bin", 1));
    assert!(supported_files(&files).is_err());
}

#[test]
fn lua_packages_preserve_cache_paths_and_reject_other_content() {
    for (path, valid) in [
        ("LJ/lua/fixture.lua", true),
        ("LJ/lua/AI/fixture.lua", false),
        ("Maps/fixture.sanmap", false),
        ("LJ/lua/Fixture.dll", false),
    ] {
        let root = tempfile::tempdir().unwrap();
        let bytes = zip(&[(path, b"return 1")]);
        let mut artifact = artifact(&bytes);
        artifact.layout = Layout::StarframeLuaZip {};
        let mut metadata = catalog(artifact.clone());
        assert!(metadata.validate().is_err());
        metadata.schema_version = 2;
        metadata.validate().unwrap();
        fs::write(root.path().join("download.zip"), bytes).unwrap();
        fs::create_dir(root.path().join("content")).unwrap();
        let result = extract(
            &root.path().join("download.zip"),
            &root.path().join("content"),
            &artifact,
            &Cancel::default(),
        );
        assert_eq!(result.is_ok(), valid, "{path}");
        if valid {
            assert_eq!(result.unwrap().files[0].path, path);
        }
    }
}
fn operation(artifact: &Artifact) -> Operation {
    Operation {
        id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        release_id: "fixture.core.1".into(),
        hash: artifact.sha256.clone(),
        status: Status::Preparing,
        message: "Preparing".into(),
        received_bytes: 0,
        total_bytes: artifact.size_bytes,
    }
}
fn server(responses: Vec<Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(&response);
        }
    });
    format!("http://{address}/package.zip")
}
fn response(bytes: &[u8], size: usize) -> Vec<u8> {
    let mut result =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n")
            .into_bytes();
    result.extend(bytes);
    result
}
fn test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}

#[test]
fn managed_zip_is_verified_without_loading_dlls_or_running_scripts() {
    let bytes = zip(&[
        ("package/Core.dll", b"inert DLL fixture"),
        ("package/install.ps1", b"throw 'must never execute'"),
        ("README.txt", b"fixture"),
    ]);
    let prepared = extract_fixture(&bytes).unwrap();
    assert_eq!(prepared.files.len(), 3);
    assert_eq!(
        prepared
            .files
            .iter()
            .find(|f| f.path == "package/Core.dll")
            .unwrap()
            .sha256,
        format!("{:x}", Sha256::digest(b"inert DLL fixture"))
    );
    assert!(
        extract_fixture(&zip(&[("other.dll", b"wrong layout")]))
            .unwrap_err()
            .contains("layout")
    );
}

#[test]
fn malicious_paths_collisions_links_and_declared_bombs_are_rejected() {
    for path in [
        "../escape.dll",
        "/escape.dll",
        "C:/escape.dll",
        "package\\escape.dll",
        "package/NUL.dll",
        "package/COM1.txt",
        "package/a:stream",
        "package/a.",
        "package/a ",
        "package//x",
        "package/./x",
        "package/café.dll",
    ] {
        let bytes = zip(&[("package/Core.dll", b"fixture"), (path, b"escape")]);
        assert!(extract_fixture(&bytes).is_err(), "accepted {path}");
    }
    for paths in [
        vec!["package/Core.dll", "package/core.dll"],
        vec!["package/Core.dll", "Package/other.dll"],
        vec!["package/Core.dll", "package/Core.dll/child"],
    ] {
        let entries: Vec<_> = paths
            .iter()
            .map(|name| (*name, b"data".as_slice()))
            .collect();
        assert!(extract_fixture(&zip(&entries)).is_err());
    }
    let good = zip(&[("package/Core.dll", b"fixture")]);
    let central = good.windows(4).position(|p| p == b"PK\x01\x02").unwrap();
    let mut symlink = good.clone();
    symlink[central + 5] = 3;
    symlink[central + 38..central + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
    assert!(extract_fixture(&symlink).is_err());
    let mut reparse = good.clone();
    reparse[central + 38..central + 42].copy_from_slice(&0x400u32.to_le_bytes());
    assert!(extract_fixture(&reparse).is_err());
    let mut bomb = good.clone();
    bomb[central + 24..central + 28].copy_from_slice(&(MAX_FILE_BYTES as u32 + 1).to_le_bytes());
    assert!(extract_fixture(&bomb).is_err());
    let mut count = good;
    let end = count.windows(4).rposition(|p| p == b"PK\x05\x06").unwrap();
    count[end + 8..end + 10].copy_from_slice(&4097u16.to_le_bytes());
    count[end + 10..end + 12].copy_from_slice(&4097u16.to_le_bytes());
    assert!(extract_fixture(&count).is_err());
}

#[test]
fn exact_archive_hash_is_rechecked_before_extraction() {
    let bytes = zip(&[("package/Core.dll", b"fixture")]);
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("download.zip");
    let content = root.path().join("content");
    fs::create_dir(&content).unwrap();
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    fs::write(&archive, changed).unwrap();
    assert!(
        extract(&archive, &content, &artifact(&bytes), &Cancel::default())
            .unwrap_err()
            .contains("SHA-256")
    );
    assert_eq!(fs::read_dir(content).unwrap().count(), 0);
}

#[test]
fn transfers_retry_interruption_and_preserve_exact_identity() {
    tauri::async_runtime::block_on(async {
        let bytes = zip(&[("package/Core.dll", b"fixture")]);
        let root = tempfile::tempdir().unwrap();
        let mut expected = artifact(&bytes);
        expected.url = server(vec![
            response(&bytes[..10], bytes.len()),
            response(&bytes, bytes.len()),
        ]);
        let mut output = tokio::fs::File::create(root.path().join("download.zip"))
            .await
            .unwrap();
        let progress = AtomicU64::new(0);
        transfer::download(&test_client(), &expected, &mut output, &progress)
            .await
            .unwrap();
        assert_eq!(fs::read(root.path().join("download.zip")).unwrap(), bytes);
        assert_eq!(progress.load(Ordering::Relaxed), expected.size_bytes);
        let mut changed = bytes.clone();
        changed[10] ^= 1;
        expected.url = server(vec![response(&changed, changed.len())]);
        assert!(
            transfer::download(&test_client(), &expected, &mut output, &progress)
                .await
                .unwrap_err()
                .contains("SHA-256")
        );
        expected.url = server(vec![response(&bytes, bytes.len() + 1)]);
        assert!(
            transfer::download(&test_client(), &expected, &mut output, &progress)
                .await
                .unwrap_err()
                .contains("size")
        );
    });
}

#[test]
fn preparation_promotes_verified_content_and_recovers_before_database_commit() {
    tauri::async_runtime::block_on(async {
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("game");
        fs::create_dir(&game).unwrap();
        fs::write(game.join("sentinel"), b"unchanged").unwrap();
        let data = root.path().join("data");
        let mut storage = Storage::open(&data).unwrap();
        let bytes = zip(&[("package/Core.dll", b"fixture")]);
        let approved = artifact(&bytes);
        let catalog = catalog(approved.clone());
        storage
            .save_catalog_cache(&Cache {
                catalog: Some(catalog.clone()),
                ..Default::default()
            })
            .unwrap();
        let mut operation = operation(&approved);
        storage.save_package(&operation).unwrap();
        let mut source = approved.clone();
        source.url = server(vec![response(&bytes, bytes.len())]);
        let prepared = prepare(
            data.clone(),
            operation.id.clone(),
            source,
            None,
            test_client(),
            Cancel::default(),
            Arc::new(AtomicU64::new(0)),
        )
        .await
        .unwrap();
        assert!(storage.load().unwrap().library.is_empty());
        drop(storage);
        let mut storage = Storage::open(&data).unwrap();
        let mut queue = Packages::open(&mut storage).unwrap();
        assert_eq!(
            storage.package_operations().unwrap()[0].status,
            Status::Failed
        );
        assert!(
            data.join("artifacts")
                .join(&approved.sha256)
                .join("package/Core.dll")
                .is_file()
        );
        let (entry, _) = resolve(&catalog, "fixture.core.1").unwrap();
        operation.status = Status::Completed;
        storage
            .complete_package(&operation, &entry, &prepared)
            .unwrap();
        let request = Uuid::new_v4().to_string();
        let reuse = queue
            .start(&mut storage, &request, "fixture.core.1")
            .unwrap();
        assert_eq!(
            queue
                .start(&mut storage, &request, "fixture.core.1")
                .unwrap()
                .id,
            reuse.id
        );
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until && !queue.active.is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
            queue.poll(&mut storage).unwrap();
        }
        assert_eq!(
            storage.package_request(&request).unwrap().unwrap().status,
            Status::Completed
        );
        assert_eq!(storage.load().unwrap().library.len(), 1);
        assert_eq!(fs::read(game.join("sentinel")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&game).unwrap().count(), 1);
        fs::write(
            data.join("artifacts")
                .join(&approved.sha256)
                .join("package/Core.dll"),
            b"changed",
        )
        .unwrap();
        let error = prepare(
            data,
            Uuid::new_v4().to_string(),
            approved,
            Some(prepared),
            test_client(),
            Cancel::default(),
            Arc::new(AtomicU64::new(0)),
        )
        .await
        .unwrap_err();
        assert!(error.contains("changed") || error.contains("expected size"));
    });
}

#[test]
fn cancellation_wakes_a_stalled_transfer_and_never_promotes_partial_data() {
    tauri::async_runtime::block_on(async {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (connected, connection) = mpsc::channel();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            connected.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(300));
            drop(stream);
        });
        let mut expected = artifact(b"fixture");
        expected.url = format!("http://{address}/stalled.zip");
        let cancel = Cancel::default();
        let worker_cancel = cancel.clone();
        let task = tauri::async_runtime::spawn(prepare(
            root.path().into(),
            Uuid::new_v4().to_string(),
            expected,
            None,
            test_client(),
            worker_cancel,
            Arc::new(AtomicU64::new(0)),
        ));
        connection.recv_timeout(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        cancel.cancel();
        assert_eq!(task.await.unwrap().unwrap_err(), CANCELLED);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            fs::read_dir(root.path().join("artifacts")).unwrap().count(),
            0
        );
    });
}

#[test]
fn unapproved_and_withdrawn_dependencies_cannot_start_work() {
    let mut catalog = catalog(artifact(b"fixture"));
    assert!(resolve(&catalog, "unknown").is_err());
    catalog.mods[0].releases[0]
        .requires
        .push("unapproved.dependency".into());
    assert!(resolve(&catalog, "fixture.core.1").is_err());
    catalog.mods[0].releases[0].requires.clear();
    catalog.mods[0].releases[0].withdrawn = true;
    assert!(
        resolve(&catalog, "fixture.core.1")
            .unwrap_err()
            .contains("withdrawn")
    );
}

#[test]
fn three_workers_are_bounded_and_cancellation_wins_before_commit() {
    tauri::async_runtime::block_on(async {
        let root = tempfile::tempdir().unwrap();
        let mut storage = Storage::open(root.path()).unwrap();
        let mut approved = Catalog {
            schema_version: 1,
            catalog_revision: "1".into(),
            mods: vec![],
        };
        for index in 0..4 {
            let bytes = zip(&[("package/Core.dll", format!("fixture {index}").as_bytes())]);
            let artifact = artifact(&bytes);
            let mut owner = catalog(artifact.clone()).mods.remove(0);
            owner.id = format!("fixture.{index}");
            owner.releases[0].id = format!("fixture.{index}.1");
            let path = root
                .path()
                .join("artifacts")
                .join(&artifact.sha256)
                .join("package");
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("Core.dll"), format!("fixture {index}")).unwrap();
            approved.mods.push(owner);
            let (entry, _) = resolve(&approved, &format!("fixture.{index}.1")).unwrap();
            let mut op = operation(&artifact);
            op.release_id = format!("fixture.{index}.1");
            storage.save_package(&op).unwrap();
            op.status = Status::Completed;
            storage
                .complete_package(&op, &entry, &extract_fixture(&bytes).unwrap())
                .unwrap();
        }
        storage
            .save_catalog_cache(&Cache {
                catalog: Some(approved),
                ..Default::default()
            })
            .unwrap();
        let mut queue = Packages::open(&mut storage).unwrap();
        let mut ids = vec![];
        for index in 0..3 {
            ids.push(
                queue
                    .start(
                        &mut storage,
                        &Uuid::new_v4().to_string(),
                        &format!("fixture.{index}.1"),
                    )
                    .unwrap()
                    .id,
            );
        }
        assert!(
            queue
                .start(&mut storage, &Uuid::new_v4().to_string(), "fixture.3.1")
                .unwrap_err()
                .contains("Three")
        );
        queue.cancel(&mut storage, &ids[0]).unwrap();
        assert_eq!(
            queue
                .operations(&storage)
                .unwrap()
                .iter()
                .find(|op| op.id == ids[0])
                .unwrap()
                .status,
            Status::Cancelling
        );
        let until = Instant::now() + Duration::from_secs(5);
        while !queue.active.is_empty() && Instant::now() < until {
            tokio::time::sleep(Duration::from_millis(10)).await;
            queue.poll(&mut storage).unwrap();
        }
        assert!(queue.active.is_empty());
        assert_eq!(
            storage
                .package_operations()
                .unwrap()
                .iter()
                .find(|op| op.id == ids[0])
                .unwrap()
                .status,
            Status::Cancelled
        );
        assert_eq!(storage.load().unwrap().library.len(), 4);
    });
}

#[test]
fn database_failure_rolls_back_manifest_library_and_completion_together() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(root.path()).unwrap();
    let bytes = zip(&[("package/Core.dll", b"fixture")]);
    let artifact = artifact(&bytes);
    let (entry, _) = resolve(&catalog(artifact.clone()), "fixture.core.1").unwrap();
    let mut op = operation(&artifact);
    storage.save_package(&op).unwrap();
    let db = rusqlite::Connection::open(root.path().join("sqlite/state.db")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_package BEFORE INSERT ON library BEGIN SELECT RAISE(ABORT, 'fixture storage failure'); END;").unwrap();
    op.status = Status::Completed;
    assert!(
        storage
            .complete_package(&op, &entry, &extract_fixture(&bytes).unwrap())
            .is_err()
    );
    assert!(
        storage
            .prepared_artifact(&artifact.sha256)
            .unwrap()
            .is_none()
    );
    assert!(storage.load().unwrap().library.is_empty());
    assert_eq!(storage.load().unwrap().revision, 0);
    assert_eq!(
        storage
            .package_request(&op.request_id)
            .unwrap()
            .unwrap()
            .status,
        Status::Preparing
    );
    drop(db);
    drop(storage);
    let mut storage = Storage::open(root.path()).unwrap();
    Packages::open(&mut storage).unwrap();
    assert_eq!(
        storage
            .package_request(&op.request_id)
            .unwrap()
            .unwrap()
            .status,
        Status::Failed
    );
}

#[test]
fn cancellation_preserves_cleanup_failures_in_operation_history() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(root.path()).unwrap();
    let artifact = artifact(b"fixture");
    let (entry, _) = resolve(&catalog(artifact.clone()), "fixture.core.1").unwrap();
    let op = operation(&artifact);
    storage.save_package(&op).unwrap();
    let mut queue = Packages::open(&mut storage).unwrap();
    let (sender, result) = mpsc::sync_channel(1);
    let message = format!(
        "{CANCELLED} Staging {} was retained because cleanup failed: file is locked",
        op.id
    );
    sender.send(Err(message.clone())).unwrap();
    queue.active.insert(
        op.id.clone(),
        Active {
            operation: op.clone(),
            entry: Some(entry),
            source: None,
            cancel: Cancel::default(),
            progress: Arc::new(AtomicU64::new(0)),
            result,
            ready: None,
        },
    );
    queue.cancel(&mut storage, &op.id).unwrap();
    queue.poll(&mut storage).unwrap();
    let saved = storage.package_request(&op.request_id).unwrap().unwrap();
    assert_eq!(saved.status, Status::Failed);
    assert_eq!(saved.message, message);
}

#[cfg(windows)]
#[test]
fn junctions_cannot_redirect_staging_to_a_game_directory() {
    let root = tempfile::tempdir().unwrap();
    let game = root.path().join("game");
    let data = root.path().join("data");
    fs::create_dir(&game).unwrap();
    fs::create_dir(&data).unwrap();
    fs::write(game.join("sentinel"), b"unchanged").unwrap();
    let junction = data.join("package-staging");
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&junction)
        .arg(&game)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(
        Directory::open(&data)
            .unwrap()
            .directory("package-staging/operation")
            .is_err()
    );
    assert_eq!(fs::read(game.join("sentinel")).unwrap(), b"unchanged");
    assert_eq!(fs::read_dir(&game).unwrap().count(), 1);
    fs::remove_dir(junction).unwrap();
}
