use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

const INPUT_LIMIT: usize = 65_536;

fn next(state: &mut u64) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*state >> 32) as usize
}

fn mutate(seed: &[u8], state: &mut u64) -> Vec<u8> {
    let mut bytes = seed.to_vec();
    let tokens: &[&[u8]] = &[
        b"../",
        b"C:/",
        b"\\\\?\\",
        b"NUL",
        b":stream",
        b"\0",
        b"\xff",
        b"18446744073709551616",
        b"{\"schemaVersion\":1,\"schemaVersion\":2}",
        b"PK\x01\x02",
        b"PK\x05\x06",
        b"[[[[[[[[[[[[[[[[",
        b"/./",
        b". ",
    ];
    for _ in 0..1 + next(state) % 8 {
        let at = next(state) % (bytes.len() + 1);
        match next(state) % 4 {
            0 if at < bytes.len() => bytes[at] ^= (next(state) as u8) | 1,
            1 => bytes.truncate(at),
            2 => {
                let token = tokens[next(state) % tokens.len()];
                bytes.splice(at..at, token.iter().copied());
            }
            _ => bytes.extend_from_slice(&[next(state) as u8; 16]),
        }
        bytes.truncate(INPUT_LIMIT);
    }
    bytes
}

fn corpus() -> Vec<(String, Vec<u8>)> {
    let mut corpus = vec![(
        "catalog".into(),
        serde_json::to_vec(&catalog(artifact(b"inert archive"))).unwrap(),
    )];
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../contracts/fixtures");
    let mut paths: Vec<_> = fs::read_dir(fixtures)
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy();
        for kind in ["activation", "capabilities", "report"] {
            if name.starts_with(kind) && path.extension().is_some_and(|e| e == "json") {
                let bytes = fs::read(&path).unwrap();
                if bytes.len() <= INPUT_LIMIT {
                    corpus.push((kind.into(), bytes));
                }
            }
        }
    }
    for path in [
        "package/Core.dll",
        "../sentinel",
        "package/NUL",
        "package/a:stream",
        "package/core.dll",
    ] {
        let bytes = if path == "package/Core.dll" {
            zip(&[(path, b"inert fixture")])
        } else {
            zip(&[("package/Core.dll", b"inert fixture"), (path, b"untrusted")])
        };
        corpus.push(("zip".into(), bytes));
        corpus.push(("path".into(), path.as_bytes().to_vec()));
    }
    corpus
}

fn probe(kind: &str, bytes: &[u8]) -> bool {
    match kind {
        "catalog" => match Catalog::read(bytes) {
            Ok(value) => {
                value.validate().unwrap();
                Catalog::read(&serde_json::to_vec(&value).unwrap()).unwrap();
                true
            }
            Err(_) => false,
        },
        "path" => {
            let Ok(path) = std::str::from_utf8(bytes) else {
                return false;
            };
            match crate::runtime_contract::relative_path(path) {
                Ok(normalized) => {
                    assert!(!Path::new(&normalized).is_absolute());
                    assert!(!normalized.contains(['\\', ':', '\0']));
                    assert!(normalized.split('/').all(|p| !matches!(p, "" | "." | "..")));
                    true
                }
                Err(_) => false,
            }
        }
        "zip" => {
            let root = tempfile::tempdir().unwrap();
            let archive = root.path().join("input.zip");
            let content = root.path().join("content");
            fs::create_dir(&content).unwrap();
            fs::write(root.path().join("sentinel"), b"untouched").unwrap();
            fs::write(&archive, bytes).unwrap();
            let outcome = extract(&archive, &content, &artifact(bytes), &Cancel::default());
            assert_eq!(
                fs::read(root.path().join("sentinel")).unwrap(),
                b"untouched"
            );
            if let Ok(prepared) = &outcome {
                supported_files(&prepared.files).unwrap();
                for file in &prepared.files {
                    let path = content.join(&file.path).canonicalize().unwrap();
                    assert!(path.starts_with(content.canonicalize().unwrap()));
                    let actual = fs::read(path).unwrap();
                    assert_eq!(actual.len() as u64, file.size_bytes);
                    assert_eq!(format!("{:x}", Sha256::digest(actual)), file.sha256);
                }
            }
            outcome.is_ok()
        }
        kind => match crate::runtime_contract::read(bytes, kind) {
            Ok(value) => {
                assert_eq!(
                    crate::runtime_contract::read(&serde_json::to_vec(&value).unwrap(), kind)
                        .unwrap(),
                    value
                );
                true
            }
            Err(_) => false,
        },
    }
}

#[test]
fn corpus_and_mutation_smoke() {
    let corpus = corpus();
    let mut state = 47;
    for (kind, seed) in &corpus {
        probe(kind, seed);
        probe(kind, &mutate(seed, &mut state));
    }
}

#[test]
#[ignore = "bounded campaign; run scripts/run_fuzz.py outside ordinary builds"]
fn bounded_campaign() {
    let seed: u64 = std::env::var("STARFRAME_FUZZ_SEED")
        .unwrap_or_else(|_| "47".into())
        .parse()
        .unwrap();
    let seconds: u64 = std::env::var("STARFRAME_FUZZ_SECONDS")
        .unwrap_or_else(|_| "60".into())
        .parse()
        .unwrap();
    assert!((1..=900).contains(&seconds));
    let evidence =
        PathBuf::from(std::env::var("STARFRAME_FUZZ_OUTPUT").expect("use the campaign runner"));
    let corpus = corpus();
    let started = Instant::now();
    let mut state = seed;
    let mut counts = BTreeMap::<String, (u64, u64)>::new();
    let mut iteration = 0u64;
    while started.elapsed() < Duration::from_secs(seconds) && iteration < 1_000_000 {
        let (kind, input) = &corpus[next(&mut state) % corpus.len()];
        let input = if iteration.is_multiple_of(16) {
            input.clone()
        } else {
            mutate(input, &mut state)
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| probe(kind, &input)));
        match outcome {
            Ok(accepted) => {
                let count = counts.entry(kind.clone()).or_default();
                count.0 += 1;
                count.1 += u64::from(accepted);
            }
            Err(error) => {
                fs::write(evidence.join("failure.bin"), &input).unwrap();
                fs::write(
                    evidence.join("failure.json"),
                    serde_json::to_vec_pretty(
                        &serde_json::json!({"seed":seed,"iteration":iteration,"target":kind}),
                    )
                    .unwrap(),
                )
                .unwrap();
                std::panic::resume_unwind(error);
            }
        }
        iteration += 1;
    }
    fs::write(
        evidence.join("campaign.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "seed": seed, "iterations": iteration, "seconds": started.elapsed().as_secs_f64(),
            "inputLimit": INPUT_LIMIT, "targets": counts, "coverageGuided": false,
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[cfg(windows)]
#[ignore = "Windows path replacement stress; run scripts/run_fuzz.py"]
fn directory_replacement_race() {
    let root = tempfile::tempdir().unwrap();
    let candidate = root.path().join("candidate");
    let parked = root.path().join("parked");
    let link = root.path().join("link");
    let external = root.path().join("external");
    fs::create_dir(&candidate).unwrap();
    fs::create_dir(&external).unwrap();
    fs::write(external.join("sentinel"), b"untouched").unwrap();
    assert!(
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&external)
            .output()
            .unwrap()
            .status
            .success()
    );
    let stop = AtomicBool::new(false);
    let mut escaped_pin = false;
    let mut attempts = 0;
    let mut opened = 0;
    std::thread::scope(|scope| {
        let racer = scope.spawn(|| {
            while !stop.load(Ordering::Relaxed) {
                if fs::rename(&candidate, &parked).is_err() {
                    std::thread::yield_now();
                    continue;
                }
                if fs::rename(&link, &candidate).is_ok() {
                    std::thread::yield_now();
                    while fs::rename(&candidate, &link).is_err() {
                        std::thread::yield_now();
                    }
                }
                fs::rename(&parked, &candidate).unwrap();
            }
        });
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(3) {
            attempts += 1;
            if let Ok(handle) = crate::filesystem::pin(&candidate) {
                opened += 1;
                let moved = root.path().join("moved");
                if fs::rename(&candidate, &moved).is_ok() {
                    fs::rename(&moved, &candidate).unwrap();
                    escaped_pin = true;
                }
                drop(handle);
                if escaped_pin {
                    break;
                }
            }
        }
        stop.store(true, Ordering::Relaxed);
        racer.join().unwrap();
    });
    fs::remove_dir(&link).unwrap();
    assert_eq!(fs::read(external.join("sentinel")).unwrap(), b"untouched");
    assert!(
        attempts > 0 && opened > 0,
        "race did not exercise a successful directory pin"
    );
    assert!(
        !escaped_pin,
        "a directory name changed while its pin was held"
    );
    println!("Windows replacement race: {attempts} attempts, {opened} directory pins");
}
