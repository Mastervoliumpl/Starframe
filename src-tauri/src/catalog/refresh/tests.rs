use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;

fn fixture() -> Value {
    json!({"schemaVersion":1,"catalogRevision":"1","mods":[{
        "id":"fixture.core","name":"Fixture core","author":"Test fixture",
        "sourceUrl":"https://example.invalid/source","releases":[{
            "id":"fixture.core.1","version":"author label","withdrawn":false,
            "artifact":{"url":"https://example.invalid/core.zip","sha256":"a".repeat(64),"sizeBytes":12,
                "layout":{"kind":"starframe_managed_zip","root":"package","entryAssembly":"Core.dll","entryType":"Fixture.Core"}},
            "requires":[],"testedGameBuilds":["fixture-build"]
        }]
    }]})
}

fn read(value: &Value) -> Result<Catalog, String> {
    Catalog::read(&serde_json::to_vec(value).unwrap())
}

fn response(value: &Value) -> Response {
    Response {
        status: 200,
        etag: Some("\"fixture-1\"".into()),
        last_modified: None,
        retry_after: None,
        bytes: serde_json::to_vec(value).unwrap(),
    }
}

#[test]
fn maintenance_compatibility_and_withdrawal_are_independent_facts() {
    let mut value = fixture();
    value["mods"][0]["description"] = json!("A maintained record of an older release.");
    value["mods"][0]["unmaintained"] = json!(true);
    value["mods"][0]["releases"][0]["compatibilityProblems"] = json!([{"gameBuild":"new-build","note":"Verified fixture conflict.","sourceUrl":"https://example.invalid/report"}]);
    let catalog = read(&value).unwrap();
    assert!(catalog.downloadable("fixture.core.1").is_ok());
    value["mods"][0]["releases"][0]["withdrawn"] = json!(true);
    value["mods"][0]["releases"][0]["withdrawalReason"] = json!("Author removed the archive.");
    assert!(
        read(&value)
            .unwrap()
            .downloadable("fixture.core.1")
            .is_err()
    );
    value["mods"][0]["releases"][0]["compatibilityProblems"][0]["gameBuild"] =
        json!("fixture-build");
    assert!(read(&value).is_err());
    value["mods"][0]["releases"][0]["compatibilityProblems"][0]["gameBuild"] = json!("new-build");
    value["mods"][0]["releases"][0]["compatibilityProblems"][0]["sourceUrl"] =
        json!("javascript:alert(1)");
    assert!(read(&value).is_err());
}

#[test]
fn schema_rejects_ambiguous_identities_hashes_layouts_and_dependencies() {
    let good = fixture();
    assert!(read(&good).is_ok());
    for (pointer, value) in [
        ("/schemaVersion", json!(3)),
        ("/catalogRevision", json!(1)),
        ("/catalogRevision", json!("01")),
        ("/catalogRevision", json!("0")),
        ("/mods/0/id", json!("../test")),
        ("/mods/0/name", json!("\n")),
        ("/mods/0/sourceUrl", json!("http://example.invalid")),
        (
            "/mods/0/sourceUrl",
            json!("https://user:pass@example.invalid"),
        ),
        ("/mods/0/releases/0/artifact/sha256", json!("A".repeat(64))),
        ("/mods/0/releases/0/artifact/sizeBytes", json!(0)),
        ("/mods/0/releases/0/artifact/layout/kind", json!("unknown")),
        (
            "/mods/0/releases/0/artifact/layout/root",
            json!("../escape"),
        ),
        (
            "/mods/0/releases/0/artifact/layout/entryAssembly",
            json!("CON.dll"),
        ),
        (
            "/mods/0/releases/0/artifact/layout/entryAssembly",
            json!("C:/Other.dll"),
        ),
        (
            "/mods/0/releases/0/artifact/layout/entryType",
            json!("Fixture..Core"),
        ),
        ("/mods/0/releases/0/requires", json!(["missing"])),
        ("/mods/0/releases/0/requires", json!(["fixture.core.1"])),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(read(&bad).is_err(), "{pointer}: {bad}");
    }
    let mut bad = good.clone();
    bad["unexpected"] = json!(true);
    assert!(read(&bad).is_err());
    assert!(
        Catalog::read(br#"{"schemaVersion":1,"schemaVersion":1,"catalogRevision":"1","mods":[]}"#)
            .is_err()
    );
    assert!(Catalog::read(&vec![b' '; MAX_BYTES + 1]).is_err());
    bad = good.clone();
    bad["mods"]
        .as_array_mut()
        .unwrap()
        .push(good["mods"][0].clone());
    assert!(read(&bad).is_err());
    bad["mods"][1]["id"] = json!("fixture.other");
    assert!(read(&bad).is_err());
    bad["mods"][1]["releases"][0]["id"] = json!("fixture.other.1");
    bad["mods"][1]["releases"][0]["requires"] = json!(["fixture.core.1"]);
    assert!(read(&bad).is_ok());
    bad["mods"][0]["releases"][0]["requires"] = json!(["fixture.other.1"]);
    assert!(read(&bad).is_err());
}

#[test]
fn revisions_preserve_identity_and_withdrawal_blocks_new_downloads() {
    let original = read(&fixture()).unwrap();
    assert!(original.downloadable("fixture.core.1").is_ok());
    assert!(original.downloadable("unknown").is_err());
    for pointer in [
        "/mods/0/releases/0/artifact/sha256",
        "/mods/0/releases/0/version",
        "/mods/0/id",
        "/mods/0/releases/0/id",
    ] {
        let mut changed = fixture();
        changed["catalogRevision"] = json!("2");
        let old = changed.pointer(pointer).unwrap().as_str().unwrap();
        *changed.pointer_mut(pointer).unwrap() = json!(if old.len() == 64 {
            "b".repeat(64)
        } else {
            "new.identity".into()
        });
        assert!(
            read(&changed).unwrap().accepts_after(&original).is_err(),
            "{pointer}"
        );
    }
    let mut changed = fixture();
    changed["mods"][0]["name"] = json!("Changed name");
    assert!(read(&changed).unwrap().accepts_after(&original).is_err());
    changed["catalogRevision"] = json!("2");
    changed["mods"][0]["releases"][0]["withdrawn"] = json!(true);
    let withdrawn = read(&changed).unwrap();
    assert!(withdrawn.accepts_after(&original).is_ok());
    assert!(
        withdrawn
            .downloadable("fixture.core.1")
            .unwrap_err()
            .contains("withdrawn")
    );
    changed["catalogRevision"] = json!("3");
    changed["mods"][0]["releases"][0]["withdrawn"] = json!(false);
    assert!(read(&changed).unwrap().accepts_after(&withdrawn).is_err());
    changed["mods"] = json!([]);
    assert!(read(&changed).unwrap().accepts_after(&original).is_err());
}

#[test]
fn refresh_replacement_is_durable_and_does_not_change_installed_records_or_app_version() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(root.path()).unwrap();
    let reference = crate::storage::ModReference {
        mod_id: "fixture.core".into(),
        hash: "a".repeat(64),
        origin: crate::storage::Origin::Catalog,
        release_id: Some("fixture.core.1".into()),
    };
    storage
        .put_library_entry(
            &crate::storage::LibraryEntry {
                reference,
                name: "Installed fixture".into(),
                author: "Test".into(),
                version: "old version".into(),
            },
            0,
        )
        .unwrap();
    let before = storage.load().unwrap();
    let version = env!("CARGO_PKG_VERSION");
    let mut refresh = Refresh::load(&storage).unwrap();
    refresh.complete(&mut storage, Ok(response(&fixture())), 100);
    assert_eq!(refresh.next_check, 400);
    let retained = refresh.cache.catalog.clone();
    for malformed in [b"not json".to_vec(), br#"{"schemaVersion":99}"#.to_vec()] {
        let mut bad = response(&fixture());
        bad.bytes = malformed;
        refresh.complete(&mut storage, Ok(bad), 200);
        assert_eq!(refresh.cache.catalog, retained);
        assert!(refresh.cache.error.is_some());
        assert_eq!(refresh.cache.etag.as_deref(), Some("\"fixture-1\""));
    }
    let mut newer = fixture();
    newer["catalogRevision"] = json!("2");
    newer["mods"][0]["releases"][0]["withdrawn"] = json!(true);
    refresh.complete(&mut storage, Ok(response(&newer)), 300);
    assert!(refresh.cache.error.is_none());
    assert_eq!(storage.load().unwrap(), before);
    assert_eq!(env!("CARGO_PKG_VERSION"), version);
    drop(refresh);
    drop(storage);
    let mut storage = Storage::open(root.path()).unwrap();
    let mut restarted = Refresh::load(&storage).unwrap();
    assert_eq!(
        restarted.cache.catalog.as_ref().unwrap().catalog_revision,
        "2"
    );
    assert_eq!(restarted.next_check, 0);
    restarted.complete(&mut storage, Err("Offline".into()), 400);
    assert_eq!(restarted.cache.last_success, Some(300));
    assert_eq!(restarted.cache.last_checked, Some(400));
    assert_eq!(storage.catalog_cache().unwrap().unwrap(), restarted.cache);
    assert_eq!(storage.load().unwrap(), before);
    let mut old_cache = restarted.cache.clone();
    old_cache.catalog = retained;
    assert!(storage.save_catalog_cache(&old_cache).is_err());
    assert_eq!(storage.catalog_cache().unwrap().unwrap(), restarted.cache);
}

fn server(raw: Vec<u8>) -> (String, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/catalog.json", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        sender.send(String::from_utf8(request).unwrap()).unwrap();
        let _ = stream.write_all(&raw);
    });
    (url, receiver, thread)
}

#[tokio::test]
async fn http_fixtures_cover_validators_redirects_limits_and_interrupted_bodies() {
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let body = serde_json::to_string(&fixture()).unwrap();
    let (url, request, thread) = server(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"one\"\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes());
    let response = fetch(&client, &url, &Cache::default()).await.unwrap();
    assert_eq!(response.bytes, body.as_bytes());
    assert!(request.recv().unwrap().contains("accept: application/json"));
    thread.join().unwrap();
    let mut cache = Cache {
        catalog: Some(read(&fixture()).unwrap()),
        etag: Some("\"one\"".into()),
        ..Default::default()
    };
    for use_etag in [true, false] {
        if !use_etag {
            cache.etag = None;
            cache.last_modified = Some("Mon, 07 Sep 2026 00:00:00 GMT".into());
        }
        let (url, request, thread) =
            server(b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_vec());
        assert_eq!(fetch(&client, &url, &cache).await.unwrap().status, 304);
        let request = request.recv().unwrap();
        assert!(request.contains(if use_etag {
            "if-none-match: \"one\""
        } else {
            "if-modified-since: Mon, 07 Sep 2026 00:00:00 GMT"
        }));
        thread.join().unwrap();
    }
    for status in [301, 302, 404, 429, 503] {
        let (url, request, thread) = server(format!("HTTP/1.1 {status} Fixture\r\nLocation: http://127.0.0.1:1/no-follow\r\nRetry-After: 900\r\nContent-Length: 0\r\n\r\n").into_bytes());
        let response = fetch(&client, &url, &cache).await.unwrap();
        assert_eq!(response.status, status);
        assert_eq!(response.retry_after, Some(900));
        request.recv().unwrap();
        thread.join().unwrap();
    }
    for raw in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort".to_vec(),
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            MAX_BYTES + 1
        )
        .into_bytes(),
        [
            b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".as_slice(),
            &vec![b' '; MAX_BYTES + 1],
        ]
        .concat(),
    ] {
        let (url, request, thread) = server(raw);
        assert!(fetch(&client, &url, &Cache::default()).await.is_err());
        request.recv().unwrap();
        thread.join().unwrap();
    }
}

#[tokio::test]
async fn scheduler_waits_for_shell_prevents_overlap_and_aborts_on_shutdown() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::open(root.path()).unwrap();
    let mut refresh = Refresh::load(&storage).unwrap();
    refresh.tick(&mut storage, 0, false, false);
    assert!(!refresh.checking);
    refresh.tick(&mut storage, 0, true, false);
    assert!(refresh.checking);
    let abort = refresh.pending.as_ref().unwrap().0.abort_handle();
    refresh.tick(&mut storage, 86_400, true, false);
    assert_eq!(refresh.pending.as_ref().unwrap().0.id(), abort.id());
    refresh.tick(&mut storage, 86_401, true, true);
    tokio::task::yield_now().await;
    assert!(abort.is_finished());
    assert!(!refresh.checking);
    assert!(storage.catalog_cache().unwrap().is_none());
    refresh.complete(&mut storage, Ok(response(&fixture())), 100);
    refresh.tick(&mut storage, 399, true, false);
    assert!(!refresh.checking);
    let mut unchanged = response(&fixture());
    unchanged.status = 304;
    refresh.complete(&mut storage, Ok(unchanged), 400);
    assert_eq!(refresh.cache.last_success, Some(400));
    assert_eq!(refresh.next_check, 700);
    refresh.tick(&mut storage, 86_400, true, false);
    assert!(refresh.checking);
    refresh.tick(&mut storage, 86_400, true, true);
    tokio::task::yield_now().await;
    for i in 1..=9 {
        refresh.complete(&mut storage, Err("offline".into()), 700);
        assert_eq!(
            refresh.next_check,
            700 + (30u64 * (1 << (i - 1).min(6))).min(1800)
        );
    }
    let mut busy = response(&fixture());
    busy.status = 429;
    busy.retry_after = Some(3600);
    refresh.complete(&mut storage, Ok(busy), 1000);
    assert_eq!(refresh.next_check, 4600);
}
