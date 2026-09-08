# Local build watching

Issue [#25](https://github.com/Mastervoliumpl/Starframe/issues/25), milestone 0.5.0, internal version `0.5.0-dev.1`. This adds source watching to [local imports](local-imports.md). The complete author/game workflow is recorded in [0.5.0 exit evidence](milestone-0.5.0.md). No installer or public release is produced.

## Behavior

Import a supported DLL with its metadata sidecar, or a supported build folder with `starframe.local.json`. The latest explicit import for each mod ID becomes its watched source. Starframe follows that source while the desktop is open, including while minimized. Closing the desktop stops watching; there is no service or tray worker.

My mods shows a short build hash and either **Following source** or **Saved build**. The watched build shows its current, settling or error message. Existing enable, order, details and uninstall controls still apply. Source errors keep the previous managed copy available and retry after a new notification or fallback check.

A verified rebuild advances the matching entry in the active locally created collection. The existing deployment worker applies that saved collection while the game is closed. During play, successive builds replace the pending collection reference; game files retain their deployed contents until exit. There is no DLL hot reload.

Shared imported collections and inactive collections retain their exact references. An active entry that already points at another saved build also stays unchanged. New builds remain available in My mods for explicit selection. A changed mod ID in the source metadata requires a new explicit import.

Older builds stay in the library until explicitly uninstalled. They can share the same author-supplied version, so the build hash distinguishes them. This retains exact collection matching and rollback choices, but uses disk space as builds accumulate. Existing space checks reject a copy that cannot fit and keep the previous build. There is no automatic build-history pruning in this issue.

Uninstalling the watched build stops following that source. Uninstalling an older saved build leaves the current watch in place. A prepared result arriving after uninstall cannot recreate the removed watch. Neither operation deletes the author's source, metadata sidecar or per-mod configuration.

## Source verification and limits

One worker checks up to 256 imported sources. Windows directory notifications prompt checks; they do not identify a verified build. The worker debounces notifications, hashes the source, waits two seconds, then requires the same normalized metadata and sorted file inventory before copying. Copying reuses the import path: retained read handles, path/reparse-point guards, size limits, file hashes, final inventory checks and managed staging. File work stays outside the desktop and SQLite owner threads.

Watching pauses new scans while manual package work or uninstall cleanup is pending. A scan already in progress can finish; accepting its result still checks the exact previous source. The existing package queue retains its own concurrency limit. A source scan is bounded by the import limits: 1,024 runtime files, 256 MiB per build and the supported layout rules.

Locked files, missing sources, changing fingerprints and malformed metadata keep the last copy. Watched DLLs also require bounded PE sections and a CLR metadata header. This rejects common truncated compiler output without loading the DLL. It does not validate all CLR tables, establish `IMod` compatibility, check Lua syntax, or prove that a build succeeds in the game. A stable, structurally acceptable file can still contain faulty code. Authors should publish completed output to their import location; Starframe has no compiler completion signal. [Microsoft PE format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format)

Every watched source receives a fallback hash check after 30 seconds, subject to preceding bounded work. Startup reconciles immediately. The desktop's existing observation-gap check requests reconciliation after a pause longer than ten seconds or a backward clock change. Repeated missed ticks do not enqueue repeated scans. Notifications are reopened after changes/errors. This also recovers removed and restored locations or unavailable native notification handles. Windows documents that remote notifications may be missed; the fallback covers this case without relying on them. [Windows directory notifications](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstchangenotificationw)

## Storage and recovery

SQLite schema 12 adds `local_watches`, keyed by mod ID and pointing at one saved `local_sources` record. Migration selects a source only when schema 11 contains one build for that mod. If it contains several, explicit reimport chooses the source; migration does not guess the newest from a hash.

The library entry, source record, watched reference, prepared inventory, matching active collection entry, collection revision and completed operation commit in one transaction. Failed commits retain the old references and mark the operation failed. A later scan can reuse verified managed bytes. Watch status changes do not advance the deployment revision.

Closing during preparation cancels file work. Copied content is published only after verification; it does not alter loader paths. As with manual imports, interrupted staging or unreferenced verified artifacts can remain for recovery. Startup re-reads persisted sources and prepares the current build again. The desktop does not attempt to restore a missed sequence of intermediate builds.

## Verification

Local checks on 8 September 2026:

| Check | Result |
| --- | --- |
| Rustfmt, Clippy, locked Rust suite and added migration test | Passed; 116 tests, plus 4 subprocess entry points exercised by parent tests |
| Frontend formatting, lint, types, unit tests and production build | Passed; 10 unit tests, no Svelte warnings |
| Local import, management, ordering and sharing browser workflows | 9 passed, including keyboard focus and reflow |
| Windows debug Tauri executable | Built successfully |
| Existing native mod and local import regressions | Passed: collection/order, game-exit/restart, picker, offline import and source retention |
| Native watched-source workflow | Passed: repeated builds during play, invalid metadata, desktop close/restart, latest deployment after exit and source/settings retention |

Watcher tests cover rapid writes, malformed output, truncated managed-image headers, the actual 30-second fallback with notifications disabled, resume reconciliation, missing source files, changed identity, restart reconciliation, shared/inactive exact references, an injected SQLite transaction failure, late results after uninstall and schema 11 migration with ambiguous sources.

The native workflow uses a temporary Steam fixture, a renamed Windows `ping.exe` as its observable game process, inert bootstrap/runtime fixtures and local Lua payloads. It compares activation bytes during play and the deployed payload after exit. It also verifies that typing focus and search text survive a rebuild. The fixture does not execute mod code. Shared game setup is reused by the existing native mod regression; all paths are normalized before persistence.

The UI retains the existing navy/orange layout, controls and typography. Energy 1, rhythm 1, motion 1. Build identity and watch status use the existing row text and error treatment. Reviewed screenshots and logs are kept under ignored `test-results`; browser tests cover enlarged layout and forced colors. Native OS sleep, screen-reader behavior and real Sanctuary loading are not established by these fixtures. The installed-game fixture workflow is recorded in [0.5.0 exit evidence](milestone-0.5.0.md); broader format/adaptor acceptance remains in #30.

Run the standard checks from [DEVELOPMENT.md](../../DEVELOPMENT.md), build the debug executable, then run `node tests/native/local-watch.mjs`. The normal `npm run test:native` sequence includes this test. CI results are recorded in milestone draft PR #56.
