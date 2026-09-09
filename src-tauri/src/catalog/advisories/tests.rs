use super::*;

fn fixture(state: State) -> Advisories {
    Advisories {
        schema_version: 1,
        revision: "1".into(),
        advisories: vec![Advisory {
            id: "sfsa.2026.1".into(),
            title: "Synthetic security finding".into(),
            affected: vec![AffectedArtifact {
                release_id: "fixture.1".into(),
                sha256: "a".repeat(64),
                payload_sha256: vec!["b".repeat(64)],
            }],
            history: vec![Finding {
                recorded_at: 1,
                state,
                explanation: "Synthetic evidence only".into(),
                evidence: vec!["https://example.invalid/evidence".into()],
                recommended_action: "Do not activate the affected fixture".into(),
            }],
        }],
    }
}

#[test]
fn exact_hashes_match_catalog_and_local_payloads() {
    let document = fixture(State::Confirmed);
    let parsed = Advisories::read(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(parsed, document);
    assert_eq!(document.findings_for(&"a".repeat(64), &[]).len(), 1);
    assert!(document.findings_for(&"c".repeat(64), &[]).is_empty());
    let local = PreparedFile {
        path: "renamed.dll".into(),
        sha256: "b".repeat(64),
        size_bytes: 1,
    };
    assert_eq!(
        document.findings_for(&"c".repeat(64), &[local])[0]
            .current()
            .state,
        State::Confirmed
    );
    let suspected = fixture(State::Suspected);
    assert_eq!(
        suspected.findings_for(&"a".repeat(64), &[])[0]
            .current()
            .state,
        State::Suspected
    );
}

#[test]
fn corrections_append_history_and_cannot_silently_remove_blocks() {
    let previous = fixture(State::Confirmed);
    let mut corrected = previous.clone();
    corrected.revision = "2".into();
    let mut finding = previous.advisories[0].current().clone();
    finding.recorded_at = 2;
    finding.state = State::Cleared;
    finding.explanation = "The fixture finding was incorrect".into();
    corrected.advisories[0].history.push(finding);
    corrected.accepts_after(&previous).unwrap();
    assert!(corrected.findings_for(&"a".repeat(64), &[]).is_empty());
    assert!(previous.accepts_after(&corrected).is_err());
    corrected.advisories[0].history.remove(0);
    assert!(corrected.accepts_after(&previous).is_err());
    corrected.advisories.clear();
    assert!(corrected.accepts_after(&previous).is_err());
    let mut changed = previous.clone();
    changed.advisories[0].history[0].state = State::Cleared;
    assert!(changed.accepts_after(&previous).is_err());
}

#[test]
fn malformed_and_ambiguous_findings_are_rejected() {
    let valid = fixture(State::Confirmed);
    let mut duplicate = valid.clone();
    duplicate.advisories.push(duplicate.advisories[0].clone());
    assert!(duplicate.validate().is_err());
    let mut bad_hash = valid.clone();
    bad_hash.advisories[0].affected[0].sha256 = "A".repeat(64);
    assert!(bad_hash.validate().is_err());
    let mut no_history = valid.clone();
    no_history.advisories[0].history.clear();
    assert!(no_history.validate().is_err());
    let mut unsafe_link = valid.clone();
    unsafe_link.advisories[0].history[0].evidence = vec!["file:///private".into()];
    assert!(unsafe_link.validate().is_err());
    assert!(Advisories::read(&vec![b' '; MAX_BYTES + 1]).is_err());
}
