use super::*;
use crate::packages::{Directory, read_file};
use std::io::{Cursor, Seek};

pub(super) const FILE_LIMIT: u64 = 512 * 1024 * 1024;
pub(super) const TOTAL_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

pub(crate) enum Source {
    Bytes(Vec<u8>),
    File {
        path: PathBuf,
        hash: String,
        size: u64,
    },
}

pub(super) fn digest_file(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(_) => (),
    }
    let _parents = Directory::open(path.parent().ok_or("Missing file parent.")?)?;
    let mut file = read_file(path)?;
    digest(&mut file).map(|(_, hash)| Some(hash))
}

fn digest(input: &mut impl Read) -> Result<(u64, String)> {
    let mut sha = Sha256::new();
    let mut size = 0;
    let mut buffer = [0; 65536];
    loop {
        let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > FILE_LIMIT {
            return Err("Deployment file exceeds 512 MiB.".into());
        }
        sha.update(&buffer[..count]);
    }
    Ok((size, format!("{:x}", sha.finalize())))
}

pub(super) fn retain_file(store: &Storage, path: &Path, expected: &str, size: u64) -> Result<()> {
    if !valid_hash(expected) || size > FILE_LIMIT {
        return Err("Invalid deployment source.".into());
    }
    let mut directory = Directory::open(store.package_root())?;
    let backup = directory.directory("deployment-content")?;
    let target = backup.join(expected);
    if let Some(actual) = digest_file(&target)? {
        if actual != expected {
            return Err("Retained deployment content is corrupt. Files were retained.".into());
        }
        return Ok(());
    }
    let _source = Directory::open(path.parent().ok_or("Missing source parent.")?)?;
    let mut input = read_file(path)?;
    if input.metadata().map_err(|e| e.to_string())?.len() != size {
        return Err("Deployment source size changed during preparation.".into());
    }
    crate::space::require(&backup, size)?;
    let temporary = backup.join(format!("{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let copied = std::io::copy(&mut (&mut input).take(FILE_LIMIT + 1), &mut output)
            .map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        if copied != size || digest_file(&temporary)?.as_deref() != Some(expected) {
            return Err("Deployment source changed during preparation.".into());
        }
        fs::rename(&temporary, &target).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(super) fn reader(store: &Storage, hash: &str) -> Result<Box<dyn Read>> {
    if !valid_hash(hash) {
        return Err("Invalid deployment content hash.".into());
    }
    let path = store.package_root().join("deployment-content").join(hash);
    if fs::symlink_metadata(&path).is_ok() {
        let _parents = Directory::open(path.parent().unwrap())?;
        let mut input = read_file(&path)?;
        if digest(&mut input)?.1 != hash {
            return Err("Retained deployment content failed verification.".into());
        }
        input.rewind().map_err(|e| e.to_string())?;
        return Ok(Box::new(input));
    }
    store
        .deployment_blob(hash)
        .map(|bytes| Box::new(Cursor::new(bytes)) as Box<dyn Read>)
        .map_err(|e| e.to_string())
}
