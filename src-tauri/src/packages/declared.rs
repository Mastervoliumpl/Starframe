use super::*;
use crate::registry::trust::VerifiedRelease;

pub fn prepare_registry_archive(
    root: &Path,
    operation_id: Uuid,
    release: &VerifiedRelease,
    cancelled: Arc<AtomicBool>,
) -> Result<Prepared> {
    if operation_id.is_nil() {
        return Err("Invalid registry preparation operation ID.".into());
    }
    let identity = release
        .download_identity()
        .ok_or("The exact registry release is unavailable or blocked.")?;
    let crate::registry::ReleaseResult::Release(release) = &release.release else {
        return Err("The registry release has no installation metadata.".into());
    };
    let plan = release.metadata.installation.as_ref().ok_or("This release has no approved installation declaration. Ask the author for a new declared release.")?;
    let cancel = Cancel {
        flag: cancelled,
        ..Default::default()
    };
    cancel.check()?;
    let mut directory = Directory::open(root)?;
    let archives = directory.directory("registry-archives")?;
    directory.directory("artifacts")?;
    let id = operation_id.to_string();
    let content = directory.directory(&format!("package-staging/{id}/content"))?;
    let outcome = (|| {
        let archive = archives.join(format!("{}.zip", identity.reference().sha256.as_str()));
        let prepared = archive::extract_declared(&archive, &content, &identity, plan, &cancel)?;
        cancel.check()?;
        let final_path = root.join("artifacts").join(&prepared.hash);
        match fs::symlink_metadata(&final_path) {
            Ok(_) => verify_existing(&mut directory, &final_path, &prepared, &cancel)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                directory.pins.retain(|path, _| !path.starts_with(&content));
                fs::rename(&content, &final_path).map_err(|error| format!("Cannot save verified registry content: {error}. Retry after checking app-data permissions."))?;
            }
            Err(error) => return Err(error.to_string()),
        }
        Ok(prepared)
    })();
    match directory.remove_stage(&id) {
        Ok(()) => outcome,
        Err(cleanup) => Err(format!(
            "{} Staging {id} was retained because cleanup failed: {cleanup}",
            outcome.err().unwrap_or_else(|| {
                "Registry content verified; library commit was withheld.".into()
            })
        )),
    }
}

#[cfg(test)]
pub(crate) mod tests;
