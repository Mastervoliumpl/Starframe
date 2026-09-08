# Milestone 0.5.0 verification

Milestone [0.5.0](https://github.com/Mastervoliumpl/Starframe/milestone/6) covers local imports (#24), watched builds (#25) and the developer workflow (#26), through [PR #56](https://github.com/Mastervoliumpl/Starframe/pull/56). This is an internal build; no installer or app release is published.

The [local development guide](../local-development.md) gives the import, rebuild, launch, settings, ordering, sharing and uninstall steps. [Import verification](local-imports.md) defines supported metadata and layouts. [Watcher verification](local-watching.md) records stability checks, exact collection behavior, storage migration and recovery limits.

## Automated checks

Implementation head `8a56f5c` passed [required checks](https://github.com/Mastervoliumpl/Starframe/actions/runs/34251851545) and [dependency audits](https://github.com/Mastervoliumpl/Starframe/actions/runs/34251851536). Required CI includes repository/version checks, frontend formatting/lint/types/unit and browser tests, Rustfmt/Clippy and 116 Rust tests, C# runtime/contract tests, the full native Windows suite and the Windows release executable. Four Rust subprocess entry points are invoked by their parent tests.

The native watcher fixture verifies multiple builds during one game process, invalid metadata, desktop close/restart, unchanged game files during play, latest deployment after exit, focus retention and source/settings retention. Rust tests cover the actual fallback interval, resume reconciliation, missing output, truncated managed-image headers, exact shared/inactive references, transaction failure, late results after uninstall and schema 12 migration. These checks use isolated data and fake game installations.

## Installed-game workflow, 8 September 2026

The game walkthrough used the installed Sanctuary Playtest: Steam build **25135612**, game **0.0.1.15**, Unity **6000.3.22**, BepInEx **5.4.23.5**, runtime contract **1** and activation schema **3**. The desktop and rebuilt Unity entry plugin used `0.5.0-dev.1`. Finalizing the product version to `0.5.0` changes version metadata, not the verified behavior.

Only an isolated author library, a separate recipient library and the repository's managed fixture were used. Three compiled copies of `Starframe.FixtureMods.dll` differed in assembly-title metadata while retaining the same entry type and mod ID, `fixture.local.workflow`. A dedicated source folder contained that DLL and its local metadata. No game assembly or fixture binary is committed or distributed.

The walkthrough passed these steps:

1. Import and enable the local folder through My mods. Launch from Starframe and match the process/session-bound report to the deployed entry.
2. Open the game's normal Mods menu, inspect the loaded fixture, change marker count from `3` to `41`, and save with Enter. The page showed **Saved · Restart required**.
3. Replace the source DLL with a 256-byte partial build. My mods reported incomplete managed output, retained the previous build and left the running game's activation unchanged.
4. Publish the complete second DLL. The watched build advanced and the launch area showed a queued change. Closing the desktop left the game running. A third build written while the desktop was closed was found on restart.
5. Close the test game. Only the latest build deployed. Relaunch, verify the new report, open Mods again and confirm marker count `41` remained without the restart-required message.
6. Export the exact local collection. In the recipient library, review it first as an unresolved local requirement, then import the same source/metadata from another folder. Accepting the collection reused the matching build, and re-export produced identical JSON. The current catalog contained no mods; no catalog package supplied this local build.
7. Uninstall all author-library builds and remove the test integration through guarded cleanup. Source files and metadata remained, the configuration bytes were retained, and all **1,021** recorded original game-file hashes matched their pre-test values. Both test desktops and game processes were stopped.

The final original-build session used process `19568`, revision `3`, session `576822cf-2ced-4189-bb45-56a01cd5a08e`. The latest-build session used process `34964`, revision `5`, session `79b36846-59a2-46ea-9e80-5720cda4504b`. Both reported the fixture as loaded, and the desktop confirmed the matching result.

The first menu attempts encountered the game's **Permissions denied** screen while the harness supplied an invalid offline proxy. The same fixture reached the menu after those test-only overrides were removed, and subsequent normal-environment sessions succeeded. No product change or access-check bypass was used. The harness was also corrected for saved setup state, fresh report identity and BepInEx's escaped configuration strings. Its final catalog assertion accepts an empty fetched catalog as well as an absent offline cache. These were acceptance-harness corrections.

The Unity entry-plugin build retains the two previously recorded framework-reference warnings, and the game emits its existing UnityLogWriter/Mono warning. They did not prevent the successful runtime reports or settings checks. Local reports, activation manifests, source variants, desktop screenshots, configuration evidence and the manual harness remain under ignored `test-results/0.5.0`. The in-game screens were inspected through Computer Use at a 1707 × 960 window capture.

## UI and capability limits

Local builds use the ordinary manager controls. Following-source and saved-build labels distinguish equal version labels; source errors explain that the prior copy remains available. Pending deployment appears in the existing launch area. The in-game fixture used the normal Mods list and settings rows; there is no separate developer-only control panel. The review found no blocking layout or status issue for this workflow.

The workflow does not establish general gameplay, multiplayer, controller, screen-reader or arbitrary author-DLL compatibility. File stability cannot prove compiler success or correct mod behavior. There is no DLL hot reload. Shared imported and inactive collections retain exact builds; local sharing requires matching files and metadata supplied separately. Conventional BepInEx adapters and representative author-mod acceptance remain in #30. Physical OS sleep was not performed here; the automated resume signal and missed-notification checks are the evidence for reconciliation.

Future submission forms, richer mod pages and optional hosting are recorded in [the planning note](../planning/mod-hosting.md). They add no server, upload flow or telemetry to this milestone.
