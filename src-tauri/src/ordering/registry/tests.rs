use super::*;
use crate::{
    registry::{Dependency, ReleaseId, Sha256},
    storage::RegistryDisplay,
};

fn entry(id: u64, label: &str) -> RegistryLibraryEntry {
    RegistryLibraryEntry {
        reference: ExactReference {
            mod_id: ModId::try_from(id).unwrap(),
            release_id: ReleaseId(uuid::Uuid::new_v4()),
            sha256: Sha256::try_from("b".repeat(64)).unwrap(),
        },
        display: RegistryDisplay {
            name: format!("Mod {id}"),
            author: String::new(),
        },
        version_label: label.into(),
        tested_game_build: "Fixture build".into(),
        metadata_revision: 1,
        installation: serde_json::from_value(serde_json::json!({
            "schemaVersion":1,"kind":"code","loader":"lua","entryPath":"LJ/lua/main.lua"
        }))
        .unwrap(),
        dependencies: vec![],
    }
}

#[test]
fn native_order_pins_exact_approvals_and_preserves_unrelated_priority() {
    let mut dependent = entry(1, "version text");
    let required = entry(2, "same label");
    let unused = entry(3, "1.0.0");
    dependent.dependencies.push(Dependency::Exact {
        mod_id: required.reference.mod_id,
        release_id: required.reference.release_id,
    });
    let requested = vec![
        dependent.reference.clone(),
        unused.reference.clone(),
        required.reference.clone(),
    ];
    let result = resolve_registry(
        &[dependent.clone(), required.clone(), unused.clone()],
        &requested,
    )
    .unwrap();
    assert_eq!(
        result.effective,
        vec![
            unused.reference,
            required.reference.clone(),
            dependent.reference.clone()
        ]
    );
    assert_eq!(result.adjustments.len(), 1);
    let other = entry(2, "same label");
    assert_eq!(other.reference.sha256, required.reference.sha256);
    assert!(
        resolve_registry(
            &[dependent.clone(), other.clone()],
            &[dependent.reference.clone(), other.reference.clone()]
        )
        .unwrap_err()
        .contains("different release")
    );
    assert!(
        resolve_registry(&[dependent.clone(), other], &requested)
            .unwrap_err()
            .contains("not installed")
    );
    assert!(
        resolve_registry(&[dependent.clone(), required], &[dependent.reference])
            .unwrap_err()
            .contains("Enable a matching")
    );
}

#[test]
fn native_ranges_and_cycles_reject_invalid_selected_setups_without_replacement() {
    let mut dependent = entry(1, "1.0.0");
    let mut required = entry(2, "1.5.0");
    dependent.dependencies.push(Dependency::Range {
        mod_id: required.reference.mod_id,
        minimum: Some("1.0.0".into()),
        before: Some("2.0.0".into()),
        include_prerelease: false,
    });
    let requested = vec![dependent.reference.clone(), required.reference.clone()];
    assert_eq!(
        resolve_registry(&[dependent.clone(), required.clone()], &requested)
            .unwrap()
            .effective,
        vec![required.reference.clone(), dependent.reference.clone()]
    );
    required.version_label = "2.0.0".into();
    assert!(resolve_registry(&[dependent.clone(), required.clone()], &requested).is_err());
    required.version_label = "1.5.0-alpha".into();
    assert!(resolve_registry(&[dependent.clone(), required.clone()], &requested).is_err());
    required.version_label = "1.5.0".into();
    required.dependencies.push(Dependency::Exact {
        mod_id: dependent.reference.mod_id,
        release_id: dependent.reference.release_id,
    });
    assert!(
        resolve_registry(&[dependent.clone(), required.clone()], &requested)
            .unwrap_err()
            .contains("cycle")
    );
    assert!(
        resolve_registry(
            &[dependent.clone(), required],
            &[dependent.reference.clone(), dependent.reference]
        )
        .unwrap_err()
        .contains("Only one")
    );
}
