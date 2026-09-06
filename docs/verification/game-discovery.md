# Game discovery and milestone 0.1.0 verification

Issue [#10](https://github.com/Mastervoliumpl/Starframe/issues/10), checked on Windows 11 Education 25H2, build 26200, on 6 September 2026. This completes the desktop foundation with issues #6–#9. Version `0.1.0` is an internal milestone build; it has no installer, published release, mod management or game launch support.

## Implemented behavior

Settings can find Steam installations, choose a folder through the native Windows picker and save one selected installation. Cancel leaves the selection unchanged. Invalid folders show an error and retain the previous selection. Saved-data failures prevent selection from being reported as saved. Paths remain readable at large text sizes; navigation and diagnostic work remain available during discovery.

Steam's registered location leads to `libraryfolders.vdf` and the known app manifests. Metadata includes the release app `1699050`, demo `2375120` and playtest `4511930`. Release and demo identities are listed on their [official Steam pages](https://store.steampowered.com/app/1699050/Sanctuary_Shattered_Sun/) and [demo page](https://store.steampowered.com/app/2375120/Sanctuary_Shattered_Sun_Demo/); the playtest identity was verified from the installed manifest. These identities do not promise support for an uninspected future game layout.

The inspector accepts the verified `engine/` layout and a fixture-tested flat equivalent. It checks the Sanctuary product metadata, PE x64 headers, Unity player, managed/Mono directories and Unity build GUID. Steam build numbers are read from the matching manifest, not compared with a fixed allowed build. Unfinished Steam installs/updates are unavailable. A manually selected folder without matching Steam metadata retains its Unity build identity and reports the Steam build as unknown. Required paths are canonicalized and must remain within the installation. Inspection does not establish mod compatibility or binary safety.

All SQL stays in `storage.rs`. Schema 3 adds one selected installation ID/path to the schema-2 database. Migration retains library and collection records and creates a backup first. The existing [storage recovery procedure](storage.md#recovery-procedure) applies. Selection commands accept a discovery ID or open a native picker; the frontend cannot submit a filesystem path for inspection.

The worker owns the database and performs file work outside the UI thread. It scans Steam libraries at startup and on Find in Steam. It observes only matching executable names through a Windows process snapshot every two seconds, then resolves their full paths before reporting the selected game as running. An inaccessible matching process or failed snapshot reports unknown. Another installation's process does not count as the selected game. [Microsoft's process-snapshot API](https://learn.microsoft.com/en-us/windows/win32/toolhelp/taking-a-snapshot-and-viewing-processes).

The selected layout/build is revalidated every 30 seconds. A long observation gap or clock reversal invalidates prior process evidence and triggers one fresh validation. There is no replay of missed scans, recursive Steam polling, service or tray process. Closing Starframe ends the worker. Game files are never written by this issue.

Dependencies added for this behavior are `keyvalues-parser 0.2.4` and `tauri-plugin-dialog 2.7.3`, pinned exactly. Process/registry calls use the existing `windows 0.61.3` dependency with the required API features. The [native dialog plugin](https://v2.tauri.app/plugin/dialog/) runs through Rust; frontend filesystem and dialog-plugin permissions remain unavailable.

## Verification

| Check | Evidence |
| --- | --- |
| Multiple libraries | Rust fixtures cover separate libraries, both layouts, changing build numbers, malformed/repeated metadata, invalid folders and escaping installation paths. |
| Persistence | Schema-2 upgrade and close/reopen retain the selected ID/path and existing library records. Invalid selection records fail without replacing them. |
| Process uncertainty | Tests distinguish the selected executable, another path with the same name, unreadable matching processes and failed snapshots. Simulated elapsed-time gaps and clock reversal expire observations. |
| Native picker | Windows tests select a valid fixture, cancel the picker and choose an invalid folder. Selection survives cancellation/error and subsequent restart. The picker is parented to the main app window. |
| Native observation | A renamed Windows ping executable serves as a temporary fixture. Starting it outside Starframe changes the live state to running; stopping it changes the state back. No game is started by the test. |
| Live changes | Editing the fixture's Steam build changes the displayed build without refresh. Invalidating its product metadata shows an unavailable saved location and retains that path. |
| Read-only installed layout | The inspection example recognized playtest app `4511930`, Steam build `25135612`, Unity GUID `8f51f4585cff46d18b37fd04b0c808d2` and a stopped process. Before/after inventories matched for all 2,014 files by path, size and last-write timestamp. This is an inventory check, not a full binary hash comparison. |
| Frontend | Formatting, lint, Svelte/TypeScript, six Vitest tests and production build passed. Five browser tests include keyboard game selection, 200% text sizing, forced colors and reduced motion. |
| Rust | Rustfmt, Clippy with warnings denied and 17 tests passed. The ignored storage worker is invoked by the forced-termination test. Generated frontend contracts match Rust. |
| Repository | Version synchronization, document references and 12 Python regression tests passed. Local AGENTS.md remains excluded and untracked. |
| Windows | Release and debug executables build. Native checks cover storage recovery messages, picker behavior, game state, diagnostics, reconnect, cancellation, single-instance ownership and process exit. Tests use temporary data and game fixtures. |

Run `npm run check`, `npm run check:rust`, `npm run test:browser`, then build the debug desktop and run `npm run test:native`. The Windows test runner needs WebView2 and an interactive desktop. Native picker tests address controls in their own app process; their process-only PowerShell execution-policy override does not change system policy. Release builds ignore `STARFRAME_TEST_DATA_DIR` and `STARFRAME_TEST_STEAM_ROOT`. The inspection example accepts an explicit folder and performs reads only.

## Milestone exit and limits

Restart persistence, slow-work interaction and Windows executable checks pass. The native diagnostic run retained navigation, search, selection and unfinished notes with three active lanes; its 100-search p95 was below 100 ms. Workload and hardware limits remain in the [desktop verification record](desktop-state.md). The new Settings view follows the approved palette, type, navigation and native-picker direction; its normal and error states were inspected in WebView2 at 1280 × 800 logical pixels and 150% display scale. No new visual direction was selected.

The required GitHub status on [PR #38](https://github.com/Mastervoliumpl/Starframe/pull/38) must pass before merging this completion record. It includes repository, frontend and Windows jobs. A local build is not evidence of a hosted-runner result.

The initial hosted runs passed Rust checks and executable builds, then failed to connect to WebView2. The runner had an interactive desktop and Runtime 151 installed. Microsoft explains that [elevated hosts ignore WebView2 environment overrides from Runtime 150 onward](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5640). Debug builds now pass the test port and isolated profile through Tauri's WebView configuration when a test data directory is set. Only ports 9223/9224 are accepted; release builds exclude this setup. Native tests retain startup output and allow 30 seconds for first startup; the 100 ms interaction target is unchanged.

CI checks the Evergreen Runtime before compiling and installs Microsoft's signed bootstrapper only when it is absent. Runtime detection follows [Microsoft's distribution guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

Physical sleep/resume, protected game processes, native screen-reader use and the four-core/8 GB performance baseline remain unverified on this machine. Their deterministic uncertainty rules and browser semantics are tested. Real transfers, extraction, loader/runtime behavior, final launch artwork and installer/update lifecycle belong to later milestones. Demo/release layouts still need actual installation evidence before support is advertised. No later milestone starts with this completion.
