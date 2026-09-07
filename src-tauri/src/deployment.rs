use crate::{game, storage::Storage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

mod content;
#[cfg(test)]
mod stream_tests;
pub(crate) use content::Source;

type Result<T> = std::result::Result<T, String>;
type Files = BTreeMap<String, String>;
const MAX_FILE: u64 = 8_388_608;
const MAX_TOTAL: usize = 33_554_432;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    owned: Files,
    pending: Option<Journal>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    id: Uuid,
    next: Files,
    borrowed: Files,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn relative(value: &str) -> Result<()> {
    crate::runtime_contract::relative_path(value).map(|_| ())
}
fn validate_files(files: &Files) -> Result<()> {
    if files.len() > 8704 {
        return Err("Too many deployment files.".into());
    }
    let mut paths = BTreeSet::new();
    for (path, digest) in files {
        relative(path)?;
        if !valid_hash(digest) || !paths.insert(path.to_ascii_lowercase()) {
            return Err("Invalid or repeated deployment path/hash.".into());
        }
    }
    for path in &paths {
        let mut parent = Path::new(path).parent();
        while let Some(part) = parent {
            if paths.contains(&part.to_string_lossy().to_string()) {
                return Err("Overlapping deployment files.".into());
            }
            parent = part.parent();
        }
    }
    Ok(())
}
fn load(store: &Storage, root: &Path) -> Result<Record> {
    let value = store
        .deployment_record(&root.to_string_lossy())
        .map_err(|e| e.to_string())?;
    let record: Record = match value {
        Some(value) if value.len() <= 1_048_576 => {
            serde_json::from_str(&value).map_err(|e| format!("Invalid deployment journal: {e}"))?
        }
        Some(_) => return Err("Deployment journal exceeds its size limit.".into()),
        None => Record {
            version: 1,
            ..Record::default()
        },
    };
    if record.version != 1 {
        return Err("Unsupported deployment journal version. Files were retained.".into());
    }
    validate_files(&record.owned)?;
    if let Some(pending) = &record.pending {
        validate_files(&pending.next)?;
        validate_files(&pending.borrowed)?;
        if pending.id.is_nil()
            || pending
                .next
                .keys()
                .any(|p| pending.borrowed.contains_key(p))
        {
            return Err("Invalid deployment journal ownership.".into());
        }
    }
    Ok(record)
}
fn save(
    store: &mut Storage,
    root: &Path,
    record: &Record,
    blobs: &[(String, Vec<u8>)],
) -> Result<()> {
    store
        .save_deployment(
            &root.to_string_lossy(),
            &serde_json::to_string(record).map_err(|e| e.to_string())?,
            blobs,
        )
        .map_err(|e| e.to_string())
}

struct Engine {
    root: PathBuf,
    _lock: File,
    pins: Vec<File>,
    pinned: BTreeSet<PathBuf>,
}
pub(crate) fn regular_metadata(path: &Path, directory: bool) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(format!(
                "Reparse point is not an owned path: {}",
                path.display()
            ));
        }
    }
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(format!("Unsupported path: {}", path.display()));
    }
    Ok(metadata)
}
pub(crate) fn pin(path: &Path) -> Result<File> {
    regular_metadata(path, true)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Hold directory names stable while hashing and replacing their children.
        options.share_mode(3).custom_flags(0x02000000);
    }
    let handle = options.open(path).map_err(|e| e.to_string())?;
    regular_metadata(path, true)?;
    Ok(handle)
}
impl Engine {
    fn open(root: &Path, guard: &mut impl FnMut() -> Result<()>) -> Result<Self> {
        guard()?;
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let mut pins = Vec::new();
        let mut pinned = BTreeSet::new();
        for part in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
            pins.push(pin(part)?);
            pinned.insert(part.to_owned());
        }
        let lock_path = root.join(".starframe-bootstrap.lock");
        let magic = b"Starframe bootstrap ownership lock v1\n";
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(3);
        }
        let lock = match options.open(&lock_path) {
            Ok(mut file) => {
                file.try_lock().map_err(|e| e.to_string())?;
                file.write_all(magic).map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
                file
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                regular_metadata(&lock_path, false)?;
                let mut existing = OpenOptions::new();
                existing.read(true).write(true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    existing.share_mode(3);
                }
                let file = existing.open(&lock_path).map_err(|e| e.to_string())?;
                file.try_lock()
                    .map_err(|_| "Another Starframe operation owns this game installation.")?;
                let mut contents = Vec::new();
                (&file)
                    .take(128)
                    .read_to_end(&mut contents)
                    .map_err(|e| e.to_string())?;
                if contents != magic {
                    return Err("Unrecognized bootstrap lock file. It was retained.".into());
                }
                file
            }
            Err(e) => return Err(e.to_string()),
        };
        Ok(Self {
            root,
            _lock: lock,
            pins,
            pinned,
        })
    }
    fn path(
        &mut self,
        value: &str,
        create: bool,
        guard: &mut impl FnMut() -> Result<()>,
    ) -> Result<PathBuf> {
        relative(value)?;
        let target = self.root.join(value);
        let parent = target.parent().ok_or("Missing deployment parent.")?;
        let mut current = self.root.clone();
        for part in parent
            .strip_prefix(&self.root)
            .map_err(|e| e.to_string())?
            .components()
        {
            current.push(part);
            match fs::symlink_metadata(&current) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => {
                    guard()?;
                    fs::create_dir(&current).map_err(|e| e.to_string())?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(target),
                Err(e) => return Err(e.to_string()),
                Ok(_) => {}
            }
            if self.pinned.insert(current.clone()) {
                self.pins.push(pin(&current)?);
            } else {
                regular_metadata(&current, true)?;
            }
        }
        Ok(target)
    }
    fn digest(
        &mut self,
        path: &str,
        guard: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<String>> {
        content::digest_file(&self.path(path, false, guard)?)
    }
    fn replace(
        &mut self,
        path: &str,
        expected: Option<&String>,
        bytes: Option<Box<dyn Read>>,
        guard: &mut impl FnMut() -> Result<()>,
        event: &mut impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        guard()?;
        let destination = self.path(path, bytes.is_some(), guard)?;
        if self.digest(path, guard)?.as_ref() != expected {
            return Err(format!(
                "Repair required: {path} changed outside this operation. Files and backups were retained."
            ));
        }
        if let Some(mut bytes) = bytes {
            let temp = destination.with_file_name(format!(".starframe-{}.tmp", Uuid::new_v4()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|e| e.to_string())?;
            let size = std::io::copy(&mut (&mut bytes).take(content::FILE_LIMIT + 1), &mut file)
                .map_err(|e| e.to_string())?;
            if size > content::FILE_LIMIT {
                return Err("Deployment content exceeds its size limit.".into());
            }
            file.sync_all().map_err(|e| e.to_string())?;
            drop(file);
            event("temporary-written")?;
            guard()?;
            if self.digest(path, guard)?.as_ref() != expected {
                return Err(format!(
                    "Repair required: {path} changed before replacement. Temporary content was retained."
                ));
            }
            fs::rename(temp, destination).map_err(|e| e.to_string())?;
        } else {
            guard()?;
            fs::remove_file(destination).map_err(|e| e.to_string())?;
        }
        event("file-changed")?;
        Ok(())
    }
}
fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(_) => {}
    }
    if regular_metadata(path, false)?.len() > MAX_FILE {
        return Err(format!("Bootstrap file is too large: {}", path.display()));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("Bootstrap file grew beyond its size limit.".into());
    }
    Ok(Some(bytes))
}
fn changed(before: &Files, after: &Files) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .filter(|p| before.get(*p) != after.get(*p))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn rollback(
    store: &mut Storage,
    engine: &mut Engine,
    record: &mut Record,
    guard: &mut impl FnMut() -> Result<()>,
    event: &mut impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let Some(pending) = &record.pending else {
        return Ok(());
    };
    for path in changed(&record.owned, &pending.next).into_iter().rev() {
        let before = record.owned.get(&path);
        let after = pending.next.get(&path);
        let current = engine.digest(&path, guard)?;
        if current.as_ref() == before {
            continue;
        }
        if current.as_ref() != after {
            return Err(format!(
                "Repair required: {path} has unknown content. Recovery retained it and the backup."
            ));
        }
        let bytes = before
            .map(|hash| content::reader(store, hash))
            .transpose()?;
        engine.replace(&path, after, bytes, guard, event)?;
    }
    for (path, expected) in &record.owned {
        if engine.digest(path, guard)?.as_ref() != Some(expected) {
            return Err(format!(
                "Repair required: previous file {path} could not be verified."
            ));
        }
    }
    record.pending = None;
    save(store, &engine.root, record, &[])?;
    event("recovery-committed")?;
    Ok(())
}
fn apply(
    store: &mut Storage,
    engine: &mut Engine,
    files: Vec<(String, Vec<u8>)>,
    guard: &mut impl FnMut() -> Result<()>,
    event: &mut impl FnMut(&str) -> Result<()>,
) -> Result<usize> {
    if files.iter().map(|(_, bytes)| bytes.len()).sum::<usize>() > MAX_TOTAL {
        return Err("Oversized bootstrap payload.".into());
    }
    apply_sources(
        store,
        engine,
        files
            .into_iter()
            .map(|(path, bytes)| (path, Source::Bytes(bytes)))
            .collect(),
        guard,
        event,
    )
}

fn apply_sources(
    store: &mut Storage,
    engine: &mut Engine,
    files: Vec<(String, Source)>,
    guard: &mut impl FnMut() -> Result<()>,
    event: &mut impl FnMut(&str) -> Result<()>,
) -> Result<usize> {
    guard()?;
    let mut record = load(store, &engine.root)?;
    if record.pending.is_some() {
        rollback(store, engine, &mut record, guard, event)?;
    }
    let mut desired = Files::new();
    let mut blobs = Vec::new();
    let mut total = 0u64;
    for (path, source) in files {
        guard()?;
        let (digest, size) = match source {
            Source::Bytes(bytes) => {
                if bytes.len() as u64 > MAX_FILE {
                    return Err("Oversized bootstrap file.".into());
                }
                let digest = hash(&bytes);
                let size = bytes.len() as u64;
                blobs.push((digest.clone(), bytes));
                (digest, size)
            }
            Source::File { path, hash, size } => {
                content::retain_file(store, &path, &hash, size)?;
                (hash, size)
            }
        };
        total = total.checked_add(size).ok_or("Deployment size overflow.")?;
        if total > content::TOTAL_LIMIT || desired.insert(path, digest).is_some() {
            return Err("Invalid or oversized deployment payload.".into());
        }
    }
    validate_files(&desired)?;
    let mut next = Files::new();
    let mut borrowed = Files::new();
    for (path, expected) in &record.owned {
        let current = engine.path(path, false, guard)?;
        if content::digest_file(&current)?.as_ref() != Some(expected) {
            return Err(format!(
                "Repair required: owned file {path} changed or is missing. It was retained."
            ));
        }
        let size = regular_metadata(&current, false)?.len();
        if size <= MAX_FILE
            && !store
                .package_root()
                .join("deployment-content")
                .join(expected)
                .exists()
        {
            blobs.push((expected.clone(), read(&current)?.unwrap()));
        } else {
            content::retain_file(store, &current, expected, size)?;
        }
    }
    for (path, expected) in desired {
        if record.owned.contains_key(&path) {
            next.insert(path, expected);
            continue;
        }
        match engine.digest(&path, guard)? {
            None => {
                next.insert(path, expected);
            }
            Some(actual) if actual == expected => {
                borrowed.insert(path, expected);
            }
            Some(_) => {
                return Err(format!(
                    "Unowned file conflict: {path}. Import/adopt it explicitly before replacement."
                ));
            }
        }
    }
    crate::space::require(&engine.root, total)?;
    crate::space::require(
        store.package_root(),
        blobs
            .iter()
            .map(|(_, bytes)| bytes.len() as u64)
            .sum::<u64>()
            .saturating_mul(3),
    )?;
    record.pending = Some(Journal {
        id: Uuid::new_v4(),
        next,
        borrowed,
    });
    save(store, &engine.root, &record, &blobs)?;
    event("journal-committed")?;
    let outcome = (|| {
        crate::space::require(&engine.root, total)?;
        let pending = record.pending.as_ref().unwrap();
        for path in changed(&record.owned, &pending.next) {
            let bytes = pending
                .next
                .get(&path)
                .map(|hash| content::reader(store, hash))
                .transpose()?;
            engine.replace(&path, record.owned.get(&path), bytes, guard, event)?;
        }
        for (path, expected) in pending.next.iter().chain(pending.borrowed.iter()) {
            if engine.digest(path, guard)?.as_ref() != Some(expected) {
                return Err(format!("Deployment verification failed: {path}"));
            }
        }
        guard()?;
        event("files-verified")?;
        Ok(())
    })();
    if let Err(error) = outcome {
        return match rollback(store, engine, &mut record, guard, event) {
            Ok(()) => Err(format!("{error} Previous deployment restored.")),
            Err(recovery) => Err(format!("{error} {recovery} Recovery remains pending.")),
        };
    }
    record.owned = record.pending.take().unwrap().next;
    save(store, &engine.root, &record, &[])?;
    event("deployment-committed")?;
    Ok(record.owned.len())
}

#[derive(Deserialize)]
struct BootstrapSource {
    version: String,
    files: Vec<SourceFile>,
}
#[derive(Deserialize)]
struct SourceFile {
    path: String,
    sha256: String,
}
fn payload(path: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let source: BootstrapSource =
        serde_json::from_str(include_str!("../../runtime/bootstrap.json"))
            .map_err(|e| e.to_string())?;
    if source.version != "5.4.23.5" {
        return Err("Unsupported bootstrap release.".into());
    }
    let mut files = Vec::new();
    for entry in source.files.into_iter().filter(|v| {
        v.path.ends_with(".dll") || v.path == "doorstop_config.ini" || v.path == ".doorstop_version"
    }) {
        // Prepared package paths are fixed by the checked-in official release inventory.
        let mut current = path.to_path_buf();
        regular_metadata(&current, true)?;
        for part in Path::new(&entry.path).parent().unwrap().components() {
            current.push(part);
            regular_metadata(&current, true)?;
        }
        let bytes = read(&path.join(&entry.path))?.ok_or("Prepared bootstrap file is missing.")?;
        if hash(&bytes) != entry.sha256 {
            return Err(format!("Bootstrap hash mismatch: {}", entry.path));
        }
        files.push((entry.path, bytes));
    }
    files.push((
        "Starframe/notices/BepInEx-5.4.23.5.txt".into(),
        include_bytes!("../../docs/notices/BepInEx-5.4.23.5.txt").to_vec(),
    ));
    Ok(files)
}
pub(crate) fn guard_game(game: &game::Installation) -> Result<()> {
    let current = game::inspect(Path::new(&game.path))?;
    if current.executable != game.executable || current.build != game.build {
        return Err("Game installation changed; prepare again.".into());
    }
    #[cfg(windows)]
    {
        if game::classify(
            Path::new(&game.executable),
            crate::windows_game::processes(),
        ) == game::Running::Stopped
        {
            return Ok(());
        }
    }
    Err("Game is running or its process state is unknown. File changes are deferred.".into())
}
fn engine_root(game: &game::Installation) -> Result<PathBuf> {
    let root = Path::new(&game.executable)
        .parent()
        .ok_or("Missing engine directory")?;
    fs::canonicalize(root).map_err(|e| e.to_string())
}

/// Prepare the pinned BepInEx files. Mod activation and readiness are separate operations.
pub fn install(store: &mut Storage, game: &game::Installation, prepared: &Path) -> Result<usize> {
    let files = payload(prepared)?;
    let mut guard = || guard_game(game);
    let mut engine = Engine::open(&engine_root(game)?, &mut guard)?;
    apply(store, &mut engine, files, &mut guard, &mut |_| Ok(()))
}

/// Restore an interrupted operation when the observed game is closed.
pub fn recover(store: &mut Storage, game: &game::Installation) -> Result<bool> {
    let root = engine_root(game)?;
    let mut record = load(store, &root)?;
    if record.pending.is_none() {
        return Ok(false);
    }
    let mut guard = || guard_game(game);
    let mut engine = Engine::open(&root, &mut guard)?;
    rollback(store, &mut engine, &mut record, &mut guard, &mut |_| Ok(()))?;
    Ok(true)
}

/// Remove only recorded files, retaining shared loaders needed by external plugins.
pub fn remove(store: &mut Storage, game: &game::Installation) -> Result<usize> {
    let mut guard = || guard_game(game);
    let mut engine = Engine::open(&engine_root(game)?, &mut guard)?;
    let mut record = load(store, &engine.root)?;
    rollback(store, &mut engine, &mut record, &mut guard, &mut |_| Ok(()))?;
    for folder in ["BepInEx/plugins", "BepInEx/patchers"] {
        let folder = engine
            .path(&format!("{folder}/probe"), false, &mut guard)?
            .parent()
            .unwrap()
            .to_owned();
        if folder.exists() && external_dll(&folder, &engine.root, &record.owned, 0, &mut 0)? {
            return Err("External plugins still use BepInEx. Loader files were retained.".into());
        }
    }
    apply(store, &mut engine, Vec::new(), &mut guard, &mut |_| Ok(()))
}
fn external_dll(
    folder: &Path,
    root: &Path,
    owned: &Files,
    depth: usize,
    count: &mut usize,
) -> Result<bool> {
    regular_metadata(folder, true)?;
    if depth > 16 {
        return Err("Plugin scan is too deep; loader removal was deferred.".into());
    }
    for entry in fs::read_dir(folder).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        *count += 1;
        if *count > 4096 {
            return Err("Plugin scan exceeds its limit; loader removal was deferred.".into());
        }
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.is_dir() {
            if external_dll(&path, root, owned, depth + 1, count)? {
                return Ok(true);
            }
        } else {
            regular_metadata(&path, false)?;
            if path
                .extension()
                .is_some_and(|v| v.eq_ignore_ascii_case("dll"))
                && !owned.contains_key(
                    &path
                        .strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .replace('\\', "/"),
                )
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Development entry: deploy an explicitly selected, hash-inventoried runtime package.
/// Author downloads must pass catalog/package approval before using this boundary.
pub fn install_runtime(
    store: &mut Storage,
    game: &game::Installation,
    bootstrap: &Path,
    prepared: &Path,
) -> Result<usize> {
    let extra = runtime_payload(prepared)?;
    let mut files = payload(bootstrap)?;
    files.extend(extra);
    let mut guard = || guard_game(game);
    let mut engine = Engine::open(&engine_root(game)?, &mut guard)?;
    apply(store, &mut engine, files, &mut guard, &mut |_| Ok(()))
}

/// Prepare the desktop's current activation through the existing ownership journal.
pub fn prepare_desktop(
    store: &mut Storage,
    game: &game::Installation,
    resources: &Path,
    activation: &serde_json::Value,
    cancelled: &impl Fn() -> bool,
) -> Result<usize> {
    crate::runtime_contract::read(
        &serde_json::to_vec(activation).map_err(|e| e.to_string())?,
        "activation",
    )?;
    let mut files = runtime_payload(&resources.join("runtime"))?;
    let template = files
        .iter()
        .find(|(p, _)| p == "Starframe/activation.json")
        .unwrap();
    let template = crate::runtime_contract::read(&template.1, "activation")?;
    if !template["mods"].as_array().unwrap().is_empty() {
        return Err("The distributed runtime template must not contain third-party mods.".into());
    }
    files.retain(|(p, _)| p != "Starframe/activation.json");
    files.push((
        "Starframe/activation.json".into(),
        serde_json::to_vec(activation).map_err(|e| e.to_string())?,
    ));
    files.extend(payload(&resources.join("bootstrap"))?);
    let mut files: Vec<_> = files
        .into_iter()
        .map(|(path, bytes)| (path, Source::Bytes(bytes)))
        .collect();
    files.extend(crate::mods::payload(store, activation)?);
    let mut guard = || {
        if cancelled() {
            return Err("Starframe is closing. Preparation stopped at a safe boundary.".into());
        }
        guard_game(game)
    };
    let mut engine = Engine::open(&engine_root(game)?, &mut guard)?;
    apply_sources(store, &mut engine, files, &mut guard, &mut |_| Ok(()))
}

/// Read confirmed ownership without creating directories, locks or game files.
pub fn prepared_activation(
    store: &Storage,
    game: &game::Installation,
) -> Result<Option<serde_json::Value>> {
    let root = engine_root(game)?;
    let record = load(store, &root)?;
    if record.pending.is_some() {
        return Err("Repair required: a deployment remains interrupted. Wait for the game to close, then retry setup.".into());
    }
    let Some(expected) = record.owned.get("Starframe/activation.json") else {
        return Ok(None);
    };
    for (path, digest) in &record.owned {
        if content::digest_file(&root.join(path))?.as_ref() != Some(digest) {
            return Err(format!(
                "Repair required: {path} changed or is missing. Files were retained."
            ));
        }
    }
    let bytes =
        read(&root.join("Starframe/activation.json"))?.ok_or("Activation file is missing.")?;
    if hash(&bytes) != *expected {
        return Err("Activation changed while checking setup.".into());
    }
    crate::runtime_contract::read(&bytes, "activation").map(Some)
}

fn runtime_payload(prepared: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    let inventory = read(&prepared.join("runtime-package.json"))?
        .ok_or("Runtime package inventory is missing.")?;
    if inventory.len() > 1_048_576 {
        return Err("Runtime inventory is too large.".into());
    }
    let sources: Vec<SourceFile> = serde_json::from_slice(&inventory).map_err(|e| e.to_string())?;
    if sources.len() > 256 {
        return Err("Runtime inventory has too many files.".into());
    }
    let mut manifest = None;
    let mut hashes = Files::new();
    for source in sources {
        relative(&source.path)?;
        if !(source.path.starts_with("BepInEx/plugins/Starframe/")
            || source.path.starts_with("Starframe/mods/")
            || source.path == "Starframe/activation.json")
        {
            return Err(
                "Runtime inventory contains a path outside runtime deployment roots.".into(),
            );
        }
        let bytes =
            read(&prepared.join(&source.path))?.ok_or("Runtime package file is missing.")?;
        if hash(&bytes) != source.sha256
            || hashes.insert(source.path.clone(), source.sha256).is_some()
        {
            return Err("Runtime package hash/path mismatch.".into());
        }
        if source.path == "Starframe/activation.json" {
            manifest = Some(crate::runtime_contract::read(&bytes, "activation")?);
        }
        files.push((source.path, bytes));
    }
    validate_files(&hashes)?;
    if !hashes.contains_key("BepInEx/plugins/Starframe/Starframe.Bootstrap.dll")
        || !hashes.contains_key("BepInEx/plugins/Starframe/Starframe.Runtime.dll")
    {
        return Err("Runtime entry libraries are missing.".into());
    }
    let manifest = manifest.ok_or("Activation manifest is missing.")?;
    let mut expected = BTreeSet::new();
    for item in manifest["mods"].as_array().unwrap() {
        let root = item["root"].as_str().unwrap();
        if !root.starts_with("mods/") {
            return Err("Mod roots must be below Starframe/mods.".into());
        }
        for file in item["files"].as_array().unwrap() {
            let path = format!("Starframe/{root}/{}", file["path"].as_str().unwrap());
            if hashes.get(&path).map(String::as_str) != file["sha256"].as_str() {
                return Err("Activation inventory differs from prepared files.".into());
            }
            expected.insert(path);
        }
    }
    if hashes
        .keys()
        .any(|path| path.starts_with("Starframe/mods/") && !expected.contains(path))
    {
        return Err("Unlisted mod payload in runtime package.".into());
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, process::Command};
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Storage, Engine) {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("engine")).unwrap();
        let storage = Storage::open(&temp.path().join("data")).unwrap();
        let engine = Engine::open(&temp.path().join("engine"), &mut || Ok(())).unwrap();
        (temp, storage, engine)
    }
    fn files(version: u8) -> Vec<(String, Vec<u8>)> {
        vec![
            ("winhttp.dll".into(), vec![version; 100]),
            ("BepInEx/core/runtime.dll".into(), vec![version; 40]),
        ]
    }
    fn verify(store: &Storage, root: &Path) {
        let record = load(store, root).unwrap();
        assert!(record.pending.is_none());
        for (path, expected) in &record.owned {
            assert_eq!(&hash(&read(&root.join(path)).unwrap().unwrap()), expected);
        }
    }
    #[test]
    fn install_update_restore_backup_and_remove_preserve_unowned_files() {
        let (temp, mut store, mut engine) = fixture();
        fs::write(engine.root.join("game-original.txt"), b"original").unwrap();
        apply(
            &mut store,
            &mut engine,
            files(1),
            &mut || Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
        apply(
            &mut store,
            &mut engine,
            files(2),
            &mut || Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            store.deployment_blob(&hash(&[1; 100])).unwrap(),
            vec![1; 100]
        );
        let backup = store.backup().unwrap();
        let restored = Storage::restore_into(&backup, &temp.path().join("restored")).unwrap();
        verify(&restored, &engine.root);
        assert_eq!(
            restored.deployment_blob(&hash(&[1; 100])).unwrap(),
            vec![1; 100]
        );
        apply(
            &mut store,
            &mut engine,
            Vec::new(),
            &mut || Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            fs::read(engine.root.join("game-original.txt")).unwrap(),
            b"original"
        );
        assert!(!engine.root.join("winhttp.dll").exists());
        verify(&store, &engine.root);
    }
    #[test]
    fn matching_external_files_are_borrowed_never_adopted() {
        let (_temp, mut store, mut engine) = fixture();
        fs::write(engine.root.join("winhttp.dll"), vec![1; 100]).unwrap();
        assert_eq!(
            apply(
                &mut store,
                &mut engine,
                files(1),
                &mut || Ok(()),
                &mut |_| Ok(())
            )
            .unwrap(),
            1
        );
        apply(
            &mut store,
            &mut engine,
            Vec::new(),
            &mut || Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            fs::read(engine.root.join("winhttp.dll")).unwrap(),
            vec![1; 100]
        );
    }
    #[test]
    fn conflicts_fail_before_mutation_and_changed_owned_files_are_retained() {
        let (_temp, mut store, mut engine) = fixture();
        fs::write(engine.root.join("winhttp.dll"), b"foreign loader").unwrap();
        assert!(
            apply(
                &mut store,
                &mut engine,
                files(1),
                &mut || Ok(()),
                &mut |_| Ok(())
            )
            .unwrap_err()
            .contains("Unowned")
        );
        assert!(!engine.root.join("BepInEx").exists());
        fs::remove_file(engine.root.join("winhttp.dll")).unwrap();
        apply(
            &mut store,
            &mut engine,
            files(1),
            &mut || Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
        fs::write(engine.root.join("winhttp.dll"), b"user edit").unwrap();
        assert!(
            apply(
                &mut store,
                &mut engine,
                Vec::new(),
                &mut || Ok(()),
                &mut |_| Ok(())
            )
            .is_err()
        );
        assert_eq!(
            fs::read(engine.root.join("winhttp.dll")).unwrap(),
            b"user edit"
        );
    }
    #[test]
    fn game_start_defers_rollback_then_closed_game_can_recover() {
        let (_temp, mut store, mut engine) = fixture();
        let running = Cell::new(false);
        let mut guard = || {
            if running.get() {
                Err("Game running or unknown".into())
            } else {
                Ok(())
            }
        };
        let mut event = |name: &str| {
            if name == "file-changed" {
                running.set(true);
            }
            Ok(())
        };
        assert!(apply(&mut store, &mut engine, files(1), &mut guard, &mut event).is_err());
        let mut record = load(&store, &engine.root).unwrap();
        assert!(record.pending.is_some());
        let before = read(&engine.root.join("BepInEx/core/runtime.dll")).unwrap();
        assert!(
            rollback(
                &mut store,
                &mut engine,
                &mut record,
                &mut guard,
                &mut |_| Ok(())
            )
            .is_err()
        );
        assert_eq!(
            read(&engine.root.join("BepInEx/core/runtime.dll")).unwrap(),
            before
        );
        running.set(false);
        rollback(
            &mut store,
            &mut engine,
            &mut record,
            &mut guard,
            &mut |_| Ok(()),
        )
        .unwrap();
        verify(&store, &engine.root);
        assert!(!engine.root.join("BepInEx/core/runtime.dll").exists());
    }
    #[test]
    fn recovery_preserves_external_changes_and_can_retry_after_manual_resolution() {
        let (_temp, mut store, mut engine) = fixture();
        let running = Cell::new(false);
        let mut guard = || {
            if running.get() {
                Err("deferred".into())
            } else {
                Ok(())
            }
        };
        assert!(
            apply(&mut store, &mut engine, files(1), &mut guard, &mut |name| {
                if name == "file-changed" {
                    running.set(true);
                }
                Ok(())
            })
            .is_err()
        );
        running.set(false);
        let target = engine.root.join("BepInEx/core/runtime.dll");
        fs::write(&target, b"external edit").unwrap();
        let mut record = load(&store, &engine.root).unwrap();
        assert!(
            rollback(
                &mut store,
                &mut engine,
                &mut record,
                &mut guard,
                &mut |_| Ok(())
            )
            .unwrap_err()
            .contains("unknown content")
        );
        assert_eq!(fs::read(&target).unwrap(), b"external edit");
        fs::write(&target, vec![1; 40]).unwrap();
        rollback(
            &mut store,
            &mut engine,
            &mut record,
            &mut guard,
            &mut |_| Ok(()),
        )
        .unwrap();
        verify(&store, &engine.root);
    }
    #[test]
    fn installation_lock_and_unknown_game_state_prevent_new_writes() {
        let (_temp, _store, engine) = fixture();
        assert!(Engine::open(&engine.root, &mut || Ok(())).is_err());
        let root = engine.root.clone();
        drop(engine);
        assert!(Engine::open(&root, &mut || Err("unknown process".into())).is_err());
        assert!(Engine::open(&root, &mut || Ok(())).is_ok());
    }
    #[test]
    fn invalid_paths_and_case_aliases_never_become_a_plan() {
        let (_temp, mut store, mut engine) = fixture();
        for name in [
            "../outside.dll",
            "CON.dll",
            "C:/outside.dll",
            "a\\b.dll",
            "BepInEx/core/../outside.dll",
        ] {
            assert!(
                apply(
                    &mut store,
                    &mut engine,
                    vec![(name.into(), vec![1])],
                    &mut || Ok(()),
                    &mut |_| Ok(())
                )
                .is_err()
            );
        }
        assert!(
            apply(
                &mut store,
                &mut engine,
                vec![("a.dll".into(), vec![1]), ("A.dll".into(), vec![2])],
                &mut || Ok(()),
                &mut |_| Ok(())
            )
            .is_err()
        );
        verify(&store, &engine.root);
    }
    #[test]
    fn external_plugins_require_retaining_the_shared_loader() {
        let (_temp, _store, engine) = fixture();
        let plugins = engine.root.join("BepInEx/plugins");
        fs::create_dir_all(&plugins).unwrap();
        fs::write(plugins.join("Other.DLL"), b"external").unwrap();
        assert!(external_dll(&plugins, &engine.root, &Files::new(), 0, &mut 0).unwrap());
    }
    #[cfg(windows)]
    #[test]
    fn locked_destination_failure_restores_the_previous_deployment() {
        use std::os::windows::fs::OpenOptionsExt;
        let (_temp, mut store, mut engine) = fixture();
        apply(
            &mut store,
            &mut engine,
            files(1),
            &mut || Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
        let held = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(engine.root.join("winhttp.dll"))
            .unwrap();
        assert!(
            apply(
                &mut store,
                &mut engine,
                files(2),
                &mut || Ok(()),
                &mut |_| Ok(())
            )
            .is_err()
        );
        drop(held);
        verify(&store, &engine.root);
        assert_eq!(
            fs::read(engine.root.join("BepInEx/core/runtime.dll")).unwrap(),
            vec![1; 40]
        );
    }
    #[cfg(windows)]
    #[test]
    fn junction_parent_is_rejected_without_touching_the_destination() {
        use std::os::windows::process::CommandExt;
        let (temp, mut store, mut engine) = fixture();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let output = Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(engine.root.join("BepInEx"))
            .arg(&outside)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            apply(
                &mut store,
                &mut engine,
                files(1),
                &mut || Ok(()),
                &mut |_| Ok(())
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    }
    #[cfg(windows)]
    #[test]
    fn public_recovery_revalidates_game_and_does_nothing_without_a_journal() {
        let (temp, mut store, mut engine) = fixture();
        crate::game::tests::fixture(temp.path(), "engine");
        let game = game::inspect(temp.path()).unwrap();
        let running = Cell::new(false);
        assert!(
            apply(
                &mut store,
                &mut engine,
                files(1),
                &mut || if running.get() {
                    Err("deferred".into())
                } else {
                    Ok(())
                },
                &mut |name| {
                    if name == "file-changed" {
                        running.set(true);
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        let root = engine.root.clone();
        drop(engine);
        assert!(recover(&mut store, &game).unwrap());
        verify(&store, &root);
        assert!(!recover(&mut store, &game).unwrap());
    }

    fn child(root: &Path, action: &str, checkpoint: usize) {
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "deployment::tests::crash_worker", "--ignored"])
            .env("STARFRAME_DEPLOY_TEST_ROOT", root)
            .env("STARFRAME_DEPLOY_TEST_ACTION", action)
            .env("STARFRAME_DEPLOY_TEST_STOP", checkpoint.to_string())
            .output()
            .unwrap();
        assert_eq!(
            status.status.code(),
            Some(86),
            "{} {}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr)
        );
    }
    #[test]
    fn process_exit_at_each_persisted_boundary_recovers_install_update_and_remove() {
        for action in ["install", "update", "remove"] {
            for checkpoint in 1..=if action == "remove" { 5 } else { 7 } {
                let (temp, mut store, mut engine) = fixture();
                if action != "install" {
                    apply(
                        &mut store,
                        &mut engine,
                        files(1),
                        &mut || Ok(()),
                        &mut |_| Ok(()),
                    )
                    .unwrap();
                }
                let root = engine.root.clone();
                drop(engine);
                drop(store);
                child(temp.path(), action, checkpoint);
                let mut store = Storage::open(&temp.path().join("data")).unwrap();
                let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
                let mut record = load(&store, &root).unwrap();
                rollback(
                    &mut store,
                    &mut engine,
                    &mut record,
                    &mut || Ok(()),
                    &mut |_| Ok(()),
                )
                .unwrap();
                verify(&store, &root);
                let committed = checkpoint == if action == "remove" { 5 } else { 7 };
                let expected = match (action, committed) {
                    ("install", false) | ("remove", true) => None,
                    ("update", true) => Some(vec![2; 100]),
                    _ => Some(vec![1; 100]),
                };
                assert_eq!(
                    read(&root.join("winhttp.dll")).unwrap(),
                    expected,
                    "{action} checkpoint {checkpoint}"
                );
            }
        }
    }
    #[test]
    fn process_exit_during_rollback_can_recover_again() {
        for checkpoint in 1..=3 {
            let (temp, mut store, mut engine) = fixture();
            apply(
                &mut store,
                &mut engine,
                files(1),
                &mut || Ok(()),
                &mut |_| Ok(()),
            )
            .unwrap();
            let root = engine.root.clone();
            drop(engine);
            drop(store);
            child(temp.path(), "update", 3);
            child(temp.path(), "recover", checkpoint);
            let mut store = Storage::open(&temp.path().join("data")).unwrap();
            let mut engine = Engine::open(&root, &mut || Ok(())).unwrap();
            let mut record = load(&store, &root).unwrap();
            rollback(
                &mut store,
                &mut engine,
                &mut record,
                &mut || Ok(()),
                &mut |_| Ok(()),
            )
            .unwrap();
            verify(&store, &root);
            assert_eq!(
                fs::read(root.join("BepInEx/core/runtime.dll")).unwrap(),
                vec![1; 40]
            );
        }
    }
    #[test]
    #[ignore = "worker exits at durable deployment boundaries; invoked by parent tests"]
    fn crash_worker() {
        let root = PathBuf::from(std::env::var_os("STARFRAME_DEPLOY_TEST_ROOT").unwrap());
        let action = std::env::var("STARFRAME_DEPLOY_TEST_ACTION").unwrap();
        let stop: usize = std::env::var("STARFRAME_DEPLOY_TEST_STOP")
            .unwrap()
            .parse()
            .unwrap();
        let mut count = 0;
        let mut event = |_: &str| {
            count += 1;
            if count == stop {
                std::process::exit(86);
            }
            Ok(())
        };
        let mut store = Storage::open(&root.join("data")).unwrap();
        let mut engine = Engine::open(&root.join("engine"), &mut || Ok(())).unwrap();
        if action == "recover" {
            let mut record = load(&store, &engine.root).unwrap();
            rollback(
                &mut store,
                &mut engine,
                &mut record,
                &mut || Ok(()),
                &mut event,
            )
            .unwrap();
        } else {
            apply(
                &mut store,
                &mut engine,
                if action == "remove" {
                    Vec::new()
                } else {
                    files(if action == "update" { 2 } else { 1 })
                },
                &mut || Ok(()),
                &mut event,
            )
            .unwrap();
        }
    }
}

#[cfg(test)]
#[test]
fn prepared_runtime_inventory_rejects_unlisted_content_and_hash_changes() {
    let root = std::env::temp_dir().join(format!("starframe-runtime-package-{}", Uuid::new_v4()));
    let plugin = root.join("BepInEx/plugins/Starframe");
    fs::create_dir_all(&plugin).unwrap();
    fs::create_dir_all(root.join("Starframe")).unwrap();
    let mut inventory = Vec::new();
    for name in ["Starframe.Bootstrap.dll", "Starframe.Runtime.dll"] {
        let path = format!("BepInEx/plugins/Starframe/{name}");
        fs::write(root.join(&path), b"fixture").unwrap();
        inventory.push(serde_json::json!({"path":path,"sha256":hash(b"fixture")}));
    }
    let manifest = include_bytes!("../../contracts/fixtures/activation-empty.json");
    fs::write(root.join("Starframe/activation.json"), manifest).unwrap();
    inventory.push(serde_json::json!({"path":"Starframe/activation.json","sha256":hash(manifest)}));
    let index = root.join("runtime-package.json");
    fs::write(&index, serde_json::to_vec(&inventory).unwrap()).unwrap();
    assert_eq!(runtime_payload(&root).unwrap().len(), 3);
    fs::write(plugin.join("Starframe.Runtime.dll"), b"changed").unwrap();
    assert!(runtime_payload(&root).unwrap_err().contains("hash/path"));
    fs::write(plugin.join("Starframe.Runtime.dll"), b"fixture").unwrap();
    fs::create_dir_all(root.join("Starframe/mods/disabled")).unwrap();
    fs::write(root.join("Starframe/mods/disabled/extra.dll"), b"fixture").unwrap();
    inventory.push(
        serde_json::json!({"path":"Starframe/mods/disabled/extra.dll","sha256":hash(b"fixture")}),
    );
    fs::write(&index, serde_json::to_vec(&inventory).unwrap()).unwrap();
    assert!(runtime_payload(&root).unwrap_err().contains("Unlisted"));
    fs::remove_dir_all(root).unwrap();
}
