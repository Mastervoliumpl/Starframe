use std::path::Path;

pub fn require(path: &Path, bytes: u64) -> Result<(), String> {
    check(available(path)?, bytes)
}

fn check(available: u64, bytes: u64) -> Result<(), String> {
    let needed = bytes
        .checked_add(64 * 1024 * 1024)
        .ok_or("Space estimate overflow.")?;
    if available < needed {
        return Err(format!(
            "Not enough disk space: need about {} MiB including working space; {} MiB available. Free space and retry.",
            needed.div_ceil(1024 * 1024),
            available / (1024 * 1024)
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub fn available(path: &Path) -> Result<u64, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{Win32::Storage::FileSystem::GetDiskFreeSpaceExW, core::PCWSTR};
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0;
    // The API writes one u64; the UTF-16 path remains alive for the call.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(path.as_ptr()), Some(&mut free), None, None) }
        .map_err(|e| format!("Could not check disk space: {e}"))?;
    Ok(free)
}

#[cfg(not(windows))]
pub fn available(_: &Path) -> Result<u64, String> {
    Err("Disk-space checks require the supported Windows platform.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn estimates_keep_working_margin_and_reject_overflow() {
        assert!(check(128 * 1024 * 1024, 64 * 1024 * 1024).is_ok());
        assert!(check(128 * 1024 * 1024 - 1, 64 * 1024 * 1024).is_err());
        assert!(check(u64::MAX, u64::MAX).is_err());
        #[cfg(windows)]
        assert!(available(&std::env::temp_dir()).unwrap() > 0);
    }
}
