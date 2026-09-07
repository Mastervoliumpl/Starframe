use serde_json::Value;
use starframe::runtime_contract;
use std::path::Path;

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
