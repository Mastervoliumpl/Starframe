use super::*;
use crate::registry::{ModId, ReleaseId, trust};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::signature::Ed25519KeyPair;
use serde_json::Value;
use std::io::Cursor;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zip::{ZipWriter, write::SimpleFileOptions};

fn fixtures() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/registry-installation-v1.json"
    ))
    .unwrap()
}

fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

fn manifest(bytes: &[u8], plan: Option<Value>) -> VerifiedRelease {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/registry-keys-v1.json"
    ))
    .unwrap();
    let now = OffsetDateTime::parse("2026-09-24T12:00:00Z", &Rfc3339).unwrap();
    let root: [u8; 32] = STANDARD
        .decode(fixture["rootPublicKey"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let keys = trust::verify_keys(
        &serde_json::to_vec(&fixture["envelope"]).unwrap(),
        &root,
        now,
    )
    .unwrap();
    let security = trust::verify_security(
        &serde_json::to_vec(&fixture["securityEnvelope"]).unwrap(),
        &keys,
        now,
    )
    .unwrap();
    let mut envelope = fixture["releaseEnvelope"].clone();
    envelope["signed"]["release"]["artifact"]["sha256"] =
        format!("{:x}", Sha256::digest(bytes)).into();
    envelope["signed"]["release"]["artifact"]["bytes"] = (bytes.len() as u64).into();
    if let Some(plan) = plan {
        envelope["signed"]["schemaVersion"] = 2.into();
        envelope["signed"]["release"]["metadata"]["installation"] = plan;
    }
    let online = Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
    envelope["signatures"][0]["signature"] = STANDARD
        .encode(
            online
                .sign(&serde_json::to_vec(&envelope["signed"]).unwrap())
                .as_ref(),
        )
        .into();
    trust::verify_release(
        &serde_json::to_vec(&envelope).unwrap(),
        &keys,
        &security,
        ModId::try_from(1).unwrap(),
        ReleaseId(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()),
        now,
    )
    .unwrap()
}

fn managed_image() -> Vec<u8> {
    let mut bytes = vec![0u8; 1024];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[128..132].copy_from_slice(b"PE\0\0");
    for (at, value) in [
        (60, 128u32),
        (244, 16),
        (360, 8192),
        (364, 72),
        (388, 8192),
        (392, 512),
        (396, 512),
        (512, 72),
        (520, 8320),
        (524, 64),
    ] {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [(134, 1u16), (148, 224), (150, 0x2000), (152, 0x10b)] {
        bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    bytes[640..644].copy_from_slice(b"BSJB");
    bytes
}

fn cached(root: &Path, bytes: &[u8]) -> PathBuf {
    fs::create_dir_all(root.join("registry-archives")).unwrap();
    let path = root
        .join("registry-archives")
        .join(format!("{:x}.zip", Sha256::digest(bytes)));
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn four_declared_archives_prepare_and_reuse_without_a_catalog_or_code_execution() {
    let cases = fixtures();
    let image = managed_image();
    let packages = [
        zip(&[
            ("LJ/lua/Example/main.lua", b"return {}"),
            ("LJ/lua/Example/helper.lua", b"return {}"),
        ]),
        zip(&[
            ("Package/Example.dll", &image),
            ("Package/README.txt", b"fixture"),
        ]),
        zip(&[
            ("Maps/Example/Example.sanmap", b"inert map"),
            ("Maps/Example/Textures/height.png", b"inert image"),
        ]),
        zip(&[
            ("AI/Example/AIInit.lua", b"return {}"),
            ("AI/Example/formers/rush.lua", b"return {}"),
        ]),
    ];
    for (index, bytes) in packages.iter().enumerate() {
        let root = tempfile::tempdir().unwrap();
        let archive = cached(root.path(), bytes);
        let release = manifest(bytes, Some(cases["cases"]["valid"][index]["plan"].clone()));
        let prepared = prepare_registry_archive(
            root.path(),
            Uuid::new_v4(),
            &release,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(prepared.files.len(), 2);
        assert_eq!(fs::read(&archive).unwrap(), *bytes);
        assert_eq!(
            prepare_registry_archive(
                root.path(),
                Uuid::new_v4(),
                &release,
                Arc::new(AtomicBool::new(false))
            )
            .unwrap(),
            prepared
        );
        assert_eq!(
            fs::read_dir(root.path().join("package-staging"))
                .unwrap()
                .count(),
            0
        );
        let target = root
            .path()
            .join("artifacts")
            .join(&prepared.hash)
            .join(&prepared.files[0].path);
        fs::write(&target, b"user change").unwrap();
        assert!(
            prepare_registry_archive(
                root.path(),
                Uuid::new_v4(),
                &release,
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
        );
        assert_eq!(fs::read(target).unwrap(), b"user change");
    }
}

#[test]
fn contradictory_unsafe_and_nested_content_never_promotes() {
    let plan = fixtures()["cases"]["valid"][2]["plan"].clone();
    let packages = [
        zip(&[("Maps/Example/missing.sanmap", b"map")]),
        zip(&[
            ("Maps/Example/Example.sanmap", b"map"),
            ("Other/readme.txt", b"outside"),
        ]),
        zip(&[
            ("Maps/Example/Example.sanmap", b"map"),
            ("Maps/Example/plugin.dll", b"inert"),
        ]),
        zip(&[
            ("Maps/Example/Example.sanmap", b"map"),
            ("Maps/Example/../escape.txt", b"unsafe"),
        ]),
        zip(&[
            ("Maps/Example/Example.sanmap", b"map"),
            ("Maps/Example/data.txt", b"PK\x03\x04nested"),
        ]),
        zip(&[
            ("Maps/Example/Example.sanmap", b"map"),
            ("Maps/Example/data.txt", b"MZdisguised executable"),
        ]),
    ];
    for bytes in packages {
        let root = tempfile::tempdir().unwrap();
        cached(root.path(), &bytes);
        let release = manifest(&bytes, Some(plan.clone()));
        assert!(
            prepare_registry_archive(
                root.path(),
                Uuid::new_v4(),
                &release,
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
        );
        assert_eq!(
            fs::read_dir(root.path().join("artifacts")).unwrap().count(),
            0
        );
        assert_eq!(
            fs::read_dir(root.path().join("package-staging"))
                .unwrap()
                .count(),
            0
        );
    }
}

#[test]
fn missing_plan_changed_archive_invalid_dll_and_cancel_are_not_prepared() {
    let bytes = zip(&[("Package/Example.dll", b"not a DLL")]);
    let plan = fixtures()["cases"]["valid"][1]["plan"].clone();
    let root = tempfile::tempdir().unwrap();
    let path = cached(root.path(), &bytes);
    let old = manifest(&bytes, None);
    assert!(
        prepare_registry_archive(
            root.path(),
            Uuid::new_v4(),
            &old,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .contains("no approved installation")
    );
    let release = manifest(&bytes, Some(plan));
    assert!(
        prepare_registry_archive(
            root.path(),
            Uuid::new_v4(),
            &release,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .contains("managed image")
    );
    assert!(
        prepare_registry_archive(
            root.path(),
            Uuid::new_v4(),
            &release,
            Arc::new(AtomicBool::new(true))
        )
        .unwrap_err()
        .contains("cancelled")
    );
    fs::write(&path, b"changed archive").unwrap();
    assert!(
        prepare_registry_archive(
            root.path(),
            Uuid::new_v4(),
            &release,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .contains("SHA-256 and size")
    );
    assert_eq!(
        fs::read_dir(root.path().join("artifacts")).unwrap().count(),
        0
    );
}
