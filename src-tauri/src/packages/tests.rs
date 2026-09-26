use super::*;
use std::io::Cursor;
use zip::{ZipWriter, write::SimpleFileOptions};

mod fuzz;

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
fn artifact(bytes: &[u8]) -> Archive {
    Archive {
        sha256: format!("{:x}", Sha256::digest(bytes)),
        size_bytes: bytes.len() as u64,
        layout: Layout::StarframeManagedZip {
            root: "package".into(),
            entry_assembly: "Core.dll".into(),
            entry_type: "Fixture.Core".into(),
        },
    }
}
struct Archive {
    sha256: String,
    size_bytes: u64,
    layout: Layout,
}
fn extract(
    archive: &Path,
    content: &Path,
    expected: &Archive,
    cancel: &Cancel,
) -> Result<Prepared> {
    extract_checked(
        archive,
        content,
        &expected.sha256,
        expected.size_bytes,
        cancel,
        |files| layout(files, &expected.layout),
    )
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
fn operation(artifact: &Archive) -> Operation {
    Operation {
        id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        release_id: "local-import".into(),
        hash: artifact.sha256.clone(),
        kind: Kind::Package,
        receipt_id: None,
        status: Status::Preparing,
        message: "Preparing".into(),
        received_bytes: 0,
        total_bytes: artifact.size_bytes,
    }
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
fn cancellation_preserves_cleanup_failures_in_operation_history() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(root.path()).unwrap();
    let artifact = artifact(b"fixture");
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
            entry: None,
            registry_reference: None,
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

#[test]
fn obsolete_catalog_prepare_action_is_rejected() {
    let value = serde_json::json!({"kind":"prepare", "requestId":Uuid::new_v4().to_string(), "releaseId":"fixture.core.1"});
    assert!(serde_json::from_value::<Action>(value).is_err());
}
