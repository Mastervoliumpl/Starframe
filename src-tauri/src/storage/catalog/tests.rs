use super::*;
use crate::catalog::{
    advisories::{Advisory, AffectedArtifact, Finding, State},
    authentication::tests::verified,
};

fn findings() -> Advisories {
    Advisories {
        schema_version: 1,
        revision: "1".into(),
        advisories: vec![Advisory {
            id: "synthetic.finding".into(),
            title: "Synthetic finding".into(),
            affected: vec![AffectedArtifact {
                release_id: "fixture.1".into(),
                sha256: "a".repeat(64),
                payload_sha256: vec![],
            }],
            history: vec![Finding {
                recorded_at: 1,
                state: State::Confirmed,
                explanation: "Synthetic test evidence".into(),
                evidence: vec!["https://example.invalid/evidence".into()],
                recommended_action: "Do not activate the fixture".into(),
            }],
        }],
    }
}

#[tokio::test]
async fn migration_restart_and_expiry_retain_confirmed_findings() {
    let root = tempfile::tempdir().unwrap();
    let store = Storage::open(root.path()).unwrap();
    store
        .conn
        .execute_batch("DROP TABLE catalog_security; PRAGMA user_version=12;")
        .unwrap();
    drop(store);
    let mut store = Storage::open(root.path()).unwrap();
    assert!(store.catalog_security().unwrap().is_none());
    let value = verified(1, &findings()).await;
    let saved = store.save_verified_catalog(&value, 100).unwrap();
    let backup = store.backup().unwrap();
    drop(store);
    let store = Storage::open(root.path()).unwrap();
    let security = store.catalog_security().unwrap().unwrap();
    assert_eq!(store.catalog_cache().unwrap().unwrap(), saved);
    security.require_fresh(value.catalog(), 100).unwrap();
    security
        .require_fresh(value.catalog(), security.expires() - 1)
        .unwrap();
    assert!(
        security
            .require_fresh(value.catalog(), security.expires())
            .is_err()
    );
    assert!(security.require_fresh(value.catalog(), 99).is_err());
    assert_eq!(
        security.advisories().findings_for(&"a".repeat(64), &[])[0]
            .current()
            .state,
        State::Confirmed
    );
    let mut changed = value.catalog().clone();
    changed.catalog_revision = "2".into();
    assert!(security.require_fresh(&changed, 101).is_err());
    let restored = Storage::restore_into(&backup, &root.path().join("restored")).unwrap();
    assert_eq!(restored.catalog_security().unwrap().unwrap(), security);
    assert_eq!(restored.catalog_cache().unwrap().unwrap(), saved);
}

#[tokio::test]
async fn failed_security_write_rolls_back_both_targets_and_corrections_need_history() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    let first = verified(1, &findings()).await;
    let saved = store.save_verified_catalog(&first, 100).unwrap();
    let retained = store.catalog_security().unwrap().unwrap();
    let mut corrected = findings();
    corrected.revision = "2".into();
    let mut finding = corrected.advisories[0].current().clone();
    finding.recorded_at = 2;
    finding.state = State::Cleared;
    finding.explanation = "Synthetic correction".into();
    corrected.advisories[0].history.push(finding);
    let second = verified(2, &corrected).await;
    store.conn.execute_batch("CREATE TRIGGER fail_security BEFORE UPDATE ON catalog_security BEGIN SELECT RAISE(ABORT, 'fixture write failure'); END;").unwrap();
    assert!(store.save_verified_catalog(&second, 101).is_err());
    assert_eq!(store.catalog_cache().unwrap().unwrap(), saved);
    assert_eq!(store.catalog_security().unwrap().unwrap(), retained);
    store
        .conn
        .execute_batch("DROP TRIGGER fail_security;")
        .unwrap();
    let mut removed = corrected.clone();
    removed.advisories.clear();
    assert!(
        store
            .save_verified_catalog(&verified(2, &removed).await, 101)
            .is_err()
    );
    let mut rewritten = corrected.clone();
    rewritten.advisories[0].history.remove(0);
    assert!(
        store
            .save_verified_catalog(&verified(2, &rewritten).await, 101)
            .is_err()
    );
    assert!(store.save_verified_catalog(&second, 99).is_err());
    let updated = store.save_verified_catalog(&second, 101).unwrap();
    assert!(
        store
            .catalog_security()
            .unwrap()
            .unwrap()
            .advisories()
            .findings_for(&"a".repeat(64), &[])
            .is_empty()
    );
    assert!(store.save_verified_catalog(&first, 102).is_err());
    assert_eq!(store.catalog_cache().unwrap().unwrap(), updated);
}

#[tokio::test]
async fn unsigned_cache_cannot_replace_authenticated_content_or_renew_expiry() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Storage::open(root.path()).unwrap();
    let first = verified(1, &findings()).await;
    let mut cache = store.save_verified_catalog(&first, 100).unwrap();
    let retained = store.catalog_security().unwrap().unwrap();
    cache.last_checked = Some(101);
    cache.last_success = Some(101);
    cache.error = Some("Refresh failed".into());
    store.save_catalog_cache(&cache).unwrap();
    assert_eq!(store.catalog_security().unwrap().unwrap(), retained);
    cache.catalog.as_mut().unwrap().catalog_revision = "2".into();
    assert!(store.save_catalog_cache(&cache).is_err());
    assert!(store.save_catalog_cache(&Cache::default()).is_err());
    assert_eq!(
        store.catalog_cache().unwrap().unwrap().catalog.unwrap(),
        *first.catalog()
    );
}
