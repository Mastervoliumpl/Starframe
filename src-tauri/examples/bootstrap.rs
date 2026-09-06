use starframe::{deployment, game, storage::Storage};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 3 {
        return Err("Usage: bootstrap <install|remove|recover> <app-data-directory> <game-installation> [prepared-bootstrap-directory]".into());
    }
    let action = args[0].to_str().ok_or("Invalid action")?;
    if !matches!(action, "install" | "remove" | "recover")
        || args.len() != if action == "install" { 4 } else { 3 }
    {
        return Err("Invalid bootstrap action or arguments.".into());
    }
    let game = game::inspect(&PathBuf::from(&args[2]))?;
    let mut storage = Storage::open(&PathBuf::from(&args[1])).map_err(|e| e.to_string())?;
    match action {
        "install" => {
            let owned = deployment::install(&mut storage, &game, &PathBuf::from(&args[3]))?;
            println!(
                "Bootstrap files prepared; {owned} files owned. Game/runtime activation is not verified."
            );
        }
        "remove" => {
            deployment::remove(&mut storage, &game)?;
            println!("Recorded bootstrap files removed. Unowned files and saved data retained.");
        }
        _ => {
            deployment::recover(&mut storage, &game)?;
            println!("No bootstrap recovery remains pending.");
        }
    }
    Ok(())
}
