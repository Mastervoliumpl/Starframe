# Using Starframe

This guide describes the current 0.7.0 development build for Windows 11 x64. Prior Windows 11 25H2 installer acceptance is retained. The signed 0.6.0 development draft remains for maintainer testing; no public app release or 0.7.0 installer is available yet. Obtain future public installers from [Starframe's official releases](https://github.com/Mastervoliumpl/Starframe/releases). Other operating systems and architectures are outside the initial support scope.

## Install and select the game

Close Starframe before running setup. Installation is for the current Windows account and includes Starframe's game-runtime files. Microsoft Edge WebView2 is required to display the app. If it is missing, setup attempts to obtain it; a download or installation failure stops setup with an explanation. Restore the connection or install Microsoft's WebView2 runtime, then retry setup. A game installation is not required just to open the manager.

The initial alpha has no Windows publisher certificate. Windows may show an unknown-publisher warning or block it under local policy. Starframe's update signatures verify update files but do not remove Windows publisher warnings. Do not disable security software or bypass a malware detection to run it.

In Settings, use **Find in Steam** or **Choose game folder**, then select the Sanctuary installation. Close the game while Starframe prepares its runtime automatically. Wait for the launch area to report that the setup is ready. If files need repair, use **Repair or reinstall runtime** in Settings. It retains mods and settings; conflicts with externally changed files are reported for inspection.

## Install and enable mods

Open Mods and sign in through Settings using Steam on Starframe’s website. Search or filter the bounded results, then open a mod’s details. Select the release you want from its history and choose Install this release. Downloads shows transfer, preparation, retry/cancel controls and any failure. Approval applies to that exact release; it does not guarantee that its code is harmless.

For a dependency, choose Find releases to see its exact or version-range constraint and matching history. A suggestion identifies the newest usable match on that page; check older pages when needed. Choose installation explicitly, then enable the required releases in My mods. To update, install your chosen newer release, disable the previous selection and enable the new one. Installed versions and exported/shared references keep their exact identities.

Production registry downloads in this development build need a separately approved registry trust root. Local fixture acceptance does not provision that root or establish hosted Steam readiness. Installed mods and local imports remain available.

Installed releases appear in My mods. Enabling a release adds it to the active collection. Collections hold names and ordered mod references. Required dependencies constrain the effective order; drag priority changes order where those constraints permit it. Later supported Lua overlays take precedence when they supply the same path.

Changes apply automatically while the game is closed. If the game is running, close it and wait for the saved setup to become ready before launching again. The game Mods menu lists successful entries from the current session and separates failures under **Could not load**. Mod settings belong to each mod, independently of the collection; restart when a setting requires it.

A game-version warning means compatibility has not been established for that build. You may still try the mod. Starframe does not synchronize a multiplayer lobby's mods or settings in this version.

## Share a collection or import a local build

In Collections, choose **Export**. The resulting `.starframe-collection.json` contains the collection name and exact ordered references. It contains no binaries, mod settings or local source paths.

The recipient chooses **Import collection**, selects the file or pastes its JSON, and reviews the result. Starframe reuses verified local copies and downloads missing approved releases. Unavailable references stay visible; it never silently substitutes a newer version. Use **Retry import** after fixing an interrupted download. Choose **Use collection** once the setup is complete.

Local builds must be supplied separately with their metadata and imported into the recipient's library. Follow [Develop and share a local mod](local-development.md) for DLL/folder metadata, source watching and exact-build sharing. Watching runs only while Starframe is open; game files wait until the game closes. Uninstalling a managed mod retains its original source and saved mod configuration.

## Updates, offline use and removal

Settings provides **Stable** and **Preview** app-update channels. New prerelease installations default to Preview; stable installations default to Stable. Starframe checks after startup and periodically while open. You choose when to update, or choose Later. Updates verify the installer signature, wait for the game and file operations, preserve app data and reopen Starframe. Closing Starframe stops its background work; it does not install a service or a persistent updater.

Mods refresh is separate from app updates. Discovery runs after sign-in and every five minutes while Starframe is open; manual refresh keeps your filters. A failed refresh identifies previous results. New downloads require a valid manager session and fresh signed approval/security metadata. Installed copies remain usable offline, subject to retained signed security blocks. A block stops new downloads and activation through Starframe while preserving files and settings. Removal cannot undo code that already ran.

Run setup again to reinstall missing or damaged app files without deleting saved data. For full removal, use Windows Installed apps. Close Starframe and the game first. Uninstall removes Starframe's recorded game integration and then the app. A conflict or unavailable game location stops cleanup so you can correct it and retry.

**Keep my library, collections and settings** starts unchecked. Leave it unchecked to delete Starframe-managed data; select it to retain that data for a later installation. Original local-import sources, game saves and unowned game configuration remain. Do not manually delete recovery records to get past a cleanup error.

## Saved data in 0.7.0

Current registry/local records and collections survive restart. Schema 19 records are retained after backup and removal of catalog-only rows. Earlier pre-release databases and collection exports are unsupported. An unsupported database produces an error and remains on disk; it is never silently replaced. Close Starframe and retain the entire old app-data directory before choosing a separate new data directory. Preserve original local sources and game recovery records. Do not delete game files to clear a storage error.

## Troubleshooting and reporting

| Problem | Next step |
| --- | --- |
| Setup says Starframe is running | Close its window and wait for file work to stop, then retry. |
| WebView2 download or installation failed | Restore network access or install Microsoft's runtime, then rerun setup. |
| Game folder unavailable | Reconnect the drive or select the correct installation in Settings. |
| Runtime preparation or uninstall stopped | Close the game, read the conflicting-file/location message, correct the cause and retry. Preserve recovery files. |
| Collection cannot be applied | Resolve the listed dependency, missing release or security block. Keep the exact references supplied by the sender. |
| Update signature rejected | Keep the current installation and report the release identity. Do not disable verification. |
| Mods unavailable or session ended | Check the connection, refresh or sign in again; use already installed mods while offline. |

Help & logs contains a responsiveness check using simulated transfers and memory hashing. It does not download mods or modify game files. Log export is not implemented. Diagnostics are not automatically attached to a report.

Use **Report a mod problem** for ordinary problems and **Report a security concern privately** for suspected vulnerabilities or malicious code. Include Starframe's version, the game build if relevant, exact mod/release identifiers from Package details, the error text and the shortest reproduction steps. Review text and screenshots before sharing: game/source paths and logs can identify your Windows account or contain private information. Replace personal paths with placeholders, and remove credentials, private chat and unrelated desktop content. **Copy source path** copies the actual path; it is not a privacy-filtered diagnostic export. Collection export excludes source paths and settings, but its chosen name and mod IDs are still visible to recipients.

## Compatibility and current limits

The integration supports Starframe managed entries, compatible BepInEx 5 Unity/Mono component plugins and the tested Lua overlay layout. Ladder Reporter 0.3.0 has [recorded installed-game acceptance](verification/ladder-reporter-catalog.md). [Six other Remmy catalog releases](verification/remmy-catalog.md) have archive/metadata review, not recorded gameplay acceptance through Starframe. They must not be described as gameplay-tested. Sanctuary HUD 0.8.0's bundled voice packs use an author-specific fixed path that Starframe does not populate; its fallback and limits are recorded in that review.

[BepInEx compatibility](bepinex-mods.md) excludes BepInEx 6, preloader patchers, plugins requiring earlier loading and reliance on a file-backed `Assembly.Location`. Registry Map and AI declarations have verified fake-game file placement. Map/AI loading and gameplay remain unverified; Starframe adds no AI selector. Initialization success does not prove later gameplay or multiplayer behavior. Do not install a duplicate plugin through another loader. DLL hot reload and automatic multiplayer mod synchronization are not available.

Existing default-display and keyboard results are retained. Additional high-contrast, screen-reader and unusual scaling combinations are not part of 0.6.0 acceptance; report a concrete usability problem with its display settings so it can be investigated.
