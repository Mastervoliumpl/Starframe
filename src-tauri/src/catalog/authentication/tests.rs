use super::*;
use aws_lc_rs::{rand::SystemRandom, signature::Ed25519KeyPair};
use serde_json::json;
use std::{collections::HashMap, fs, num::NonZeroU64, path::PathBuf};
use tough::{
    editor::{RepositoryEditor, signed::SignedRole},
    key_source::KeySource,
    schema::{KeyHolder, Root},
    sign::{Sign, parse_keypair},
};

struct TestKey(Vec<u8>);
impl std::fmt::Debug for TestKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ephemeral test key")
    }
}
#[async_trait]
impl KeySource for TestKey {
    async fn as_sign(&self) -> Result<Box<dyn Sign>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(parse_keypair(&self.0)?))
    }
    async fn write(
        &self,
        _: &str,
        _: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("Test keys stay in memory".into())
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    metadata: PathBuf,
    targets: PathBuf,
    store: PathBuf,
    root_path: PathBuf,
    keys: Vec<Box<dyn KeySource>>,
    root: Root,
    trusted: Vec<u8>,
}
impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let metadata = directory.path().join("metadata");
        let targets = directory.path().join("targets");
        let store = directory.path().join("store");
        for path in [&metadata, &targets, &store] {
            fs::create_dir(path).unwrap();
        }
        let keys = Self::keys();
        let root = Self::root(&keys, 1).await;
        let signed = SignedRole::new(
            root.clone(),
            &KeyHolder::Root(root.clone()),
            &keys,
            &SystemRandom::new(),
        )
        .await
        .unwrap();
        let trusted = signed.buffer().clone();
        let root_path = directory.path().join("trusted.json");
        fs::write(&root_path, &trusted).unwrap();
        Self {
            _root: directory,
            metadata,
            targets,
            store,
            root_path,
            keys,
            root,
            trusted,
        }
    }
    fn keys() -> Vec<Box<dyn KeySource>> {
        (0..4)
            .map(|_| {
                Box::new(TestKey(
                    Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                        .unwrap()
                        .as_ref()
                        .to_vec(),
                )) as Box<dyn KeySource>
            })
            .collect()
    }
    async fn root(keys: &[Box<dyn KeySource>], version: u64) -> Root {
        let mut public = HashMap::new();
        let mut roles = HashMap::new();
        for (role, key) in ["root", "targets", "snapshot", "timestamp"]
            .into_iter()
            .zip(keys)
        {
            let key = key.as_sign().await.unwrap().tuf_key();
            let id = key.key_id().unwrap();
            let id = serde_json::to_value(id)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned();
            public.insert(id.clone(), key);
            roles.insert(role, json!({"threshold":1,"keyids":[id]}));
        }
        serde_json::from_value(json!({"_type":"root","spec_version":"1.0.0","version":version,"expires":"2100-01-01T00:00:00Z","consistent_snapshot":false,"keys":public,"roles":roles})).unwrap()
    }
    async fn publish(&self, version: u64, expires: &str) {
        self.publish_advisories(
            version,
            expires,
            &json!({"schemaVersion":1,"revision":version.to_string(),"advisories":[]}),
        )
        .await;
    }
    async fn publish_advisories(
        &self,
        version: u64,
        expires: &str,
        advisories: &serde_json::Value,
    ) {
        self.publish_contents(
            version,
            expires,
            &json!({"schemaVersion":2,"catalogRevision":version.to_string(),"mods":[]}),
            advisories,
        )
        .await;
    }
    async fn publish_contents(
        &self,
        version: u64,
        expires: &str,
        catalog: &serde_json::Value,
        advisories: &serde_json::Value,
    ) {
        fs::write(
            self.targets.join("advisories.json"),
            serde_json::to_vec(advisories).unwrap(),
        )
        .unwrap();
        fs::write(
            self.targets.join("catalog.json"),
            serde_json::to_vec(catalog).unwrap(),
        )
        .unwrap();
        let mut editor = RepositoryEditor::new(&self.root_path).await.unwrap();
        editor
            .targets_version(NonZeroU64::new(version).unwrap())
            .unwrap()
            .targets_expires(expires.parse().unwrap())
            .unwrap()
            .snapshot_version(NonZeroU64::new(version).unwrap())
            .snapshot_expires(expires.parse().unwrap())
            .timestamp_version(NonZeroU64::new(version).unwrap())
            .timestamp_expires(expires.parse().unwrap())
            .add_target_path(self.targets.join("catalog.json"))
            .await
            .unwrap()
            .add_target_path(self.targets.join("advisories.json"))
            .await
            .unwrap();
        editor
            .sign(&self.keys)
            .await
            .unwrap()
            .write(&self.metadata)
            .await
            .unwrap();
    }
    async fn read(&self) -> Result<VerifiedCatalog, String> {
        load(
            &self.trusted,
            &self.store,
            Url::from_directory_path(&self.metadata).unwrap(),
            Url::from_directory_path(&self.targets).unwrap(),
            tough::FilesystemTransport,
        )
        .await
    }
    async fn rotate(&mut self, cross_sign: bool) {
        let keys = Self::keys();
        let root = Self::root(&keys, 2).await;
        let mut signed = SignedRole::new(
            root.clone(),
            &KeyHolder::Root(root.clone()),
            &keys,
            &SystemRandom::new(),
        )
        .await
        .unwrap();
        if cross_sign {
            let old = SignedRole::new(
                root.clone(),
                &KeyHolder::Root(self.root.clone()),
                &self.keys,
                &SystemRandom::new(),
            )
            .await
            .unwrap();
            signed = signed
                .add_old_signatures(old.signed().signatures.clone())
                .unwrap();
        }
        fs::write(self.metadata.join("2.root.json"), signed.buffer()).unwrap();
        fs::write(&self.root_path, signed.buffer()).unwrap();
        self.keys = keys;
        self.root = root;
    }
}

pub(crate) async fn verified(
    version: u64,
    advisories: &crate::catalog::advisories::Advisories,
) -> VerifiedCatalog {
    let fixture = Fixture::new().await;
    fixture
        .publish_advisories(
            version,
            "2100-01-01T00:00:00Z",
            &serde_json::to_value(advisories).unwrap(),
        )
        .await;
    fixture.read().await.unwrap()
}

pub(crate) async fn verified_catalog(
    catalog: &Catalog,
    advisories: &crate::catalog::advisories::Advisories,
) -> VerifiedCatalog {
    let fixture = Fixture::new().await;
    fixture
        .publish_contents(
            1,
            "2100-01-01T00:00:00Z",
            &serde_json::to_value(catalog).unwrap(),
            &serde_json::to_value(advisories).unwrap(),
        )
        .await;
    fixture.read().await.unwrap()
}

#[tokio::test]
async fn verified_bytes_expiry_and_rollback() {
    let fixture = Fixture::new().await;
    fixture.publish(2, "2100-01-01T00:00:00Z").await;
    let verified = fixture.read().await.unwrap();
    assert_eq!(verified.catalog().catalog_revision, "2");
    assert_eq!(verified.expires(), 4_102_444_800);
    assert_eq!(verified.advisories().revision, "2");
    fs::write(fixture.targets.join("catalog.json"), b"tampered").unwrap();
    assert!(fixture.read().await.is_err());
    fixture.publish(1, "2100-01-01T00:00:00Z").await;
    let error = fixture.read().await.unwrap_err();
    assert!(error.contains("previously fetched version 2"), "{error}");
    fixture.publish(3, "2000-01-01T00:00:00Z").await;
    assert!(fixture.read().await.unwrap_err().contains("expired"));
    fixture.publish(4, "2100-01-01T00:00:00Z").await;
    assert_eq!(
        fixture.read().await.unwrap().catalog().catalog_revision,
        "4"
    );
    fs::write(
        fixture.targets.join("advisories.json"),
        b"tampered advisory",
    )
    .unwrap();
    assert!(fixture.read().await.is_err());
}

#[tokio::test]
async fn signatures_and_retained_root_rotation() {
    let mut fixture = Fixture::new().await;
    fixture.publish(1, "2100-01-01T00:00:00Z").await;
    fixture.read().await.unwrap();
    let mut timestamp: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.metadata.join("timestamp.json")).unwrap())
            .unwrap();
    timestamp["signed"]["version"] = json!(2);
    fs::write(
        fixture.metadata.join("timestamp.json"),
        serde_json::to_vec(&timestamp).unwrap(),
    )
    .unwrap();
    assert!(fixture.read().await.is_err());
    fixture.rotate(true).await;
    fixture.publish(2, "2000-01-01T00:00:00Z").await;
    assert!(fixture.read().await.is_err());
    fs::remove_file(fixture.metadata.join("2.root.json")).unwrap();
    fixture.publish(3, "2100-01-01T00:00:00Z").await;
    assert_eq!(
        fixture.read().await.unwrap().catalog().catalog_revision,
        "3"
    );
    let mut wrong = Fixture::new().await;
    wrong.rotate(false).await;
    wrong.publish(2, "2100-01-01T00:00:00Z").await;
    assert!(wrong.read().await.is_err());
}

#[tokio::test]
async fn transfer_status_size_and_request_bounds() {
    use std::io::{Read, Write};
    for (status, declared, body, accepted) in [
        (200, 2, "ok", true),
        (302, 0, "", false),
        (404, 0, "", false),
        (503, 0, "", false),
        (200, MAX_BYTES + 1, "", false),
        (200, 5, "no", false),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).unwrap() > 0);
            let _ = write!(
                socket,
                "HTTP/1.1 {status} Test\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n{body}"
            );
        });
        let transport = Http {
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            bases: [base.clone(), base.clone()],
            requests: Arc::new(AtomicUsize::new(0)),
        };
        let result = transport.fetch(base.join("timestamp.json").unwrap()).await;
        assert_eq!(result.is_ok(), accepted, "status {status}, size {declared}");
        if status == 404 {
            assert!(matches!(
                result.err().unwrap().kind(),
                TransportErrorKind::FileNotFound
            ));
        }
        server.join().unwrap();
        transport.requests.store(40, Ordering::Relaxed);
        assert!(transport.fetch(base.clone()).await.is_err());
        assert!(
            transport
                .fetch(Url::parse("https://example.invalid/").unwrap())
                .await
                .is_err()
        );
    }
    let store = tempfile::tempdir().unwrap();
    assert!(
        fetch(
            b"{}",
            store.path(),
            Url::parse("http://example.invalid/").unwrap(),
            Url::parse("https://example.invalid/").unwrap()
        )
        .await
        .is_err()
    );
}
