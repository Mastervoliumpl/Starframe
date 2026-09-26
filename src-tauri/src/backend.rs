use crate::{
    deployment, game, launch, mods,
    packages::{self, Packages},
    sharing,
    storage::Storage,
    windows_game,
};
use serde_json::Value;
use std::path::Path;

pub fn mod_action(
    store: &mut Storage,
    queue: Option<&Packages>,
    action: mods::Action,
) -> Result<mods::View, String> {
    if let mods::Action::Uninstall { reference, .. } = &action
        && queue.is_some_and(|queue| queue.busy_hash(&reference.hash))
    {
        return Err(
            "This package is being prepared. Finish or cancel its download before uninstalling."
                .into(),
        );
    }
    mods::action(store, action)
}

pub fn sharing_action(
    store: &mut Storage,
    action: sharing::Action,
) -> Result<sharing::Reply, String> {
    sharing::action(store, action)
}

pub fn package_action(
    store: &mut Storage,
    queue: &mut Packages,
    action: packages::Action,
) -> Result<Vec<packages::Operation>, String> {
    match action {
        packages::Action::ImportLocal { request_id, path } => {
            queue.import_local(store, &request_id, &path)?;
        }
        packages::Action::Prepare {
            request_id,
            release_id,
        } => {
            queue.start(store, &request_id, &release_id)?;
        }
        packages::Action::Cancel { operation_id } => queue.cancel(store, &operation_id)?,
        packages::Action::List => (),
    }
    queue.operations(store)
}

pub fn registry_package_action(
    store: &mut Storage,
    queue: &mut Packages,
    request_id: &str,
    request: packages::RegistryRequest,
) -> Result<Vec<packages::Operation>, String> {
    queue.start_registry(store, request_id, request)?;
    queue.operations(store)
}

pub fn registry_receipts_action(
    store: &Storage,
    queue: &mut Packages,
    request: packages::ReceiptRequest,
) -> Result<usize, String> {
    queue.resume_registry_receipts(
        store,
        request.client,
        request.session,
        request.bearer,
        request.auth_cancel,
    )
}

pub fn registry_install_action(
    store: &mut Storage,
    queue: &mut Packages,
    request_id: &str,
    request: packages::RegistryRequest,
    display: crate::storage::RegistryDisplay,
) -> Result<Vec<packages::Operation>, String> {
    queue.start_registry_install(store, request_id, request, display)?;
    queue.operations(store)
}

pub fn poll_packages(store: &mut Storage, queue: &mut Packages) -> Result<bool, String> {
    queue.poll(store).and_then(|changed| {
        sharing::poll(store, queue).map(|imports_changed| changed || imports_changed)
    })
}

pub fn select_game(store: &mut Storage, installation: &game::Installation) -> Result<(), String> {
    store
        .select_game(&installation.id, &installation.path)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn prepare(
    store: &mut Storage,
    game: &game::Installation,
    resources: &Path,
    dispatch: bool,
    cancelled: &impl Fn() -> bool,
) -> Result<Value, String> {
    prepare_selection(store, game, resources, None, dispatch, cancelled)
}

pub fn prepare_registry(
    store: &mut Storage,
    game: &game::Installation,
    resources: &Path,
    references: &[crate::registry::ExactReference],
    dispatch: bool,
    cancelled: &impl Fn() -> bool,
) -> Result<Value, String> {
    prepare_selection(
        store,
        game,
        resources,
        Some(references),
        dispatch,
        cancelled,
    )
}

fn prepare_selection(
    store: &mut Storage,
    game: &game::Installation,
    resources: &Path,
    references: Option<&[crate::registry::ExactReference]>,
    dispatch: bool,
    cancelled: &impl Fn() -> bool,
) -> Result<Value, String> {
    if !resources.join("runtime/runtime-package.json").is_file() {
        return Err("This Starframe build does not include the game runtime. Use a build with runtime support to finish setup.".into());
    }
    let store = std::cell::RefCell::new(store);
    launch::prepare_latest(
        || match references {
            Some(references) => crate::registry::activation::requested(&store.borrow(), references),
            None => mods::requested(&store.borrow()),
        },
        |activation| {
            deployment::repair_missing(&mut store.borrow_mut(), game)?;
            deployment::prepare_desktop(
                &mut store.borrow_mut(),
                game,
                resources,
                activation,
                references,
                cancelled,
            )
            .map(|_| ())
        },
        || {
            if cancelled() {
                return Err("Starframe closed before launch was requested.".into());
            }
            if dispatch {
                windows_game::launch(game)?;
            }
            Ok(())
        },
    )
}

pub fn remove_runtime(store: &mut Storage, game: &game::Installation) -> Result<usize, String> {
    deployment::remove(store, game)
}

pub fn readiness(store: &Storage, game: &game::Installation) -> Result<Option<Value>, String> {
    deployment::prepared_activation(store, game)
}
