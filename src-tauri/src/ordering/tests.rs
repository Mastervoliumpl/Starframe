use super::*;
use crate::storage::Origin;
use serde_json::json;

fn fixture() -> (Catalog, Vec<ModReference>) {
    let mods: Vec<_> = ["a", "b", "c", "d"].iter().map(|id| json!({
        "id": id, "name": id, "author":"Fixture", "sourceUrl":"https://example.invalid",
        "releases":[{"id":format!("{id}.1"),"version":"1","withdrawn":false,"requires":[],"testedGameBuilds":[],
            "artifact":{"url":"https://example.invalid/mod.zip","sha256":"a".repeat(64),"sizeBytes":1,
            "layout":{"kind":"starframe_managed_zip","root":"","entryAssembly":"Fixture.dll","entryType":"Fixture.Entry"}}}]
    })).collect();
    let catalog = Catalog::read(
        &serde_json::to_vec(&json!({"schemaVersion":2,"catalogRevision":"1","mods":mods})).unwrap(),
    )
    .unwrap();
    let references = catalog
        .releases()
        .map(|(m, r)| ModReference {
            mod_id: m.id.clone(),
            release_id: Some(r.id.clone()),
            hash: r.artifact.sha256.clone(),
            origin: Origin::Catalog,
        })
        .collect();
    (catalog, references)
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
    let (mut catalog, requested) = fixture();
    catalog.mods[0].releases[0].requires = vec!["c.1".into()];
    catalog.validate().unwrap();
    for _ in 0..10 {
        let order = resolve(&catalog, &requested).unwrap();
        assert_eq!(ids(&order), ["b", "c", "a", "d"]);
        assert_eq!(order.adjustments[0].message, "c must load before a.");
    }
    let mut reversed = requested.clone();
    reversed.reverse();
    assert_eq!(
        ids(&resolve(&catalog, &reversed).unwrap()),
        ["d", "c", "b", "a"]
    );
    assert_eq!(ids(&resolve(&catalog, &[]).unwrap()), Vec::<&str>::new());
}

#[test]
fn mandatory_constraints_win_and_optional_cycles_do_not_block() {
    let (mut catalog, requested) = fixture();
    catalog.mods[0].releases[0].load_after = vec!["c".into()];
    catalog.mods[1].releases[0].load_before = vec!["c".into()];
    catalog.mods[0].releases[0].prefer_before = vec!["b".into(), "absent".into()];
    catalog.mods[3].releases[0].prefer_before = vec!["b".into()];
    catalog.validate().unwrap();
    let order = resolve(&catalog, &requested).unwrap();
    assert_eq!(ids(&order), ["b", "c", "a", "d"]);
    assert!(
        order
            .adjustments
            .iter()
            .any(|s| s.message.contains("a prefers to load before b"))
    );
    catalog.mods[2].releases[0].load_before = vec!["b".into()];
    let error = resolve(&catalog, &requested).unwrap_err();
    assert!(error.contains("b -> c -> b"), "{error}");
    assert!(!error.contains("a ->"));
}

#[test]
fn missing_exact_requirements_and_invalid_identities_fail() {
    let (mut catalog, requested) = fixture();
    catalog.mods[0].releases[0].requires = vec!["c.1".into()];
    assert!(
        resolve(&catalog, &requested[..2])
            .unwrap_err()
            .contains("a requires release c.1")
    );
    let mut invalid = requested.clone();
    invalid[0].hash = "b".repeat(64);
    assert!(
        resolve(&catalog, &invalid)
            .unwrap_err()
            .contains("Exact release metadata for a")
    );
    assert!(
        resolve(&catalog, &[requested[0].clone(), requested[0].clone()])
            .unwrap_err()
            .contains("Only one version")
    );
}

#[test]
fn ordering_metadata_requires_schema_two_and_cannot_change_release_identity() {
    let (mut catalog, _) = fixture();
    let previous = catalog.clone();
    catalog.mods[0].releases[0].load_before = vec!["b".into()];
    catalog.schema_version = 1;
    assert!(catalog.validate().unwrap_err().contains("schema 2"));
    catalog.schema_version = 2;
    catalog.catalog_revision = "2".into();
    assert!(
        catalog
            .accepts_after(&previous)
            .unwrap_err()
            .contains("identity changed")
    );
    catalog.mods[0].releases[0].load_before = vec!["a".into()];
    assert!(catalog.validate().unwrap_err().contains("self ordering"));
}
