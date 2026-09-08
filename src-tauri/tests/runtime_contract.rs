use serde_json::Value;
use starframe::runtime_contract;
use std::path::Path;

#[test]
fn shared_activation_boundaries() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../contracts/fixtures/activation-boundaries.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let dimension = case["dimension"].as_str().unwrap();
        let count = case["count"].as_u64().unwrap() as usize;
        let mod_count = match dimension {
            "inventory" => 0,
            "mods" => count,
            "files" => 1,
            _ => count.div_ceil(1024),
        };
        let inventory_count = if dimension == "inventory" {
            count
        } else {
            mod_count
        };
        let inventory: Vec<_> = (0..inventory_count).map(|i| serde_json::json!({"modId":format!("fixture.m{i}"),"name":"Fixture","version":"1"})).collect();
        let mods: Vec<_> = (0..mod_count).map(|i| {
            let files = match dimension { "files" => count, "totalFiles" => (count - i * 1024).min(1024), _ => 1 };
            serde_json::json!({"modId":format!("fixture.m{i}"),"root":format!("mods/m{i}"),"source":{"kind":"catalog","releaseId":format!("fixture.m{i}.1")},"requires":[],"entryAssembly":null,"entryType":null,"files":(0..files).map(|f| serde_json::json!({"path":format!("LJ/lua/f{f}.lua"),"sha256":"ab".repeat(32)})).collect::<Vec<_>>()})
        }).collect();
        let activation = serde_json::json!({"schemaVersion":3,"runtimeContractVersion":1,"integrationId":"starframe.bepinex","deploymentRevision":"1","installedMods":inventory,"omittedDisabledMods":0,"mods":mods});
        let result =
            runtime_contract::read(&serde_json::to_vec(&activation).unwrap(), "activation");
        assert_eq!(
            result.is_ok(),
            case["valid"].as_bool().unwrap(),
            "{case}: {result:?}"
        );
    }
}

#[test]
fn shared_runtime_fixtures() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../contracts/fixtures");
    let cases: Value =
        serde_json::from_slice(&std::fs::read(directory.join("cases.json")).unwrap()).unwrap();
    for case in cases.as_array().unwrap() {
        let file = case["file"].as_str().unwrap();
        let bytes = std::fs::read(directory.join(file)).unwrap();
        let result = runtime_contract::read(&bytes, case["kind"].as_str().unwrap());
        assert_eq!(
            result.is_ok(),
            case["valid"].as_bool().unwrap(),
            "{file}: {result:?}"
        );
    }
    let activation = runtime_contract::read(
        &std::fs::read(directory.join("activation-local.json")).unwrap(),
        "activation",
    )
    .unwrap();
    let expected: Value =
        serde_json::from_slice(&std::fs::read(directory.join("canonical-inventory.json")).unwrap())
            .unwrap();
    assert_eq!(
        runtime_contract::content_id(&activation["mods"][0]["files"]).unwrap(),
        expected["contentId"].as_str().unwrap()
    );
    assert!(
        runtime_contract::read(
            &vec![0; runtime_contract::MAX_DOCUMENT_BYTES + 1],
            "activation"
        )
        .is_err()
    );
}

#[test]
fn reports_require_current_process_revision_and_complete_ordered_results() {
    let activation = runtime_contract::read(
        include_bytes!("../../contracts/fixtures/activation.json"),
        "activation",
    )
    .unwrap();
    let report = runtime_contract::read(
        include_bytes!("../../contracts/fixtures/report.json"),
        "report",
    )
    .unwrap();
    assert!(runtime_contract::report_matches(
        &activation,
        &report,
        456,
        "134331234567890000"
    ));
    assert!(!runtime_contract::report_matches(
        &activation,
        &report,
        457,
        "134331234567890000"
    ));
    assert!(!runtime_contract::report_matches(
        &activation,
        &report,
        456,
        "134331234567890001"
    ));
    let mut changed = activation.clone();
    changed["deploymentRevision"] = "13".into();
    assert!(!runtime_contract::report_matches(
        &changed,
        &report,
        456,
        "134331234567890000"
    ));
    let mut missing = report.clone();
    missing["mods"] = serde_json::json!([]);
    assert!(!runtime_contract::report_matches(
        &activation,
        &missing,
        456,
        "134331234567890000"
    ));
    let mut foreign = report.clone();
    foreign["mods"][0]["modId"] = "different.mod".into();
    assert!(!runtime_contract::report_matches(
        &activation,
        &foreign,
        456,
        "134331234567890000"
    ));
}
