use super::*;

#[test]
fn streamed_mod_update_failure_restores_large_owned_files_and_preserves_unknown_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("engine");
    fs::create_dir(&root).unwrap();
    let mut store = Storage::open(&temp.path().join("data")).unwrap();
    let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
    let source = temp.path().join("fixture.dll");
    let first = vec![1; 9 * 1024 * 1024];
    let second = vec![2; first.len()];
    let path = "Starframe/mods/fixture/Fixture.dll";
    let make = |bytes: &[u8]| {
        fs::write(&source, bytes).unwrap();
        vec![(
            path.into(),
            Source::File {
                path: source.clone(),
                hash: hash(bytes),
                size: bytes.len() as u64,
            },
        )]
    };
    fs::write(root.join("user-settings.cfg"), b"retain").unwrap();
    apply_sources(
        &mut store,
        &mut engine,
        make(&first),
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    let mut failed = false;
    let error = apply_sources(
        &mut store,
        &mut engine,
        make(&second),
        &mut || Ok(()),
        &mut |step| {
            if step == "file-changed" && !failed {
                failed = true;
                return Err("Injected failed write boundary".into());
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("Previous deployment restored"));
    assert_eq!(fs::read(root.join(path)).unwrap(), first);
    assert!(load(&store, &root).unwrap().pending.is_none());
    apply_sources(
        &mut store,
        &mut engine,
        make(&second),
        &mut || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(fs::read(root.join(path)).unwrap(), second);
    apply(&mut store, &mut engine, vec![], &mut || Ok(()), &mut |_| {
        Ok(())
    })
    .unwrap();
    assert!(!root.join(path).exists());
    assert_eq!(fs::read(root.join("user-settings.cfg")).unwrap(), b"retain");
}

#[test]
fn streamed_source_hash_failure_and_corrupt_backup_leave_the_game_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("engine");
    fs::create_dir(&root).unwrap();
    let mut store = Storage::open(&temp.path().join("data")).unwrap();
    let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
    let source = temp.path().join("fixture.dll");
    fs::write(&source, b"changed").unwrap();
    let make = || {
        vec![(
            "Starframe/mods/fixture/Fixture.dll".into(),
            Source::File {
                path: source.clone(),
                hash: hash(b"approved"),
                size: 8,
            },
        )]
    };
    assert!(
        apply_sources(
            &mut store,
            &mut engine,
            make(),
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .is_err()
    );
    assert!(!root.join("Starframe/mods/fixture/Fixture.dll").exists());
    fs::write(&source, b"approved").unwrap();
    let backup = store
        .package_root()
        .join("deployment-content")
        .join(hash(b"approved"));
    fs::write(backup, b"corrupt").unwrap();
    assert!(
        apply_sources(
            &mut store,
            &mut engine,
            make(),
            &mut || Ok(()),
            &mut |_| Ok(())
        )
        .is_err()
    );
    assert!(!root.join("Starframe/mods/fixture/Fixture.dll").exists());
}
