use starframe::catalog::{Catalog, MAX_BYTES};
use std::{io::Read, process::Command};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 && !(args.len() == 3 && args[1] == "--git-base") {
        return Err("Usage: check_catalog catalog/releases.json [--git-base REF]".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&args[0])?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let catalog = Catalog::read(&bytes)?;
    if args.len() == 3 {
        let tree = Command::new("git")
            .args(["ls-tree", &args[2], "--", "catalog/releases.json"])
            .output()?;
        if !tree.status.success() {
            return Err("Could not read the catalog base commit.".into());
        }
        if !tree.stdout.is_empty() {
            let previous = Command::new("git")
                .args(["show", &format!("{}:catalog/releases.json", args[2])])
                .output()?;
            if !previous.status.success() {
                return Err("Could not read the previous catalog.".into());
            }
            catalog.accepts_after(&Catalog::read(&previous.stdout)?)?;
        }
    }
    println!(
        "Catalog revision {}: {} mods, {} releases validated.",
        catalog.catalog_revision,
        catalog.mods.len(),
        catalog.releases().count()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
