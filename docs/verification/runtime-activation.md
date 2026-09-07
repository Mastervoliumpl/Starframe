# Runtime activation and package support

Issue [#13](https://github.com/Mastervoliumpl/Starframe/issues/13), milestone 0.2.0. The entry plugin runs inside BepInEx and activates only the prepared managed entries. Desktop setup, launch/readiness UI and in-game settings remain #14/#15.

## Loading and reporting

`engine/BepInEx/plugins/Starframe` holds the entry plugin, core runtime and its locked Microsoft dependencies. `engine/Starframe/activation.json` lists active payloads below `engine/Starframe/mods`; disabled inventory entries need no deployed DLL. The existing guarded deployment command installs the complete package using the #12 ownership journal and rollback. Its development inventory is explicitly selected local input, not a trusted author download or catalog substitute.

The core validates the full activation document before reading payloads. It checks paths, rejects reparse points, verifies every declared file hash and keeps executable bytes in memory before activation. Managed assembly filenames must match their identities; conflicting inventories, game/runtime name collisions and unlisted managed dependencies fail. The game/framework libraries are trusted dependencies. This is controlled activation, not a security sandbox: an activated DLL has the game's process privileges and can perform its own loading or I/O.

Managed `IMod.Initialize` calls run in supplied order. Failed required dependencies cause `skipped_dependency`; unrelated entries can continue. The context supplies the mod ID, verified content root and prefixed logging. Optional shutdown runs in reverse order, including best-effort cleanup after failed initialization. It does not unload assemblies or promise to undo arbitrary patches. A new activation requires a new game process. Existing BepInEx plugins outside Starframe's directories remain outside this ordering policy.

Reports contain deployment revision, a new session GUID, process ID and Windows process creation time. A report is flushed and replaced only after complete activation results exist. The Rust matcher checks revision, process identity and the complete ordered mod list; the launch feature must also require successful outcomes. Invalid activation documents produce logs and no new successful report. Stale reports retained on disk cannot satisfy another process or deployment. The runtime has no database, network or desktop connection.

## Targets and game evidence

Verified on 7 September 2026 against installed playtest build 25135612, game version 0.0.1.15, Unity 6000.3.22 and BepInEx 5.4.23.5. The BepInEx entry plugin uses .NET Standard 2.1 because the installed Unity references require it. The core and fixture mods use .NET Standard 2.0, so their JSON dependency assets are selected for Mono. The entry-plugin build copies the core's complete dependency output, including System.Memory, System.Buffers and System.Threading.Tasks.Extensions. Unity and BepInEx references use `Private=false`; no game assembly is copied or committed.

The first launch of the all-2.1 build exposed a missing System.Memory assembly that the .NET test host had not needed. Retargeting the core and copying its runtime dependencies resolved it. This is a packaging requirement, not a requirement to install .NET 10 into the game.

Two fixture DLL entries then initialized in order in the game and produced a matching report. The process remained responsive for more than 60 seconds with the desktop absent. A fresh launch using the failing fixture produced `failed` followed by `skipped_dependency`, with a different process/session identity and deployment revision. These are runtime smoke checks, not a complete gameplay or multiplayer acceptance test.

The entry plugin sets the existing BepInEx host's hide flags to `HideAndDontSave`. This follows the Sanctuary host-lifetime problem described in [Remmy's loader/setup notes](https://github.com/Remmyboy/sanctuary-mods/tree/709562f3de3afc227cdc1c3b247fc9d808f1b0b0#modloader), without replacing a user's BepInEx configuration file. Starframe does not change BepInEx's normal plugin scan directory or implement hot reload.

After smoke tests, the guarded removal command removed the recorded bootstrap/runtime/payload files. Original engine-root files retained their pre-test SHA-256 values. Generated BepInEx logs/config, runtime reports, empty directories, lock metadata and database backups were retained under the existing ownership policy. Tests stopped only processes started for these checks.

## Package-format investigation

| Package | Current result |
| --- | --- |
| Managed DLL implementing the internal Starframe lifecycle | Fixture activation and dependency failure propagation verified in the installed game. |
| Managed DLL plus companion content | Declared files are verified and the content root is available to the mod. This does not automatically overlay the game's Lua/data cache. |
| Conventional BepInEx DLL plugin | Unsupported through Starframe activation. Its entry does not implement the internal lifecycle. A tested adapter must preserve BepInEx metadata, dependency and component behavior before support is advertised. |
| Lua-only or map-only package | Activation schema 2 represents it with null entry assembly/type and a nonempty inventory. The current runtime reports `unsupported_content`; it never creates a wrapper DLL or claims content was applied. |
| AI package | Deferred by owner until suitable game extension/selection facilities exist. |

[Map evidence](../planning/map-support.md) separates `.sanmap` folders and companion textures from MapLocalFiles, an optional DLL fallback for map-local Lua reads. Read-only inspection of the installed `Trebuchet.dll` confirms `EM.Lua.FilesCache`, its file-content/directory indexes, `TryGetFileContent`, `LoadFiles`, `RebuildDirectoryIndex` and Lua hash methods. Cache construction contains the `LJ/lua` and `Sanctuary_Data` roots with `.lua`, `.santp` and `.sanmap` searches; its Lua hash method filters `.lua`. The installation contains map directories under `engine/Sanctuary_Data/Maps`.

This supports investigating the cache and map discovery paths described by Remmy. It does not establish that a newly deployed map is playable, that cache changes are safe during a match, or that every content type participates in multiplayer hashes. No shipped Lua, AI or map content was changed. Content activation, collision precedence and conventional-plugin compatibility remain explicitly unsupported until separately verified; #20 must retain those limits when exposing ordering.

## Local build and smoke commands

Prepare the official bootstrap with the #12 command first. From the repository root:

```powershell
dotnet build runtime/Starframe.Bootstrap/Starframe.Bootstrap.csproj --configuration Release -p:BootstrapPath="<verified-bootstrap>" -p:GameManagedPath="<game>/engine/Sanctuary_Data/Managed"
dotnet build runtime/Starframe.FixtureMods/Starframe.FixtureMods.csproj --configuration Release
python scripts/prepare_runtime_fixture.py "<new-package>" --runtime runtime/Starframe.Bootstrap/bin/Release/netstandard2.1 --fixture runtime/Starframe.FixtureMods/bin/Release/netstandard2.0/Starframe.FixtureMods.dll
cargo run --manifest-path src-tauri/Cargo.toml --locked --example bootstrap -- runtime "<app-data>" "<game>" "<verified-bootstrap>" "<new-package>"
```

Close the desktop and game before deployment. Launch the selected game, inspect its `BepInEx/LogOutput.log` and `Starframe/report.json`, and compare process ID/creation time and deployment revision. Prepare a separate package using `--fail`, close the game, deploy and relaunch to verify failure propagation. Remove integration with the #12 `remove` command after closing the test game. Fixture packages are for development only and must not be published as catalog releases.

Ordinary CI builds/tests the core and fixture projects without proprietary references. The Unity entry plugin has a separate mandatory local build and game smoke check; it is not in the reference-free CI solution. Missing local references fail its build explicitly. This remains a CI coverage limit until a permitted game-reference acquisition path is available to the runner. No reference stubs or copied game binaries stand in for that check.

The focused checks cover shared positive/negative contracts, content-only metadata, activation order, hash failures, dependent skipping, shutdown, atomic report replacement, stale/foreign/incomplete reports, prepared package hashes and unlisted payload rejection. Full Rust/C# suites and required GitHub checks are recorded on #13 before closure.
