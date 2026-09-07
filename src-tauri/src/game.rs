use keyvalues_parser::{Obj, Vdf};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

// Steam identities are independent of build numbers and filesystem locations.
pub const EDITIONS: &[(u32, &str)] = &[
    (1699050, "Release"),
    (2375120, "Demo"),
    (4511930, "Playtest"),
];

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    pub id: String,
    pub path: String,
    pub executable: String,
    pub edition: String,
    pub build: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Running {
    Unknown,
    Running,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    pub launch: crate::launch::LaunchView,
    pub busy: bool,
    pub candidates: Vec<Installation>,
    pub selected: Option<Installation>,
    pub selected_path: Option<String>,
    pub running: Running,
    pub message: String,
    pub error: String,
}
impl Default for GameView {
    fn default() -> Self {
        Self {
            launch: Default::default(),
            busy: true,
            candidates: vec![],
            selected: None,
            selected_path: None,
            running: Running::Unknown,
            message: "Looking for Sanctuary…".into(),
            error: String::new(),
        }
    }
}

fn text(path: &Path) -> Result<String, String> {
    let mut content = String::new();
    fs::File::open(path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?
        .take(2 * 1024 * 1024 + 1)
        .read_to_string(&mut content)
        .map_err(|e| e.to_string())?;
    if content.len() > 2 * 1024 * 1024 {
        return Err("Game metadata exceeds the size limit.".into());
    }
    Ok(content)
}
fn document(content: &str, key: &str) -> Result<Vdf<'static>, String> {
    // Bound nesting before invoking the recursive VDF parser, including malformed input.
    let mut depth = 0usize;
    for byte in content.bytes() {
        if byte == b'{' {
            depth += 1;
        }
        if depth > 32 {
            return Err("Steam metadata is nested too deeply.".into());
        }
        if byte == b'}' {
            depth = depth.saturating_sub(1);
        }
    }
    let value = keyvalues_parser::parse(content).map_err(|_| "Steam metadata is malformed.")?;
    if value.key != key || value.value.get_obj().is_none() {
        return Err("Steam metadata has an unexpected format.".into());
    }
    if !value.bases.is_empty() {
        return Err("Steam metadata includes unsupported external files.".into());
    }
    Ok(value.into_owned().into())
}
fn field<'a>(obj: &'a Obj<'_>, key: &str) -> Result<&'a str, String> {
    match obj.get(key).map(Vec::as_slice) {
        Some([value]) => value
            .get_str()
            .ok_or_else(|| format!("Steam field {key} is not text.")),
        _ => Err(format!("Steam field {key} is missing or repeated.")),
    }
}
fn numeric(value: &str) -> bool {
    !value.is_empty() && value.len() <= 20 && value.bytes().all(|b| b.is_ascii_digit())
}

pub fn libraries(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = vec![root.to_path_buf()];
    let doc = document(
        &text(&root.join("steamapps/libraryfolders.vdf"))?,
        "libraryfolders",
    )?;
    for (index, entries) in doc.value.get_obj().unwrap().iter() {
        if !numeric(index) {
            continue;
        }
        if paths.len() > 64 {
            return Err("Steam lists too many libraries.".into());
        }
        if entries.len() != 1 {
            return Err("Steam repeats a library entry.".into());
        }
        let path = match entries[0].get_obj() {
            Some(obj) => field(obj, "path")?,
            None => entries[0]
                .get_str()
                .ok_or("Steam library path is invalid.")?,
        };
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err("Steam library path is not absolute.".into());
        }
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn manifest(path: &Path, app_id: u32) -> Result<(String, String), String> {
    let doc = document(&text(path)?, "AppState")?;
    let obj = doc.value.get_obj().unwrap();
    if field(obj, "appid")? != app_id.to_string() {
        return Err("Steam app identity does not match its manifest.".into());
    }
    let folder = field(obj, "installdir")?;
    if folder.is_empty()
        || folder == "."
        || folder == ".."
        || folder.chars().any(|c| "\\/:\0".contains(c))
    {
        return Err("Steam installation folder is invalid.".into());
    }
    let build = field(obj, "buildid")?;
    if !numeric(build) || build == "0" {
        return Err("Steam build identity is unavailable.".into());
    }
    let flags = field(obj, "StateFlags")?
        .parse::<u32>()
        .map_err(|_| "Steam installation state is invalid.")?;
    if flags & 4 == 0 || flags & (2 | 32 | 128 | 256 | 512 | 1024 | 2048 | 131072) != 0 {
        return Err("Steam has not finished installing or updating this game.".into());
    }
    Ok((folder.into(), build.into()))
}

pub fn discover(root: &Path) -> (Vec<Installation>, Vec<String>) {
    let mut errors = vec![];
    let libraries = match libraries(root) {
        Ok(paths) => paths,
        Err(e) => {
            errors.push(e);
            vec![root.to_path_buf()]
        }
    };
    let mut found = vec![];
    for library in libraries {
        for (id, _) in EDITIONS {
            let file = library.join(format!("steamapps/appmanifest_{id}.acf"));
            if !file.exists() {
                continue;
            }
            match manifest(&file, *id)
                .and_then(|(folder, _)| inspect(&library.join("steamapps/common").join(folder)))
            {
                Ok(item) => {
                    if !found
                        .iter()
                        .any(|other: &Installation| other.path == item.path)
                    {
                        found.push(item);
                    }
                }
                Err(e) => errors.push(format!("Steam app {id}: {e}")),
            }
        }
    }
    (found, errors)
}

fn contained(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|_| format!("Required game file is missing: {relative}"))?;
    if !path.starts_with(root) {
        return Err(format!(
            "Game file points outside the selected folder: {relative}"
        ));
    }
    Ok(path)
}
fn pe_x64(path: &Path) -> Result<(), String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header)
        .map_err(|_| "Game executable header is incomplete.")?;
    if &header[..2] != b"MZ" {
        return Err("Game executable is not a Windows PE file.".into());
    }
    let offset = u32::from_le_bytes(header[60..64].try_into().unwrap());
    if offset > 1024 * 1024 {
        return Err("Game executable header is invalid.".into());
    }
    file.seek(SeekFrom::Start(offset.into()))
        .map_err(|e| e.to_string())?;
    let mut pe = [0u8; 26];
    file.read_exact(&mut pe)
        .map_err(|_| "Game executable PE header is incomplete.")?;
    if &pe[..6] != b"PE\0\0\x64\x86" || pe[24..26] != [0x0b, 0x02] {
        return Err("The game requires a Windows x64 executable.".into());
    }
    Ok(())
}

pub fn inspect(folder: &Path) -> Result<Installation, String> {
    let mut root = folder
        .canonicalize()
        .map_err(|_| "The selected game folder is unavailable.")?;
    if root.file_name().is_some_and(|name| name == "engine") {
        root = root.parent().ok_or("Game folder has no parent.")?.into();
    }
    if !root.is_dir() {
        return Err("Select the game installation folder.".into());
    }
    let engine = if root.join("engine/Sanctuary.exe").is_file() {
        "engine/"
    } else {
        ""
    };
    let exe = contained(&root, &format!("{engine}Sanctuary.exe"))?;
    pe_x64(&exe)?;
    pe_x64(&contained(&root, &format!("{engine}UnityPlayer.dll"))?)?;
    let info = text(&contained(
        &root,
        &format!("{engine}Sanctuary_Data/app.info"),
    )?)?;
    let mut lines = info.lines();
    if lines.next() != Some("Enhearten Media PTY") || lines.next() != Some("Sanctuary") {
        return Err("The Unity product identity is not Sanctuary.".into());
    }
    if !contained(&root, &format!("{engine}Sanctuary_Data/Managed"))?.is_dir()
        || !contained(&root, &format!("{engine}MonoBleedingEdge"))?.is_dir()
    {
        return Err("The game does not have the supported Unity/Mono layout.".into());
    }
    let config = text(&contained(
        &root,
        &format!("{engine}Sanctuary_Data/boot.config"),
    )?)?;
    let guid = config
        .lines()
        .find_map(|line| line.strip_prefix("build-guid="))
        .filter(|v| v.len() == 32 && v.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("The Unity build identity is unavailable.")?;
    let mut edition = "Manual folder".to_owned();
    let mut build = format!("Unity {guid} · Steam build unknown");
    if let Some(common) = root
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "common"))
    {
        let steamapps = common.parent().ok_or("Steam library layout is invalid.")?;
        for (id, label) in EDITIONS {
            let file = steamapps.join(format!("appmanifest_{id}.acf"));
            if !file.exists() {
                continue;
            }
            let doc = document(&text(&file)?, "AppState")?;
            let folder = field(doc.value.get_obj().unwrap(), "installdir")?;
            if common.join(folder).canonicalize().ok().as_ref() == Some(&root) {
                let (_, number) = manifest(&file, *id)?;
                edition = format!("{label} · Steam {id}");
                build = format!("Steam {number} · Unity {guid}");
                break;
            }
        }
    }
    Ok(Installation {
        id: uuid::Uuid::new_v4().to_string(),
        path: root
            .to_str()
            .ok_or("Game path is not valid Unicode.")?
            .into(),
        executable: exe
            .to_str()
            .ok_or("Executable path is not valid Unicode.")?
            .into(),
        edition,
        build,
    })
}

pub fn classify(executable: &Path, observations: Result<Vec<Option<PathBuf>>, String>) -> Running {
    match observations {
        Err(_) => Running::Unknown,
        Ok(paths) => {
            if paths.iter().flatten().any(|p| p == executable) {
                Running::Running
            } else if paths.iter().any(Option::is_none) {
                Running::Unknown
            } else {
                Running::Stopped
            }
        }
    }
}

pub fn observation_expired(previous: std::time::SystemTime, now: std::time::SystemTime) -> bool {
    now.duration_since(previous)
        .map_or(true, |gap| gap > std::time::Duration::from_secs(10))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture(root: &Path, engine: &str) {
        let base = root.join(engine);
        fs::create_dir_all(base.join("Sanctuary_Data/Managed")).unwrap();
        fs::create_dir_all(base.join("MonoBleedingEdge")).unwrap();
        let mut pe = vec![0; 128];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..70].copy_from_slice(b"PE\0\0\x64\x86");
        pe[88..90].copy_from_slice(&[0x0b, 0x02]);
        fs::write(base.join("Sanctuary.exe"), &pe).unwrap();
        fs::write(base.join("UnityPlayer.dll"), &pe).unwrap();
        fs::write(
            base.join("Sanctuary_Data/app.info"),
            "Enhearten Media PTY\nSanctuary\n",
        )
        .unwrap();
        fs::write(
            base.join("Sanctuary_Data/boot.config"),
            "build-guid=0123456789abcdef0123456789abcdef\n",
        )
        .unwrap();
    }
    fn acf(folder: &str, id: u32, build: &str) -> String {
        format!(
            "\"AppState\" {{ \"appid\" \"{id}\" \"installdir\" \"{folder}\" \"buildid\" \"{build}\" \"StateFlags\" \"4\" }}"
        )
    }
    #[test]
    fn libraries_layouts_builds_and_bad_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("steam");
        let second = temp.path().join("additional library");
        let a = first.join("steamapps/common/Game");
        let b = second.join("steamapps/common/Demo");
        fixture(&a, "engine");
        fixture(&b, "");
        fs::write(
            first.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\" {{ \"1\" {{ \"path\" \"{}\" }} }}",
                second.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        fs::write(
            first.join("steamapps/appmanifest_4511930.acf"),
            acf("Game", 4511930, "25135612"),
        )
        .unwrap();
        fs::write(
            second.join("steamapps/appmanifest_2375120.acf"),
            acf("Demo", 2375120, "99999999"),
        )
        .unwrap();
        let (found, errors) = discover(&first);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(found.len(), 2);
        assert!(found[1].build.contains("99999999"));
        assert_eq!(inspect(&a.join("engine")).unwrap().path, found[0].path);
        let before = fs::read(a.join("engine/Sanctuary.exe")).unwrap();
        fs::write(
            first.join("steamapps/appmanifest_4511930.acf"),
            acf("Game", 4511930, "25135613"),
        )
        .unwrap();
        assert!(inspect(&a).unwrap().build.contains("25135613"));
        fs::write(
            first.join("steamapps/appmanifest_4511930.acf"),
            acf("../escape", 4511930, "1"),
        )
        .unwrap();
        let (found, errors) = discover(&first);
        assert_eq!(found.len(), 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(fs::read(a.join("engine/Sanctuary.exe")).unwrap(), before);
        assert!(inspect(temp.path()).is_err());
        fs::write(
            b.join("Sanctuary_Data/app.info"),
            "Someone else\nSanctuary\n",
        )
        .unwrap();
        assert!(inspect(&b).is_err());
        assert!(document("\"AppState\" {", "AppState").is_err());
        let repeated =
            document("\"AppState\" {\"appid\" \"1\" \"appid\" \"2\"}", "AppState").unwrap();
        assert!(field(repeated.value.get_obj().unwrap(), "appid").is_err());
    }
    #[test]
    fn observations_distinguish_path_uncertainty_and_resume() {
        let selected = PathBuf::from("selected/Sanctuary.exe");
        assert_eq!(
            classify(&selected, Ok(vec![Some(selected.clone()), None])),
            Running::Running
        );
        assert_eq!(
            classify(
                &selected,
                Ok(vec![Some(PathBuf::from("other/Sanctuary.exe"))])
            ),
            Running::Stopped
        );
        assert_eq!(classify(&selected, Ok(vec![None])), Running::Unknown);
        assert_eq!(
            classify(&selected, Err("Snapshot failed".into())),
            Running::Unknown
        );
        let now = std::time::SystemTime::now();
        assert!(!observation_expired(
            now,
            now + std::time::Duration::from_secs(2)
        ));
        assert!(observation_expired(
            now,
            now + std::time::Duration::from_secs(60)
        ));
        assert!(observation_expired(
            now,
            now - std::time::Duration::from_secs(1)
        ));
    }
    #[test]
    fn generated_game_contract_matches_rust() {
        use ts_rs::TS;
        let config = ts_rs::Config::default();
        let content = format!(
            "// Generated by the Rust contract test. Do not edit.\nexport {}\nexport {}\nexport {}\nexport {}\nexport {}\n",
            crate::launch::Phase::decl(&config),
            crate::launch::LaunchView::decl(&config),
            Installation::decl(&config),
            Running::decl(&config),
            GameView::decl(&config)
        );
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/generated/game.ts");
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            fs::write(&path, &content).unwrap();
        }
        assert_eq!(fs::read_to_string(path).unwrap(), content);
    }
}
