use super::*;
use serde_json::Value;

#[test]
fn dependency_intervals_match_the_website_reference_without_version_label_guessing() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/registry-dependencies-v1.json"
    ))
    .unwrap();
    assert_eq!(
        fixtures["websiteContractRevision"],
        crate::registry::installation::CONTRACT_REVISION
    );
    let source = ModId::try_from(1).unwrap();
    for case in fixtures["valid"].as_array().unwrap() {
        let dependency: Dependency = serde_json::from_value(case["dependency"].clone()).unwrap();
        assert!(dependency.valid(source));
        for candidate in case["candidates"].as_array().unwrap() {
            let mod_id = ModId::try_from(candidate["modId"].as_u64().unwrap()).unwrap();
            let release_id =
                ReleaseId(uuid::Uuid::parse_str(candidate["releaseId"].as_str().unwrap()).unwrap());
            let label = candidate["versionLabel"].as_str().unwrap();
            assert_eq!(
                dependency.matches(mod_id, release_id, label),
                candidate["matches"].as_bool().unwrap(),
                "{} {}",
                case["dependency"],
                label
            );
            assert!(!dependency.matches(source, release_id, label));
        }
    }
    for dependencies in fixtures["invalid"].as_array().unwrap() {
        assert!(
            serde_json::from_value::<Vec<Dependency>>(dependencies.clone())
                .ok()
                .is_none_or(|dependencies| !valid_dependencies(source, &dependencies))
        );
    }
    let exact = Dependency::Exact {
        mod_id: ModId::try_from(2).unwrap(),
        release_id: ReleaseId(uuid::Uuid::new_v4()),
    };
    assert!(!exact.matches(
        exact.mod_id(),
        ReleaseId(uuid::Uuid::new_v4()),
        "same version label"
    ));
}

#[test]
fn suggestions_prefer_publication_order_and_filter_unavailable_candidates() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/registry-installation-v1.json"
    ))
    .unwrap();
    let template: crate::registry::Release =
        serde_json::from_value(fixtures["releases"][0]["envelope"]["signed"]["release"].clone())
            .unwrap();
    let mut older = template.clone();
    older.mod_id = ModId::try_from(2).unwrap();
    older.release_id = ReleaseId(uuid::Uuid::new_v4());
    older.version_label = "1.9.0".into();
    older.publication_order = super::super::PublicationOrder::try_from(1).unwrap();
    let mut newer = older.clone();
    newer.release_id = ReleaseId(uuid::Uuid::new_v4());
    newer.version_label = "1.2.0".into();
    newer.publication_order = super::super::PublicationOrder::try_from(2).unwrap();
    let mut unavailable = newer.clone();
    unavailable.release_id = ReleaseId(uuid::Uuid::new_v4());
    unavailable.publication_order = super::super::PublicationOrder::try_from(3).unwrap();
    unavailable.availability = crate::registry::Availability::Pruned;
    let dependency = Dependency::Range {
        mod_id: older.mod_id,
        minimum: Some("1.0.0".into()),
        before: Some("2.0.0".into()),
        include_prerelease: false,
    };
    let suggested = dependency
        .suggest(&[older.clone(), newer.clone(), unavailable.clone()])
        .unwrap();
    assert_eq!(suggested.release_id, newer.release_id);
    assert_eq!(suggested.sha256, newer.artifact.sha256);
    let exact = Dependency::Exact {
        mod_id: older.mod_id,
        release_id: older.release_id,
    };
    assert_eq!(
        exact.suggest(&[older.clone(), newer]).unwrap().release_id,
        older.release_id
    );
    for availability in [
        crate::registry::Availability::Withdrawn,
        crate::registry::Availability::Hidden,
        crate::registry::Availability::Pruned,
        crate::registry::Availability::Blocked,
    ] {
        unavailable.availability = availability;
        assert!(dependency.suggest(&[unavailable.clone()]).is_none());
    }
    unavailable.availability = crate::registry::Availability::Available;
    unavailable.security.status = super::super::wire::SecurityStatus::Blocked;
    assert!(dependency.suggest(&[unavailable]).is_none());
}
