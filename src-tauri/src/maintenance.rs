use crate::{
    deployment,
    filesystem::{pin, regular_metadata},
    storage::Storage,
};
use std::{fs, path::Path};

/// Called only by the finite installer command, with a resolved application data root.
pub fn uninstall(root: &Path, keep_data: bool) -> Result<(), String> {
    if !root.try_exists().map_err(|e| e.to_string())? {
        return Ok(());
    }
    let root_pin = pin(root)?;
    let mut directories: Vec<_> = [
        "artifacts",
        "package-staging",
        "deployment-content",
        "backups",
        "webview",
        "EBWebView",
        "logs",
    ]
    .iter()
    .map(|name| root.join(name))
    .collect();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix("sqlite-staging-"))
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        {
            directories.push(entry.path());
        }
    }
    directories.push(root.join("sqlite"));
    for path in &directories {
        if path.try_exists().map_err(|e| e.to_string())? {
            verify_tree(path, 0)?;
        }
    }
    let mut store = Storage::open(root).map_err(|e| e.to_string())?;
    deployment::remove_all(&mut store)?;
    if keep_data {
        return Ok(());
    }
    for source in store.local_sources().map_err(|e| e.to_string())? {
        if Path::new(&source.path)
            .canonicalize()
            .unwrap_or_else(|_| source.path.into())
            .starts_with(&root.canonicalize().map_err(|e| e.to_string())?)
        {
            return Err("An original local import is inside Starframe's data folder. Choose to keep your data or move and reimport that source before uninstalling.".into());
        }
    }
    // Keep the storage lock after closing SQLite, throughout data removal.
    let lock = store.close_for_removal();
    for path in &directories {
        if path.try_exists().map_err(|e| e.to_string())? {
            remove_tree(path, 0)?;
        }
    }
    for name in ["state.db", "state.db-wal", "state.db-shm"] {
        let path = root.join(name);
        if path.try_exists().map_err(|e| e.to_string())? {
            regular_metadata(&path, false)?;
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    drop(lock);
    fs::remove_file(root.join("state.lock")).map_err(|e| e.to_string())?;
    drop(root_pin);
    // Unknown files remain; remove the application data directory only when empty.
    match fs::remove_dir(root) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn verify_tree(path: &Path, depth: usize) -> Result<(), String> {
    if depth > 64 {
        return Err("App data is nested too deeply. Data removal stopped.".into());
    }
    regular_metadata(path, true)?;
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let info = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        regular_metadata(&path, info.is_dir())?;
        if info.is_dir() {
            verify_tree(&path, depth + 1)?;
        }
    }
    Ok(())
}

fn remove_tree(path: &Path, depth: usize) -> Result<(), String> {
    if depth > 64 {
        return Err("App data is nested too deeply. Data removal stopped.".into());
    }
    let directory = pin(path)?;
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let info = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        regular_metadata(&path, info.is_dir())?;
        if info.is_dir() {
            remove_tree(&path, depth + 1)?;
        } else {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    drop(directory);
    fs::remove_dir(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_retention_and_default_deletion_preserve_unowned_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("data");
        let store = Storage::open(&root).unwrap();
        fs::create_dir(root.join("artifacts")).unwrap();
        fs::write(root.join("artifacts/managed.dll"), b"managed copy").unwrap();
        fs::write(root.join("unowned.txt"), b"unowned").unwrap();
        let staging = root.join(format!("sqlite-staging-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&staging).unwrap();
        fs::write(staging.join("state.db"), b"interrupted conversion").unwrap();
        fs::write(temp.path().join("source.dll"), b"original source").unwrap();
        assert!(
            uninstall(&root, false)
                .unwrap_err()
                .contains("already open")
        );
        drop(store);
        uninstall(&root, true).unwrap();
        assert!(root.join("sqlite/state.db").exists());
        assert!(root.join("artifacts/managed.dll").exists());
        uninstall(&root, false).unwrap();
        assert!(!root.join("sqlite").exists());
        assert!(!staging.exists());
        assert!(!root.join("artifacts").exists());
        assert_eq!(fs::read(root.join("unowned.txt")).unwrap(), b"unowned");
        assert_eq!(
            fs::read(temp.path().join("source.dll")).unwrap(),
            b"original source"
        );
    }

    #[test]
    fn unavailable_recorded_game_prevents_data_deletion() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("data");
        let mut store = Storage::open(&root).unwrap();
        let missing = temp.path().join("unavailable-game");
        store
            .save_deployment(
                &missing.to_string_lossy(),
                &format!(
                    r#"{{"version":1,"owned":{{"winhttp.dll":"{}"}},"pending":null}}"#,
                    "a".repeat(64)
                ),
                &[],
            )
            .unwrap();
        drop(store);
        assert!(uninstall(&root, false).is_err());
        let store = Storage::open(&root).unwrap();
        assert_eq!(store.deployment_roots().unwrap().len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn junction_preflight_preserves_external_content_and_database() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("data");
        drop(Storage::open(&root).unwrap());
        let external = temp.path().join("external");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("source.dll"), b"original").unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(root.join("artifacts"))
            .arg(&external)
            .output()
            .unwrap();
        assert!(status.status.success());
        assert!(uninstall(&root, false).is_err());
        assert!(root.join("sqlite/state.db").exists());
        assert_eq!(fs::read(external.join("source.dll")).unwrap(), b"original");
        fs::remove_dir(root.join("artifacts")).unwrap();
    }
}
