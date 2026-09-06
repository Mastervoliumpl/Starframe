use std::{mem::size_of, path::PathBuf};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_NO_MORE_FILES, HANDLE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW},
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
    },
    core::{PWSTR, w},
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn steam_root() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if std::env::var_os("STARFRAME_TEST_DATA_DIR").is_some() {
        return std::env::var_os("STARFRAME_TEST_STEAM_ROOT")
            .map(PathBuf::from)
            .ok_or("Steam discovery is isolated in this test run.".into());
    }
    let mut data = [0u16; 32768];
    let mut bytes = size_of_val(&data) as u32;
    // The registry API writes at most the supplied buffer size; the handle is predefined.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Valve\\Steam"),
            w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
        .ok()
        .map_err(|_| "Steam was not found. Choose the game folder.")?;
    }
    let end = data.iter().position(|c| *c == 0).unwrap_or(data.len());
    let root =
        PathBuf::from(String::from_utf16(&data[..end]).map_err(|_| "Steam path is invalid.")?);
    if !root.is_absolute() {
        return Err("Steam path is not absolute.".into());
    }
    Ok(root)
}

pub fn processes() -> Result<Vec<Option<PathBuf>>, String> {
    // Snapshot and process handles are owned here and closed on every return path.
    unsafe {
        let snapshot =
            Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| e.to_string())?);
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        Process32FirstW(snapshot.0, &mut entry).map_err(|e| e.to_string())?;
        let mut paths = vec![];
        loop {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..end])
                .eq_ignore_ascii_case("Sanctuary.exe")
            {
                let path = OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    entry.th32ProcessID,
                )
                .ok()
                .and_then(|raw| {
                    let process = Handle(raw);
                    let mut path = [0u16; 32768];
                    let mut len = path.len() as u32;
                    QueryFullProcessImageNameW(
                        process.0,
                        PROCESS_NAME_WIN32,
                        PWSTR(path.as_mut_ptr()),
                        &mut len,
                    )
                    .ok()?;
                    PathBuf::from(String::from_utf16(&path[..len as usize]).ok()?)
                        .canonicalize()
                        .ok()
                });
                paths.push(path);
            }
            if let Err(error) = Process32NextW(snapshot.0, &mut entry) {
                if error.code() != ERROR_NO_MORE_FILES.to_hresult() {
                    return Err(error.to_string());
                }
                break;
            }
        }
        Ok(paths)
    }
}
