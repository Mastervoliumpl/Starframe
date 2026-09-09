use starframe::catalog::{Catalog, MAX_BYTES, advisories::Advisories};
use std::{io::Read, path::Path, process::Command};

fn read(path: &Path, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn previous(reference: &str, path: &str) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
    let tree = Command::new("git")
        .args(["ls-tree", reference, "--", path])
        .output()?;
    if !tree.status.success() {
        return Err("Could not read the catalog base commit.".into());
    }
    if tree.stdout.is_empty() {
        return Ok(None);
    }
    let previous = Command::new("git")
        .args(["show", &format!("{reference}:{path}")])
        .output()?;
    if !previous.status.success() {
        return Err("Could not read the previous catalog data.".into());
    }
    Ok(Some(previous.stdout))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1
        && !(args.len() == 3 && matches!(args[1].as_str(), "--git-base" | "--previous-directory"))
    {
        return Err("Usage: check_catalog catalog/releases.json [--git-base REF | --previous-directory DIR]".into());
    }
    let path = Path::new(&args[0]);
    let catalog = Catalog::read(&read(path, MAX_BYTES)?)?;
    let advisories = Advisories::read(&read(
        &path.with_file_name("advisories.json"),
        starframe::catalog::advisories::MAX_BYTES,
    )?)?;
    if args.len() == 3 && args[1] == "--git-base" {
        if let Some(bytes) = previous(&args[2], "catalog/releases.json")? {
            catalog.accepts_after(&Catalog::read(&bytes)?)?;
        }
        if let Some(bytes) = previous(&args[2], "catalog/advisories.json")? {
            advisories.accepts_after(&Advisories::read(&bytes)?)?;
        }
    } else if args.len() == 3 {
        let previous = Path::new(&args[2]);
        catalog.accepts_after(&Catalog::read(&read(
            &previous.join("catalog.json"),
            MAX_BYTES,
        )?)?)?;
        advisories.accepts_after(&Advisories::read(&read(
            &previous.join("advisories.json"),
            starframe::catalog::advisories::MAX_BYTES,
        )?)?)?;
    }
    println!(
        "Catalog revision {}: {} mods, {} releases validated.",
        catalog.catalog_revision,
        catalog.mods.len(),
        catalog.releases().count()
    );
    println!(
        "Advisory revision {}: {} findings validated.",
        advisories.revision,
        advisories.advisories.len()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
