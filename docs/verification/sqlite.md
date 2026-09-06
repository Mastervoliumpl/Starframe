# SQLite verification and recovery

Milestone 0.1.1, issues #40 and #41. The 0.1.1 application uses rusqlite 0.40.2, default features disabled, bundled plus backup. libsqlite3-sys 0.38.2 embeds SQLite 3.53.2. No system SQLite installation, network database or Turso runtime is required. The local checks and build comparison below pass. Required CI is the final merge gate on [PR #42](https://github.com/Mastervoliumpl/Starframe/pull/42).

## Storage and conversion

SQL, constraints, ordered migrations and validation remain in [storage.rs](../../src-tauri/src/storage.rs). Synchronous operations run on the existing blocking owner and one-slot request queue in game_service.rs. The native/frontend contract is unchanged. SQLite uses foreign_keys=ON, synchronous=FULL, WAL and a 250 ms busy timeout. Writes use immediate transactions and preserve stale-edit errors.

The data-directory state.lock covers startup, conversion and ordinary use. On first startup, a unique sqlite-staging-UUID directory receives copies of legacy state.db and its WAL. The converter checks application ownership, schemas 1–3, engine, table/column inventory, integrity and records, then imports into fresh constrained schema 4 with engine='sqlite'. It compares every logical record before commit and after reopen, flushes the candidate, writes complete and renames the directory to sqlite. Only then can storage report ready.

Original Turso files, old backups and artifacts remain untouched. Interrupted staging attempts remain available; retry uses a new directory. Once sqlite exists, startup uses it exclusively. Invalid or incomplete SQLite data produces an error and never causes fallback to older legacy data. Do not remove sqlite to bypass a startup error: that can expose stale legacy records.

Backups use SQLite's backup API rather than a live database file copy. Each backup contains state.db and, only after completion and validation, complete. Restore requires an empty destination. It validates a retained copy; legacy Turso backups take the same conversion path. Backups cover records, not artifact payloads.

## Local verification

All 25 regular Rust tests pass offline. Three ignored entries are worker processes invoked by their parent tests. Coverage includes exact references/order/origins, Unicode, revisions above JavaScript's safe range, selected game, constraints, stale edits, transaction rollback, schema upgrades, occupied destinations, incomplete/invalid/newer data, bounded writer contention and completed/incomplete backups.

Pinned Turso fixtures cover empty/populated schemas 1–3, committed WAL, unfinished WAL transactions and completed legacy backups. Tests compare logical snapshots and retained original/artifact bytes. Production child processes are terminated after copy, transaction, validation, candidate promotion, completion marker, immediately before/after the active-directory switch, and before/after backup completion. Restart and restore preserve committed records.

The native suite passed fresh startup/restart, converted schemas 1–3, retained originals, saved-game restart, folder-picker behavior, external process/build observation, corrupt/newer recovery messages and navigation. Navigation and diagnostics also worked while a debug-only gate held storage startup on its worker. The existing desktop interaction check measured 38.1 ms p95 across 100 samples at 150% display scale on this run. Browser, native and build measurements are separate checks.

Frontend formatting, lint, Svelte/TypeScript diagnostics, six frontend tests and the production frontend build passed. Rustfmt, Clippy with warnings denied, Rust tests and a Windows debug desktop build passed. All data and game fixtures were temporary. No personal application data or installed game files were changed.

## Recovery

1. Close Starframe and retain the entire app-data directory, including sqlite, any legacy state.db/WAL, backups and artifacts. Use the matching application version for newer-schema data.
2. Select a backup directory containing complete. An incomplete backup is rejected. Retain failed conversion directories for diagnosis; do not rename an unvalidated candidate into place.
3. Run the recovery example into a new or empty destination:

```sh
cargo run --manifest-path src-tauri/Cargo.toml --locked --offline --example restore_storage -- "<backup-directory>" "<empty-destination>"
```

4. The tool validates the result and prints record counts. It supports completed SQLite and legacy Turso backups. If it fails, keep the backup and failed destination, then use another empty destination for a retry.
5. With Starframe closed, retain the old app-data directory under a separate name. Put the verified restored directory at the original location. Preserve the old artifacts directory there too; this tool does not copy or validate artifact bytes. Start Starframe and check Saved data and the saved game selection.

Retain originals until restored records and artifacts have been checked. These tests cover process termination, not physical power loss, disk failure, disk-full conditions or every Windows filesystem. The app has no reset-on-error path. No installer or release has been published. Mod management, launch and runtime integration remain later milestones.

## Windows build comparison

Measured on 6 September 2026: Windows 11 Education 25H2 build 26200, Ryzen 7 7840HS (8 cores, 16 logical processors), about 31.3 GiB visible RAM. Both runs used Cargo 1.98.1, rustc 1.98.1, LLVM 22.1.8 and host x86_64-pc-windows-msvc. Builds ran sequentially in isolated detached checkouts, with separate initially absent target directories and a populated offline Cargo registry. No other compilation ran during these measurements.

The baseline is e33dbef400d4708657643b4721f6d977ade59a5b. The migrated implementation is 4ba28a27ff367b268138a161ae798964b8dd3846, before the final version-label/documentation commit. [Structured results and package inventories](sqlite-builds.json) include binary SHA-256 values and toolchain identities.

```sh
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --offline --filter-platform x86_64-pc-windows-msvc --format-version 1
cargo build --manifest-path src-tauri/Cargo.toml --release --locked --offline --target-dir <separate-target-directory>
# Repeat the identical build command for the first warm measurement.
cargo tree --manifest-path src-tauri/Cargo.toml --locked --offline --target x86_64-pc-windows-msvc --edges normal,build,dev --prefix none --format '{p}'
```

| Measurement | Turso baseline | SQLite implementation |
| --- | --- | --- |
| Offline metadata resolution, wall seconds | 0.658 | 0.570 |
| Clean Cargo release build, wall seconds | 199.292 | 111.859 |
| First warm command, wall seconds | 36.244 | 31.893 |
| Comparison executable, bytes | 27,538,432 | 11,354,624 |
| Metadata graph package versions, excluding app | 385 | 282 |
| Feature-selected tree, normal/build/dev | 367 | 261 |
| Feature-selected tree, normal/build | 363 | 257 |

The clean compile was 43.9% faster and the comparison executable was 58.8% smaller in this sample. These are raw Cargo release-profile builds with default features, not Tauri production packaging: custom-protocol was not enabled. Prepared frontend bytes were identical and excluded from the timed commands. The actual Tauri production executable is built separately by required CI. Do not distribute the comparison binaries or present their size as the installer size.

Both first warm builds compiled only Starframe; dependencies were cached. A subsequent fingerprint-diagnostic build compiled nothing (Cargo reported 1.10 seconds for Turso and 0.55 seconds for SQLite). The first application-only rebuild was not isolated further, so the first-warm numbers do not establish a typical edit/build cost. This single clean sample does not predict every machine or CI run.

The planning method resolves a broader metadata graph than the feature-selected cargo tree. Repeating that method gives 385 to 282, one fewer than the planned SQLite 283 after removing the unused direct Tokio test dependency. Neither count represents compiler invocations. Both the normal/build tree and the test tree exclude Turso; only legacy fixture names and the conversion engine marker retain its name.

Test execution is separate: the final local Rust run passed 25 regular tests, with 0.74 seconds reported for the storage/game library tests and less than 0.01 seconds for the other suites. Frontend Vitest passed six tests in 1.81 seconds. Native integration, browser checks and total GitHub CI include additional work and are reported through the PR checks, not folded into clean compilation. The original proof CI Windows job took 12 minutes 5 seconds; that job also compiled both engines and ran tests, so it is not the baseline compile measurement.

## Hosted-runner startup race

The first final CI run passed Rust checks but failed before native database checks: WebView2 exposed its CDP connection before the page existed. Both native entry points now wait for the page event when the context is empty. A browser regression covers delayed page creation and an already-open page. This changes test startup synchronization; it does not relax assertions or add a fixed sleep. Final CI is linked through PR #42 and the completed milestone issues.

Rapid local restarts also exposed a port-probe failure with Windows TIME_WAIT sockets and no live listener. The harness now checks listener availability and waits for teardown instead of exclusively binding a recently used port. A real TCP-listener regression covers release, and the repeated native conversion startups exercise the Windows path. The two reserved test ports and release-build restrictions are unchanged.
