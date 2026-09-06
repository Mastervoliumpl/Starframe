use starframe::storage::Storage;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if paths.len() != 2 {
        return Err("Usage: restore_storage <backup-directory> <empty-destination>".into());
    }
    let restored = Storage::restore_into(&paths[0], &paths[1])?;
    let records = restored.load()?;
    println!(
        "Verified restore: {} library entries, {} collections. Destination: {}",
        records.library.len(),
        records.collections.len(),
        paths[1].display()
    );
    Ok(())
}
