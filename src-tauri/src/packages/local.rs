use super::*;
use crate::local_import::{LocalSource, MANIFEST, MAX_MANIFEST_BYTES, Manifest};

#[cfg(test)]
mod tests;

impl Packages {
    pub fn import_local(
        &mut self,
        storage: &mut Storage,
        request_id: &str,
        path: &str,
    ) -> Result<Operation> {
        Uuid::parse_str(request_id).map_err(|_| "Invalid import request ID.")?;
        let source = PathBuf::from(path);
        if !source.is_absolute()
            || path.len() > 32768
            || path.chars().any(char::is_control)
            || source.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err("Choose an absolute DLL or folder source path.".into());
        }
        if let Some(operation) = storage
            .package_request(request_id)
            .map_err(|e| e.to_string())?
        {
            if operation.release_id != "local-import" {
                return Err("This request ID belongs to another operation.".into());
            }
            let same_source = self
                .active
                .get(&operation.id)
                .is_some_and(|active| active.source.as_deref() == Some(path))
                || storage
                    .local_sources()
                    .map_err(|e| e.to_string())?
                    .iter()
                    .any(|source| source.reference.hash == operation.hash && source.path == path);
            if matches!(
                operation.status,
                Status::Preparing | Status::Cancelling | Status::Completed
            ) && !same_source
            {
                return Err("This import request belongs to a different or removed source. Start a new import.".into());
            }
            return Ok(operation);
        }
        if self.active.len() >= 3
            || self
                .active
                .values()
                .any(|a| a.source.as_deref() == Some(path))
        {
            return Err("This source is already being imported, or three packages are being prepared. Wait for preparation to finish.".into());
        }
        if !storage
            .pending_removals()
            .map_err(|e| e.to_string())?
            .is_empty()
        {
            return Err("Finish pending uninstall cleanup before importing local content.".into());
        }
        let operation = Operation {
            id: Uuid::new_v4().to_string(),
            request_id: request_id.into(),
            release_id: "local-import".into(),
            hash: "0".repeat(64),
            status: Status::Preparing,
            message: "Copying and verifying the local source. See Downloads for the result.".into(),
            received_bytes: 0,
            total_bytes: crate::runtime_contract::MAX_ACTIVATION_BYTES,
        };
        storage
            .save_package(&operation)
            .map_err(|e| e.to_string())?;
        let cancel = Cancel::default();
        let worker_cancel = cancel.clone();
        let progress = Arc::new(AtomicU64::new(0));
        let worker_progress = progress.clone();
        let (sender, result) = mpsc::sync_channel(1);
        let root = storage.package_root().to_owned();
        let id = operation.id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = sender.send(prepare_local(
                &root,
                &id,
                &source,
                &worker_cancel,
                &worker_progress,
            ));
        });
        self.active.insert(
            operation.id.clone(),
            Active {
                operation: operation.clone(),
                entry: None,
                source: Some(path.into()),
                cancel,
                progress,
                result,
                ready: None,
            },
        );
        Ok(operation)
    }
}

fn prepare_local(
    root: &Path,
    id: &str,
    source: &Path,
    cancel: &Cancel,
    progress: &AtomicU64,
) -> Result<PreparedImport> {
    prepare_source(root, id, source, cancel, progress, Mode::Import)
}

pub(super) enum Mode<'a> {
    Import,
    Inspect,
    Copy(&'a str),
}

pub(super) fn prepare_source(
    root: &Path,
    id: &str,
    source: &Path,
    cancel: &Cancel,
    progress: &AtomicU64,
    mode: Mode<'_>,
) -> Result<PreparedImport> {
    let mut directory = Directory::open(root)?;
    directory.directory("artifacts")?;
    let stage = directory.directory(&format!("package-staging/{id}/content"))?;
    let outcome = (|| {
        let is_folder = fs::symlink_metadata(source)
            .map_err(|e| e.to_string())?
            .is_dir();
        regular_metadata(source, is_folder)?;
        let base = if is_folder {
            source
        } else {
            source.parent().ok_or("The DLL has no parent directory.")?
        };
        let mut pins = Vec::new();
        for parent in base.ancestors().collect::<Vec<_>>().into_iter().rev() {
            pins.push(pin(parent)?);
        }
        let canonical_base = fs::canonicalize(base).map_err(|e| e.to_string())?;
        let canonical_root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        if canonical_base.starts_with(&canonical_root)
            || canonical_root.starts_with(&canonical_base)
        {
            return Err(
                "Choose a developer source outside Starframe's managed data directory.".into(),
            );
        }
        let manifest_path = if is_folder {
            source.join(MANIFEST)
        } else {
            if source.extension().and_then(|e| e.to_str()) != Some("dll") {
                return Err("Choose a .dll file or a folder.".into());
            }
            source.with_extension("starframe.json")
        };
        let mut manifest_file = read_file(&manifest_path).map_err(|e| {
            format!(
                "Local metadata is required at {}. {e}",
                manifest_path.display()
            )
        })?;
        if manifest_file.metadata().map_err(|e| e.to_string())?.len() > MAX_MANIFEST_BYTES {
            return Err("Local metadata exceeds 64 KiB.".into());
        }
        let mut bytes = Vec::new();
        (&mut manifest_file)
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err("Local metadata exceeds 64 KiB.".into());
        }
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid local metadata: {e}"))?;
        manifest.validate()?;
        let paths = source_paths(base, source, is_folder, &manifest_path, &mut pins, cancel)?;
        if paths.is_empty() || paths.len() > crate::runtime_contract::MAX_FILES_PER_MOD {
            return Err("Local builds must contain 1 to 1,024 runtime files.".into());
        }
        let mut handles = Vec::new();
        let mut files = Vec::new();
        let mut total_bytes = 0;
        for path in &paths {
            cancel.check()?;
            let relative = path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("Local paths must use Unicode names.")?
                .replace('\\', "/");
            crate::runtime_contract::relative_path(&relative)?;
            let mut input = read_file(path)?;
            total_bytes += input.metadata().map_err(|e| e.to_string())?.len();
            if total_bytes > crate::runtime_contract::MAX_ACTIVATION_BYTES {
                return Err("The local build exceeds 256 MiB.".into());
            }
            let (size, hash) = digest(&mut input, 64 * 1024 * 1024, cancel)?;
            if !matches!(mode, Mode::Import) && relative.to_ascii_lowercase().ends_with(".dll") {
                super::watch::assembly::check(&mut input)?;
            }
            files.push(PreparedFile {
                path: relative,
                sha256: hash,
                size_bytes: size,
            });
            handles.push(input);
        }
        layout(&files, &manifest.layout)?;
        let mut sorted = files.clone();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));
        let mut hash = Sha256::new();
        hash.update(b"starframe-local-v1\0");
        hash.update(serde_json::to_vec(&(&manifest, &sorted)).map_err(|e| e.to_string())?);
        let hash = format!("{:x}", hash.finalize());
        let prepared = Prepared {
            hash: hash.clone(),
            files: sorted,
        };
        let local = LocalSource {
            reference: ModReference {
                mod_id: manifest.mod_id.clone(),
                hash: hash.clone(),
                origin: Origin::LocalImport,
                release_id: None,
            },
            path: source
                .to_str()
                .ok_or("Source path must use Unicode names.")?
                .into(),
            manifest,
        };
        local.validate()?;
        if matches!(mode, Mode::Inspect) {
            if paths != source_paths(base, source, is_folder, &manifest_path, &mut pins, cancel)? {
                return Err("The local file list changed. Waiting for the build to finish.".into());
            }
            return Ok(PreparedImport {
                prepared,
                local: Some(local),
            });
        }
        if let Mode::Copy(expected) = mode
            && expected != hash
        {
            return Err("The local build changed again. Waiting for writes to settle.".into());
        }
        crate::space::require(root, files.iter().map(|f| f.size_bytes).sum())?;
        for (file, input) in files.iter().zip(&mut handles) {
            input.rewind().map_err(|e| e.to_string())?;
            if let Some((parent, _)) = file.path.rsplit_once('/') {
                directory.directory(&format!("package-staging/{id}/content/{parent}"))?;
            }
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(stage.join(&file.path))
                .map_err(|e| e.to_string())?;
            let mut hash = Sha256::new();
            let mut size = 0;
            let mut buffer = [0; 64 * 1024];
            loop {
                cancel.check()?;
                let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                size += count as u64;
                if size > file.size_bytes {
                    return Err(
                        "The local build changed during import. Finish the build and retry.".into(),
                    );
                }
                output
                    .write_all(&buffer[..count])
                    .map_err(|e| e.to_string())?;
                hash.update(&buffer[..count]);
                progress.fetch_add(count as u64, Ordering::Relaxed);
            }
            if size != file.size_bytes || format!("{:x}", hash.finalize()) != file.sha256 {
                return Err(
                    "The local build changed during import. Finish the build and retry.".into(),
                );
            }
            output.sync_all().map_err(|e| e.to_string())?;
        }
        if paths != source_paths(base, source, is_folder, &manifest_path, &mut pins, cancel)? {
            return Err(
                "The local file list changed during import. Finish the build and retry.".into(),
            );
        }
        cancel.check()?;
        let target = root.join("artifacts").join(&hash);
        match fs::symlink_metadata(&target) {
            Ok(_) => verify_existing(&mut directory, &target, &prepared, cancel)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                directory.pins.retain(|path, _| !path.starts_with(&stage));
                fs::rename(&stage, &target)
                    .map_err(|e| format!("Cannot save the managed copy: {e}"))?;
            }
            Err(e) => return Err(e.to_string()),
        }
        Ok(PreparedImport {
            prepared,
            local: Some(local),
        })
    })();
    match directory.remove_stage(id) {
        Ok(()) => outcome,
        Err(error) => Err(format!(
            "{} Staging cleanup failed: {error}",
            outcome.err().unwrap_or_else(|| {
                "The local copy was verified; its library commit was withheld.".into()
            })
        )),
    }
}

fn source_paths(
    base: &Path,
    source: &Path,
    folder: bool,
    manifest: &Path,
    pins: &mut Vec<File>,
    cancel: &Cancel,
) -> Result<Vec<PathBuf>> {
    if !folder {
        return Ok(vec![source.into()]);
    }
    let mut pending = vec![source.to_owned()];
    let mut files = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    let mut count = 0;
    while let Some(directory) = pending.pop() {
        pins.push(pin(&directory)?);
        for item in fs::read_dir(directory).map_err(|e| e.to_string())? {
            cancel.check()?;
            count += 1;
            if count > MAX_ENTRIES {
                return Err("Local source exceeds 4,096 directory entries. Choose only the build output folder.".into());
            }
            let path = item.map_err(|e| e.to_string())?.path();
            let directory = fs::symlink_metadata(&path)
                .map_err(|e| e.to_string())?
                .is_dir();
            regular_metadata(&path, directory)?;
            let relative = path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("Local paths must use Unicode names.")?
                .replace('\\', "/");
            let normalized = crate::runtime_contract::relative_path(&relative)?;
            if !names.insert(normalized) {
                return Err("Local source contains case-colliding paths.".into());
            }
            if directory {
                pending.push(path);
            } else if path != manifest {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}
