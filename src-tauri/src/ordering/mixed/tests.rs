use super::*;
use crate::{
    catalog::Layout,
    local_import::Manifest,
    references::LocalReference,
    registry::{Dependency, ExactReference, ModId, ReleaseId, Sha256},
    storage::{ModReference, Origin, RegistryDisplay},
};

fn registry(id: u64) -> RegistryLibraryEntry {
    RegistryLibraryEntry {
        reference:ExactReference { mod_id:ModId::try_from(id).unwrap(), release_id:ReleaseId(uuid::Uuid::new_v4()), sha256:Sha256::try_from("a".repeat(64)).unwrap() },
        display:RegistryDisplay { name:format!("Registry {id}"), author:String::new() },
        version_label:"1.2.3".into(), tested_game_build:"fixture".into(), metadata_revision:1,
        installation:serde_json::from_value(serde_json::json!({"schemaVersion":1,"kind":"code","loader":"lua","entryPath":"LJ/lua/main.lua"})).unwrap(), dependencies:vec![],
    }
}

fn local(id: &str) -> LocalSource {
    LocalSource {
        reference: ModReference {
            mod_id: id.into(),
            hash: "b".repeat(64),
            origin: Origin::LocalImport,
            release_id: None,
        },
        path: std::env::temp_dir()
            .join("inert-source")
            .to_string_lossy()
            .into_owned(),
        manifest: Manifest {
            schema_version: 1,
            mod_id: id.into(),
            name: id.into(),
            author: "fixture".into(),
            version: "1.0.0".into(),
            layout: Layout::StarframeLuaZip {},
            requires: vec![],
            load_before: vec![],
            load_after: vec![],
            prefer_before: vec![],
            prefer_after: vec![],
        },
    }
}

#[test]
fn combined_order_keeps_manual_interleaving_and_exact_dependency_edges() {
    let mut registry_first = registry(1);
    let registry_required = registry(2);
    registry_first.dependencies.push(Dependency::Exact {
        mod_id: registry_required.reference.mod_id,
        release_id: registry_required.reference.release_id,
    });
    let local_required = local("local.required");
    let mut local_first = local("local.first");
    local_first
        .manifest
        .requires
        .push(local_required.reference.clone());
    let requested = vec![
        Reference::try_from(&local_first.reference).unwrap(),
        Reference::Registry(registry_first.reference.clone()),
        Reference::try_from(&local_required.reference).unwrap(),
        Reference::Registry(registry_required.reference.clone()),
    ];
    let installed = [registry_first.clone(), registry_required.clone()];
    let locals = [local_first.clone(), local_required.clone()];
    let resolved = resolve_mixed(&installed, &locals, &requested).unwrap();
    assert_eq!(
        resolved.effective,
        vec![
            requested[2].clone(),
            requested[0].clone(),
            requested[3].clone(),
            requested[1].clone()
        ]
    );
    assert_eq!(resolved.adjustments.len(), 2);
    assert!(resolve_mixed(&installed, &locals, &requested[..3]).is_err());
    let mut changed = requested.clone();
    changed[1] = Reference::Registry(registry(1).reference);
    assert!(resolve_mixed(&installed, &locals, &changed).is_err());
    assert!(
        resolve_mixed(
            &installed,
            &locals,
            &[requested[1].clone(), requested[1].clone()]
        )
        .is_err()
    );
}

#[test]
fn local_constraints_keep_required_precedence_and_never_map_catalog_dependencies() {
    let first = local("local.first");
    let mut second = local("local.second");
    second.manifest.requires.push(first.reference.clone());
    second
        .manifest
        .prefer_before
        .push(first.reference.mod_id.clone());
    let registry = registry(1);
    let requested = vec![
        Reference::try_from(&second.reference).unwrap(),
        Reference::Registry(registry.reference.clone()),
        Reference::try_from(&first.reference).unwrap(),
    ];
    let result = resolve_mixed(
        std::slice::from_ref(&registry),
        &[first.clone(), second.clone()],
        &requested,
    )
    .unwrap();
    assert_eq!(
        result.effective,
        vec![
            requested[1].clone(),
            requested[2].clone(),
            requested[0].clone()
        ]
    );
    assert_eq!(result.adjustments.len(), 2);
    second
        .manifest
        .load_before
        .push(first.reference.mod_id.clone());
    assert!(
        resolve_mixed(
            std::slice::from_ref(&registry),
            &[first.clone(), second.clone()],
            &requested
        )
        .unwrap_err()
        .contains("cycle")
    );
    second.manifest.load_before.clear();
    second.manifest.requires[0].origin = Origin::Catalog;
    second.manifest.requires[0].release_id = Some("old.catalog".into());
    assert!(
        resolve_mixed(
            std::slice::from_ref(&registry),
            &[first, second],
            &requested
        )
        .unwrap_err()
        .contains("obsolete catalog")
    );
    let collision = local("registry.1");
    let local_reference = Reference::Local(LocalReference {
        mod_id: "registry.1".into(),
        sha256: Sha256::try_from("b".repeat(64)).unwrap(),
    });
    assert!(
        resolve_mixed(
            std::slice::from_ref(&registry),
            &[collision],
            &[
                Reference::Registry(registry.reference.clone()),
                local_reference
            ]
        )
        .unwrap_err()
        .contains("conflicting runtime")
    );
}
