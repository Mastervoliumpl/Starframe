use super::*;

pub(crate) struct Directory {
    pub(super) root: PathBuf,
    pub(super) pins: BTreeMap<PathBuf, File>,
}

pub(crate) fn verify_artifact(store: &Storage, reference: &ModReference) -> Result<Prepared> {
    let prepared = store
        .prepared_artifact(&reference.hash)
        .map_err(|e| e.to_string())?
        .ok_or("The package has no verified file manifest. Prepare it again.")?;
    supported_files(&prepared.files)?;
    let mut directory = Directory::open(store.package_root())?;
    verify_existing(
        &mut directory,
        &store
            .artifact_directory(reference)
            .map_err(|e| e.to_string())?,
        &prepared,
        &Cancel::default(),
    )?;
    Ok(prepared)
}

pub(crate) fn remove_artifact(store: &Storage, hash: &str) -> Result<()> {
    if store
        .load()
        .map_err(|e| e.to_string())?
        .library
        .iter()
        .any(|entry| entry.reference.hash == hash)
    {
        return Err("This artifact is still in the library. Cleanup retained it.".into());
    }
    let reference = ModReference {
        mod_id: "artifact".into(),
        hash: hash.into(),
        origin: Origin::LocalImport,
        release_id: None,
    };
    let root = store
        .artifact_directory(&reference)
        .map_err(|e| e.to_string())?;
    match fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
        Ok(_) => (),
    }
    let prepared = store
        .prepared_artifact(hash)
        .map_err(|e| e.to_string())?
        .ok_or("Uninstall inventory is missing. Files were retained.")?;
    let mut directory = Directory::open(store.package_root())?;
    directory.directory(&format!("artifacts/{hash}"))?;
    let mut pending = vec![root.clone()];
    let mut folders = Vec::new();
    let mut files = Vec::new();
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
            if files.len() + folders.len() + pending.len() > MAX_ENTRIES * 32 {
                return Err("Uninstall cleanup exceeded its file limit.".into());
            }
            let path = entry.map_err(|e| e.to_string())?.path();
            let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            regular_metadata(&path, metadata.is_dir())?;
            let relative = path
                .strip_prefix(&root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            crate::runtime_contract::relative_path(&relative)?;
            if metadata.is_dir() {
                directory.directory(&format!("artifacts/{hash}/{relative}"))?;
                pending.push(path);
            } else {
                let expected = prepared
                    .files
                    .iter()
                    .find(|f| f.path == relative)
                    .ok_or("Uninstall found an unexpected file. It was retained.")?;
                if digest(
                    &mut read_file(&path)?,
                    expected.size_bytes,
                    &Cancel::default(),
                )? != (expected.size_bytes, expected.sha256.clone())
                {
                    return Err(
                        "Uninstall found changed content. It was retained for repair.".into(),
                    );
                }
                files.push(path);
            }
        }
        folders.push(folder);
    }
    for path in files {
        fs::remove_file(path).map_err(|e| {
            format!("Uninstall cleanup failed: {e}. Retry after the file is unlocked.")
        })?;
    }
    for folder in folders.into_iter().rev() {
        directory.pins.remove(&folder);
        fs::remove_dir(folder).map_err(|e| format!("Uninstall cleanup failed: {e}"))?;
    }
    Ok(())
}
impl Directory {
    pub(crate) fn open(root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err("Package storage requires an absolute app-data path.".into());
        }
        let mut pins = BTreeMap::new();
        for part in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
            pins.insert(part.to_owned(), pin(part)?);
        }
        Ok(Self {
            root: root.into(),
            pins,
        })
    }
    pub(crate) fn directory(&mut self, relative: &str) -> Result<PathBuf> {
        crate::runtime_contract::relative_path(relative)?;
        let mut path = self.root.clone();
        for part in relative.split('/') {
            path.push(part);
            if !self.pins.contains_key(&path) {
                match fs::create_dir(&path) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(format!("Cannot create package staging: {e}")),
                }
                self.pins.insert(path.clone(), pin(&path)?);
            }
        }
        Ok(path)
    }

    fn remove_stage(&mut self, id: &str) -> Result<()> {
        Uuid::parse_str(id).map_err(|_| "Invalid staging operation ID.")?;
        let stage = self.root.join("package-staging").join(id);
        if !stage.starts_with(&self.root) {
            return Err("Staging path escaped app data.".into());
        }
        let mut pending = vec![stage.clone()];
        let mut directories = Vec::new();
        let mut files = Vec::new();
        while let Some(folder) = pending.pop() {
            self.pins.insert(folder.clone(), pin(&folder)?);
            for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if directories.len() + pending.len() + files.len() > MAX_ENTRIES * 32 {
                    return Err("Staging cleanup exceeded its file limit.".into());
                }
                let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                regular_metadata(&path, metadata.is_dir())?;
                if metadata.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path);
                }
            }
            directories.push(folder);
        }
        for file in files {
            fs::remove_file(file).map_err(|e| e.to_string())?;
        }
        for folder in directories.into_iter().rev() {
            self.pins.remove(&folder);
            fs::remove_dir(folder).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

pub(super) async fn prepare(
    root: PathBuf,
    id: String,
    artifact: Artifact,
    previous: Option<Prepared>,
    client: reqwest::Client,
    cancel: Cancel,
    progress: Arc<AtomicU64>,
) -> Result<Prepared> {
    let worker_artifact = artifact.clone();
    let worker_cancel = cancel.clone();
    let stage_id = id.clone();
    let setup = tauri::async_runtime::spawn_blocking(move || {
        let mut directory = Directory::open(&root)?;
        directory.directory("artifacts")?;
        let final_path = root.join("artifacts").join(&worker_artifact.sha256);
        if let Some(previous) = previous {
            verify_existing(&mut directory, &final_path, &previous, &worker_cancel)?;
            layout(&previous.files, &worker_artifact.layout)?;
            return Ok((directory, None, Some(previous)));
        }
        let stage = directory.directory(&format!("package-staging/{stage_id}"))?;
        Ok::<_, String>((directory, Some(stage), None))
    })
    .await
    .map_err(|e| format!("Package staging worker failed: {e}"))??;
    let (mut directory, stage, previous) = setup;
    if let Some(previous) = previous {
        return Ok(previous);
    }
    let stage = stage.ok_or("Package staging was not created.")?;
    let archive = stage.join("download.zip");
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&archive)
        .await
        .map_err(|e| format!("Cannot create package download: {e}"))?;
    let downloaded = tokio::select! {
        _ = cancel.cancelled() => Err(CANCELLED.into()),
        _ = tokio::time::sleep(Duration::from_secs(300)) => Err("Package transfer exceeded five minutes. Retry on a faster connection.".into()),
        result = transfer::download(&client, &artifact, &mut file, &progress) => result,
    };
    let downloaded = match downloaded {
        Ok(()) => file
            .sync_all()
            .await
            .map_err(|e| format!("Cannot flush the package download: {e}")),
        Err(error) => Err(error),
    };
    drop(file);
    if let Err(error) = downloaded {
        return tauri::async_runtime::spawn_blocking(move || match directory.remove_stage(&id) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(format!(
                "{error} Staging {id} was retained because cleanup failed: {cleanup}"
            )),
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let outcome = (|| {
        cancel.check()?;
        let content = directory.directory(&format!("package-staging/{id}/content"))?;
        let prepared = extract(&archive, &content, &artifact, &cancel)?;
        cancel.check()?;
        let final_path = directory.root.join("artifacts").join(&artifact.sha256);
        match fs::symlink_metadata(&final_path) {
            Ok(_) => {
                // A crash may leave promoted bytes without a database commit. Never overwrite them.
                verify_existing(&mut directory, &final_path, &prepared, &cancel)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                directory.pins.remove(&content);
                fs::rename(&content, &final_path).map_err(|e| format!("Cannot promote the verified package: {e}. Retry after checking app-data permissions."))?;
            }
            Err(e) => return Err(e.to_string()),
        }
        Ok(prepared)
        })();
        match directory.remove_stage(&id) {
            Ok(()) => outcome,
            Err(cleanup) => Err(format!("{} Staging {id} was retained because cleanup failed: {cleanup}", outcome.err().unwrap_or_else(|| "Package files verified; library commit was withheld.".into()))),
        }
    }).await.map_err(|e| format!("Package extraction worker failed: {e}"))?
}

pub(super) fn verify_existing(
    directory: &mut Directory,
    path: &Path,
    prepared: &Prepared,
    cancel: &Cancel,
) -> Result<()> {
    regular_metadata(path, true)?;
    directory.directory(&format!("artifacts/{}", prepared.hash))?;
    let mut pending = vec![path.to_owned()];
    let mut found = BTreeMap::new();
    let mut entries = 0;
    while let Some(folder) = pending.pop() {
        let _pin = pin(&folder)?;
        for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
            cancel.check()?;
            entries += 1;
            if entries > MAX_ENTRIES * 32 {
                return Err("Stored package has too many entries.".into());
            }
            let target = entry.map_err(|e| e.to_string())?.path();
            let metadata = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
            regular_metadata(&target, metadata.is_dir())?;
            let relative = target
                .strip_prefix(path)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            crate::runtime_contract::relative_path(&relative)?;
            if metadata.is_dir() {
                directory.directory(&format!("artifacts/{}/{}", prepared.hash, relative))?;
                pending.push(target);
                continue;
            }
            let expected = prepared
                .files
                .iter()
                .find(|file| file.path == relative)
                .ok_or("Stored package has unexpected files. Retain it for repair.")?;
            let (size, hash) = digest(&mut read_file(&target)?, expected.size_bytes, cancel)?;
            if size != expected.size_bytes || hash != expected.sha256 {
                return Err(
                    "Stored package was changed. Retain it for repair; no files were overwritten."
                        .into(),
                );
            }
            found.insert(relative, (size, hash));
        }
    }
    if found.len() != prepared.files.len() {
        return Err("Stored package is incomplete. Retain it for repair.".into());
    }
    Ok(())
}
