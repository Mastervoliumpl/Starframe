use std::path::Path;

#[cfg(windows)]
use windows::Win32::{
    Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Storage::FileSystem::*,
    System::Threading::WaitForSingleObject,
};

pub(super) struct Changes {
    #[cfg(windows)]
    handle: HANDLE,
}

impl Changes {
    pub(super) fn open(source: &Path) -> Result<Self, String> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let folder = source.is_dir();
            let directory = if folder {
                source
            } else {
                source.parent().ok_or("Source has no parent.")?
            };
            let _pins = directory
                .ancestors()
                .map(crate::packages::pin)
                .collect::<Result<Vec<_>, _>>()?;
            let path = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let handle = unsafe {
                FindFirstChangeNotificationW(
                    windows::core::PCWSTR(wide.as_ptr()),
                    folder,
                    FILE_NOTIFY_CHANGE_FILE_NAME
                        | FILE_NOTIFY_CHANGE_DIR_NAME
                        | FILE_NOTIFY_CHANGE_SIZE
                        | FILE_NOTIFY_CHANGE_LAST_WRITE,
                )
            }
            .map_err(|e| e.to_string())?;
            Ok(Self { handle })
        }
        #[cfg(not(windows))]
        {
            let _ = source;
            Err("Native file notifications are unavailable.".into())
        }
    }

    pub(super) fn changed(&mut self) -> Result<bool, String> {
        #[cfg(windows)]
        {
            match unsafe { WaitForSingleObject(self.handle, 0) } {
                WAIT_OBJECT_0 => {
                    unsafe { FindNextChangeNotification(self.handle) }
                        .map_err(|e| e.to_string())?;
                    Ok(true)
                }
                WAIT_TIMEOUT => Ok(false),
                _ => Err("Source notifications need to be reopened.".into()),
            }
        }
        #[cfg(not(windows))]
        {
            Ok(false)
        }
    }
}

impl Drop for Changes {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = FindCloseChangeNotification(self.handle);
        }
    }
}
