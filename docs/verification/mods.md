# Mod lifecycle for issue #18

The internal 0.3.0 backend prepares the active collection through the existing bootstrap/runtime ownership journal. The management screens are still issue #19; no public installer or live user catalog is released by this work.

## Commands and saved intent

The local main window has a focused `mod_action` command: list, set enabled, uninstall and retry cleanup. Mutations identify an exact library mod/hash and expected saved-data revision. Stale requests fail. The command cannot supply a game destination, remote URL or executable command. Package installation into the library remains the existing `package_action` preparation flow.

Enabling creates a Default collection if needed and includes available exact required dependencies. Dependency order is deterministic; missing/disabled dependencies and conflicting versions prevent preparation. Full manual priority and collection sharing remain 0.4.0. Ordinary withdrawal does not block a verified installed copy. Runtime inventory includes disabled library mods, but only enabled payloads are deployed. Existing runtime contract limits still apply, including 256 inventory entries, 1,024 files per mod and the 1 MiB activation document limit.

Membership changes commit on the storage worker. They apply automatically once setup has installed the runtime and the game is observed stopped. While the game runs, edits remain saved and the current deployment stays intact. If Starframe closes first, the next startup applies the saved revision. Failed automatic preparation is not retried in a write loop; edit the setup or use the existing setup/retry control. No separate Apply action or background service is added. The single storage/deployment worker serializes disk work and commands; requests during a file operation wait for that work or receive a busy result when its bounded queue is full. Navigation remains independent.

Uninstall reports affected collections and requires the caller to acknowledge them at the current revision. It removes the active membership and library row atomically; other collections retain unresolved exact references. Schema 8 records pending artifact cleanup. Cleanup removes only known unchanged library files, refuses unexpected content/links, retains failures across restarts, and can retry after locks are released. Shared artifacts remain while another library entry uses them. Settings and local source folders are outside cleanup. Game copies are removed by normal stopped-game deployment, not by library cleanup.

## File handling and space

The preparation plan revalidates the complete library file set and builds a validated activation manifest. Mod files are streamed into SHA-256-addressed recovery content under app data, then through the existing journal into the game. Existing small bootstrap backup blobs remain readable in SQLite. Large mod content is not collected into one in-memory payload or stored as large SQLite blobs. Replacement and recovery verify the expected identities and reject changed/unowned conflicts. Limits are 512 MiB per file and 2 GiB per deployment, including runtime files.

Downloads conservatively budget the approved archive and maximum 2 GiB expansion for each concurrent preparation. Extraction rechecks its declared expanded size. Deployment checks the app-data backup volume and game volume, and rechecks game-volume space after the journal/SQLite writes. Estimates include a 64 MiB working margin; they do not reserve space against other applications. Failed writes still use normal recovery.

Recovery content and old bootstrap backup blobs are retained for repair; automatic garbage collection of old recovery content is not implemented. Database-only backups do not include library artifacts or external deployment-content files. Preserve those data directories alongside database backups when recovery depends on them. A missing recovery file causes a repair error rather than silent data replacement. Abrupt power-loss durability and broad fuzzing are not established by the process-interruption fixtures.

## Verification

- Rust lifecycle fixtures cover exact dependency order, stale edits, withdrawal, retained disabled inventory, changed library bytes, uninstall confirmation, other collection references, settings retention, failed SQL transactions and locked-file cleanup across restart.
- Streamed deployment fixtures cover a 9 MiB payload, interrupted update rollback, reapplication/removal, changed source hashes and corrupt recovery content. Existing process-termination, unknown-file conflict, junction and destination-lock fixtures exercise the same journal.
- Disk-space tests check working-margin boundaries, overflow rejection and the Windows free-space API. Failure-boundary and lock tests do not claim to fill a real disk.
- The native fixture uses a temporary Steam/game installation, inert runtime/mod bytes and the verified official bootstrap archive. A copied Windows ping executable supplies process observation; no Sanctuary or author mod is launched. It checks native command permissions, installed withdrawal, rapid toggles during play, desktop close, next-start application and uninstall retention.

Run `node tests/native/mods.mjs` after building the debug desktop. Python is needed only to prepare the pinned bootstrap fixture; `STARFRAME_TEST_PYTHON` selects an installed interpreter and `STARFRAME_TEST_BOOTSTRAP_ARCHIVE` selects an already-downloaded archive. Without an archive override, the existing preparation script fetches and verifies the official archive. These options belong to the test process, not the release app.

Verified on Windows on 7 September 2026:

- PASS: 70 Rust tests across library, application, configuration and runtime contracts. Four ignored process-worker entry points run through their parent tests.
- PASS: Rustfmt, Clippy across all targets with warnings denied, and locked Windows Tauri debug/release builds.
- PASS: the complete native suite, including the mod lifecycle fixture and existing desktop, storage, game, catalog and package checks.
- PASS: frontend formatting, ESLint, Svelte/TypeScript checks, six Vitest tests and the production build; 18 Python tests and repository/version checks.

These were the original local checks for #18 before publication. [Screen integration](management.md) and [milestone exit evidence](milestone-0.3.0.md) record the subsequent #19 checks and merge gates. The fixture does not establish compatibility with a real author mod.
