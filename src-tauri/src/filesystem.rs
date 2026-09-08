use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};
type Result<T> = std::result::Result<T, String>;

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
pub(crate) fn read_file(path: &Path) -> Result<File> {
    regular_metadata(path, false)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Deny writes/deletes while verifying; open a reparse point itself, never its target.
        options.share_mode(1).custom_flags(0x00200000);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Package path is not an ordinary file.".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Package files cannot be reparse points.".into());
        }
    }
    Ok(file)
}
