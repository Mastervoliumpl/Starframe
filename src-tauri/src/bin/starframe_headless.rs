use serde_json::{Value, json};
use starframe::{
    backend, game, mods,
    packages::{self, Packages},
    storage::Storage,
};
use std::{io::Read, path::Path, thread, time::Duration};
use uuid::Uuid;

fn argument<'a>(args: &'a [String], index: usize, name: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("Missing {name}."))
}

fn exact_args(args: &[String], count: usize) -> Result<(), String> {
    if args.len() == count {
        Ok(())
    } else {
        Err("Unexpected arguments. Run without arguments for usage.".into())
    }
}

fn installation(path: &str) -> Result<game::Installation, String> {
    game::inspect(Path::new(path))
}

fn wait_for_package(
    store: &mut Storage,
    queue: &mut Packages,
    request_id: &str,
) -> Result<Value, String> {
    let mut reported = None;
    loop {
        backend::poll_packages(store, queue)?;
        let operations = backend::package_action(store, queue, packages::Action::List)?;
        let operation = operations
            .into_iter()
            .find(|item| item.request_id == request_id)
            .ok_or("The package operation was lost.")?;
        let progress = (
            operation.status.clone(),
            operation.received_bytes / 1_048_576,
        );
        if reported.as_ref() != Some(&progress) {
            eprintln!("{}", json!({"type": "progress", "operation": operation}));
            reported = Some(progress);
        }
        if operation.status == packages::Status::Completed {
            return serde_json::to_value(operation).map_err(|error| error.to_string());
        }
        if matches!(
            operation.status,
            packages::Status::Failed | packages::Status::Cancelled
        ) {
            return Err(operation.message);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn run(args: &[String]) -> Result<Value, String> {
    if args.len() < 2 {
        return Err("Usage: starframe_headless <absolute-data-dir> <status|import|prepare-package|collection-create|collection-select|select-game|readiness|setup|setup-selection|launch> [arguments].".into());
    }
    let root = Path::new(&args[0]);
    if !root.is_absolute() {
        return Err("The data directory must be an absolute path.".into());
    }
    let mut store = Storage::open(root).map_err(|error| error.to_string())?;
    let mut queue = Packages::open(&mut store)?;
    match args[1].as_str() {
        "status" => {
            exact_args(args, 2)?;
            let view = backend::mod_action(&mut store, Some(&queue), mods::Action::List)?;
            Ok(
                json!({"mods": view, "selectedGame": store.selected_game().map_err(|error| error.to_string())?}),
            )
        }
        "import" => {
            exact_args(args, 3)?;
            let path = argument(args, 2, "source path")?;
            if !Path::new(path).is_absolute() {
                return Err("The source path must be absolute.".into());
            }
            let request_id = Uuid::new_v4().to_string();
            backend::package_action(
                &mut store,
                &mut queue,
                packages::Action::ImportLocal {
                    request_id: request_id.clone(),
                    path: path.into(),
                },
            )?;
            wait_for_package(&mut store, &mut queue, &request_id)
        }
        "prepare-package" => {
            exact_args(args, 3)?;
            let request_id = Uuid::new_v4().to_string();
            backend::package_action(
                &mut store,
                &mut queue,
                packages::Action::Prepare {
                    request_id: request_id.clone(),
                    release_id: argument(args, 2, "release ID")?.into(),
                },
            )?;
            wait_for_package(&mut store, &mut queue, &request_id)
        }
        "collection-create" => {
            exact_args(args, 3)?;
            let view = backend::mod_action(&mut store, Some(&queue), mods::Action::List)?;
            let view = backend::mod_action(
                &mut store,
                Some(&queue),
                mods::Action::CreateCollection {
                    name: argument(args, 2, "collection name")?.into(),
                    expected_revision: view.revision,
                },
            )?;
            serde_json::to_value(view).map_err(|error| error.to_string())
        }
        "collection-select" => {
            exact_args(args, 3)?;
            let view = backend::mod_action(&mut store, Some(&queue), mods::Action::List)?;
            let view = backend::mod_action(
                &mut store,
                Some(&queue),
                mods::Action::SelectCollection {
                    id: argument(args, 2, "collection ID")?.into(),
                    expected_revision: view.revision,
                },
            )?;
            serde_json::to_value(view).map_err(|error| error.to_string())
        }
        "select-game" => {
            exact_args(args, 3)?;
            let game = installation(argument(args, 2, "game directory")?)?;
            backend::select_game(&mut store, &game)?;
            serde_json::to_value(game).map_err(|error| error.to_string())
        }
        "readiness" => {
            exact_args(args, 3)?;
            let game = installation(argument(args, 2, "game directory")?)?;
            Ok(json!({"activation": backend::readiness(&store, &game)?}))
        }
        "setup" | "launch" => {
            exact_args(args, 4)?;
            let game = installation(argument(args, 2, "game directory")?)?;
            let resources = Path::new(argument(args, 3, "integration resource directory")?);
            if !resources.is_absolute() {
                return Err("The integration resource directory must be absolute.".into());
            }
            let activation =
                backend::prepare(&mut store, &game, resources, args[1] == "launch", &|| false)?;
            Ok(json!({"activation": activation}))
        }
        "setup-selection" => {
            exact_args(args, 5)?;
            let game = installation(argument(args, 2, "game directory")?)?;
            let resources = Path::new(argument(args, 3, "integration resource directory")?);
            let selection = Path::new(argument(args, 4, "exact selection file")?);
            if !resources.is_absolute() || !selection.is_absolute() {
                return Err("Resource and selection paths must be absolute.".into());
            }
            let mut bytes = Vec::new();
            std::fs::File::open(selection)
                .map_err(|error| error.to_string())?
                .take(starframe::runtime_contract::MAX_DOCUMENT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            if bytes.len() > starframe::runtime_contract::MAX_DOCUMENT_BYTES {
                return Err("The exact selection file exceeds 1 MiB.".into());
            }
            let value = starframe::runtime_contract::unique_json(&bytes)?;
            let references: Vec<starframe::references::Reference> =
                serde_json::from_value(value).map_err(|error| error.to_string())?;
            let activation = backend::prepare_references(
                &mut store,
                &game,
                resources,
                &references,
                false,
                &|| false,
            )?;
            Ok(json!({"activation": activation}))
        }
        _ => Err("Unknown headless command. Run without arguments for usage.".into()),
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(data) => println!("{}", json!({"ok": true, "data": data})),
        Err(message) => {
            let code = match args.get(1).map(String::as_str) {
                Some("import" | "prepare-package") => "package_failed",
                Some("collection-create" | "collection-select" | "status") => "mods_failed",
                Some("select-game" | "readiness" | "setup" | "setup-selection" | "launch") => {
                    "game_failed"
                }
                _ => "invalid_command",
            };
            eprintln!(
                "{}",
                json!({"ok": false, "error": {"code": code, "message": message}})
            );
            std::process::exit(2);
        }
    }
}
