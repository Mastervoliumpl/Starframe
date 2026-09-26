use super::*;
use serde_json::json;

fn fixture() -> (Vec<crate::local_import::LocalSource>, Vec<ModReference>) {
    let locals: Vec<crate::local_import::LocalSource> = ["a", "b", "c", "d"].iter().map(|id| serde_json::from_value(json!({
        "reference":{"modId":id,"hash":"a".repeat(64),"origin":"local_import","releaseId":null},
        "path":std::env::temp_dir().join(format!("ordering-{id}")),
        "manifest":{"schemaVersion":1,"modId":id,"name":id,"author":"Fixture","version":"1", "layout":{"kind":"starframe_managed_zip","root":"","entryAssembly":"Fixture.dll","entryType":"Fixture.Entry"}}
    })).unwrap()).collect();
    let references = locals
        .iter()
        .map(|source| source.reference.clone())
        .collect();
    (locals, references)
}

fn ids(resolution: &Resolution) -> Vec<&str> {
    resolution
        .effective
        .iter()
        .map(|r| r.mod_id.as_str())
        .collect()
}

#[test]
fn user_priority_is_the_tie_break_among_ready_mods() {
    let (mut locals, requested) = fixture();
    locals[0].manifest.requires = vec![requested[2].clone()];
    for _ in 0..10 {
        let order = resolve_locals(&locals, &requested).unwrap();
        assert_eq!(ids(&order), ["b", "c", "a", "d"]);
        assert_eq!(order.adjustments[0].message, "c must load before a.");
    }
    let mut reversed = requested.clone();
    reversed.reverse();
    assert_eq!(
        ids(&resolve_locals(&locals, &reversed).unwrap()),
        ["d", "c", "b", "a"]
    );
    assert_eq!(
        ids(&resolve_locals(&locals, &[]).unwrap()),
        Vec::<&str>::new()
    );
}

#[test]
fn mandatory_constraints_win_and_optional_cycles_do_not_block() {
    let (mut locals, requested) = fixture();
    locals[0].manifest.load_after = vec!["c".into()];
    locals[1].manifest.load_before = vec!["c".into()];
    locals[0].manifest.prefer_before = vec!["b".into(), "absent".into()];
    locals[3].manifest.prefer_before = vec!["b".into()];
    let order = resolve_locals(&locals, &requested).unwrap();
    assert_eq!(ids(&order), ["b", "c", "a", "d"]);
    assert!(
        order
            .adjustments
            .iter()
            .any(|s| s.message.contains("a prefers to load before b"))
    );
    locals[2].manifest.load_before = vec!["b".into()];
    let error = resolve_locals(&locals, &requested).unwrap_err();
    assert!(error.contains("b -> c -> b"), "{error}");
    assert!(!error.contains("a ->"));
}

#[test]
fn missing_exact_requirements_and_invalid_identities_fail() {
    let (mut locals, requested) = fixture();
    locals[0].manifest.requires = vec![requested[2].clone()];
    assert!(
        resolve_locals(&locals, &requested[..2])
            .unwrap_err()
            .contains("a requires local build c")
    );
    let mut invalid = requested.clone();
    invalid[0].hash = "b".repeat(64);
    assert!(
        resolve_locals(&locals, &invalid)
            .unwrap_err()
            .contains("Exact release metadata for a")
    );
    assert!(
        resolve_locals(&locals, &[requested[0].clone(), requested[0].clone()])
            .unwrap_err()
            .contains("Only one version")
    );
}
