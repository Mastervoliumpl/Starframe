# SQLite verification and recovery

Milestone 0.1.1, issues #40 and #41. The development application uses rusqlite 0.40.2, default features disabled, bundled plus backup. libsqlite3-sys 0.38.2 embeds SQLite 3.53.2. No system SQLite installation, network database or Turso runtime is required. Exit build measurements and CI results will be recorded here before completion.

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
