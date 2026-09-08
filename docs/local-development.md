# Develop and share a local mod

The 0.5.0 workflow uses the normal My mods, Collections and game Mods screens. It supports the current Starframe-managed DLL interface and supported Lua files. An arbitrary DLL or conventional BepInEx plugin is not automatically compatible; broader adapters remain in [issue #30](https://github.com/Mastervoliumpl/Starframe/issues/30).

## Import the first build

1. Build the mod into a dedicated output folder outside Starframe's managed app-data directory. Include only the runtime files it needs.
2. Add `starframe.local.json` to that folder. For a standalone `Mod.dll`, put `Mod.starframe.json` beside it instead. Use the [metadata example and layout rules](verification/local-imports.md#import-a-build).
3. In **My mods**, choose **Import local mod**, select the folder or DLL, then choose **Import copy**. Downloads reports preparation failures. A successful import appears in My mods as **Local import**.
4. Select the collection you want to use and enable the imported build. Install or import its declared dependencies first. If game setup is incomplete, select the installation in Settings and finish setup.
5. Wait for the launch area to report that the saved setup is ready, then launch the game. A runtime confirmation establishes that the declared entries activated in that process. Open the game's **Mods** menu to inspect their status and registered settings.

You supply the stable mod ID, name, author, version label, entry point/layout and declared dependencies in the metadata. Starframe validates the supported format, checks paths and file limits, computes content identity and manages verified copies. It does not infer an unknown mod's intended loader, entry type or dependencies from arbitrary files. The metadata requirement belongs to the current local import format; catalog downloads use separately maintained catalog metadata.

## Rebuild while developing

Keep Starframe open and rebuild into the imported source location. The latest explicit import for each mod ID is marked **Following source**. The short build hash distinguishes copies even when you keep the same version label.

Starframe waits for writes to settle, checks the output and saves a new managed copy. Invalid metadata, locked output or a truncated managed DLL keeps the previous copy available and shows the source error in My mods. Fix the output and watching retries. File consistency checks cannot prove that the mod's code works; inspect runtime failures after launch as well.

A new copy replaces the matching entry in your active locally created collection. If the game is closed, Starframe applies that collection automatically. During play, the launch area shows the pending revision, and game files keep their current contents. Several rebuilds leave the latest saved build queued. Close the game, wait for the ready state, then launch again. DLL hot reload is not supported.

Shared imported collections and inactive collections keep exact builds. They do not follow a source automatically. To use a newer build there, select the collection and enable that build explicitly. If you deliberately enable an older saved build in the active collection, it also stays pinned until you change it.

Closing Starframe stops watching and applying changes. It does not stop an already running game. Reopen Starframe to reconcile source changes and the current game state. Startup, resume and a bounded fallback check recover changes that did not produce a file notification. A removed source keeps its last managed copy; restore the source or import its new location.

## Order and settings

Collections save requested order. Required dependencies constrain the effective load order; manual priority applies where those constraints allow it. The manager explains unresolved dependencies, cycles and ordering conflicts. Managed entries initialize in effective order. Supported Lua files use the integration's file-overlay order. Starframe does not control unrelated BepInEx plugins installed outside its managed directories. See [ordering capabilities and checks](verification/ordering.md).

Mod settings belong to the mod ID, independently of collections and build hashes. The in-game page identifies which settings apply live and which need a restart. A setting marked **Restart required** keeps its old effective value for the current process. Rebuilding, selecting another collection and uninstalling the managed build retain saved configuration. Changing the mod ID creates a different settings identity.

## Share a local collection

Export the collection from **Collections**. The file contains its name and ordered exact references; it contains no binaries, source paths or settings. Use the exported references instead of calculating or editing hashes by hand.

Give the recipient the matching mod output and local metadata separately through your chosen channel. They import those files into their own library, then import the collection. The source folder may be elsewhere on their computer; source paths are excluded from content identity. Matching metadata and bytes allow reuse of the exact local build. A different or missing build remains an unresolved local requirement; Starframe cannot fetch a local-only build from the catalog or silently substitute a newer one.

The recipient's imported collection retains the shared references even if their local source later changes. Their settings remain their own. See [collection sharing](verification/sharing.md) for missing and withdrawn catalog releases.

## Remove a build

Use **Uninstall** on the exact row in My mods and review the affected collections. Starframe removes its managed reference and cleans up files it owns when safe. Source DLLs, source folders, metadata and per-mod settings remain. Uninstalling the watched build also stops following that source; reimport it to resume.

Older copies remain available for exact collections and rollback until you uninstall them. Repeated builds therefore use additional disk space. [Watcher verification](verification/local-watching.md) records the bounds, recovery behavior and current limitations.
