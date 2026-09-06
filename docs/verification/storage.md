# Local storage verification and recovery

Issue [#9](https://github.com/Mastervoliumpl/Starframe/issues/9), checked on Windows 11 Education 25H2, build 26200, on 6 September 2026. Product version remains `0.1.0-dev.1`; database schema is version 2. The implementation is local and unreleased.

This report records original Turso verification, not pending SQLite work. The [0.1.1 transition decision](../planning/sqlite-transition.md) supersedes the engine preference below. This Turso recovery procedure still applies to unconverted 0.1.0 data; Use the [SQLite verification and recovery procedure](sqlite.md) for 0.1.1 and later.

## Engine decision

Use **Turso 0.7.2**, pinned exactly, with default features disabled. The required checks passed on Windows, so the documented SQLite fallback was not selected. The app uses `Builder::new_local` and has no database account, token, service or synchronization configuration. It does not use the older libSQL client. [Pinned Rust API](https://docs.rs/turso/0.7.2/turso/), [local builder](https://docs.rs/turso/0.7.2/turso/struct.Builder.html).

The engine's compatibility claims were not used as proof of durability. Tests exercised the actual schema, transactions and files through the pinned engine. Re-run them before changing the engine version or introducing additional SQL features.

## Implemented records and ownership

All SQL and ordered migrations live in [storage.rs](../../src-tauri/src/storage.rs). There is no ORM. Schema 1 stores library entries, named collections, ordered exact mod references and the database revision. Schema 2 stores active-collection selection. Collections have their own edit revisions. Stale revisions fail without changing records. Database revisions sent to the frontend are decimal strings.

References retain mod identity, origin, content hash and a catalog release ID where applicable. An unavailable artifact can remain in a collection; collection membership does not require a library row. Local imports cannot claim a catalog release ID. Collection entries have unique positions and mod identities. They contain no mod settings. The library's content locations derive from validated lowercase SHA-256 values under `artifacts/`; binary content is not stored in the database.

The desktop resolves its per-user local app-data directory through Tauri, opens the database in a blocking worker and retains one connection. An OS file lock prevents a second storage owner. SQL foreign keys are enabled, synchronous mode is `FULL`, and the connection uses a bounded busy timeout. No experimental multi-process WAL, MVCC, attachment or encryption mode is enabled.

The current UI reports storage readiness, counts and the saved active collection. Library/collection editing controls, catalog records, deployment records and game discovery are later work. Startup errors stay visible without replacing saved data or disabling unrelated navigation.

## Evidence

| Check | Result |
| --- | --- |
| Required SQL | Primary keys, foreign keys, unique/check/not-null constraints, joins, ordered reads, parameter binding, upsert, immediate transactions and version pragmas passed. |
| Restart persistence | Library metadata, local/catalog origins, exact hashes, collection order, active selection and revisions survived close/reopen. |
| Failed edits | Stale revisions, repeated mod identities, invalid active-collection references and malformed hashes were rejected. Failed multi-statement edits retained the previous records and revision. |
| Ordered migration | A populated schema-1 database upgraded to schema 2 without losing library records. A backup was created first. |
| Failed migration | A migration that created a table and then executed invalid SQL rolled back its table and schema version. The pre-migration backup restored into a new directory and upgraded successfully. |
| Forced termination | Three child-process cases passed: unfinished record transaction, unfinished migration, and a committed writer. The worker flushed dirty pages to the WAL before the parent forcibly killed it. Committed records survived; unfinished changes were absent after reopening. |
| Busy/ownership | A second storage owner was rejected. A competing transaction through the same engine returned within the test's two-second bound; the first connection remained usable after rollback. |
| Backup restoration | A checkpointed database and WAL copy restored into a new directory. Incomplete backups and occupied destinations were rejected. Original data and backups were retained. |
| Invalid data | Corrupt headers, newer schema versions and unrelated databases were rejected. Corrupt/newer fixture bytes remained unchanged. |
| Native UI | Fresh startup and restart loaded saved data. Corrupt/newer files produced recovery messages while navigation and diagnostics continued to work. Every native test used a temporary data directory. |
| Offline execution | The storage suite passed with `cargo test --offline --locked --lib`; no remote database was configured. |

The storage suite has five parent tests. Its one ignored test is a child-worker entry point invoked three times by the process-interruption test. It is not an omitted recovery check. Existing frontend, Rust state/configuration and native interaction checks also passed. The Windows release executable builds with the embedded engine; no installer was produced.

The native checks use the debug executable so `STARFRAME_TEST_DATA_DIR` can isolate each run. Release builds ignore that override and use Tauri's app-data path. The updated GitHub workflow builds both variants; hosted-runner execution remains unverified until the branch is pushed.

These checks establish process-interruption recovery on this machine. They do not simulate physical power failure, disk-full faults, failing hardware, antivirus interference or every Windows filesystem. Backups cover records, not artifact files. Installer/update lifecycle checks remain later work.

## Recovery procedure

1. Close every Starframe window. Keep the complete original data directory, including `state.db` and `state.db-wal` if present. Do not open these files with a different database engine.
2. For a newer-schema message, use the matching newer Starframe version. Do not downgrade or reset the database.
3. For a failed migration or damaged database, locate a directory under `backups/` that contains a `complete` marker. Keep incomplete directories for diagnosis; the restore tool rejects them.
4. From a checkout with the Rust development prerequisites, restore into a new empty directory:

   ```powershell
   cargo run --manifest-path src-tauri/Cargo.toml --locked --example restore_storage -- "C:\path\to\backup" "C:\path\to\restored-data"
   ```

   The tool validates the restored database and prints its library and collection counts. It does not overwrite the current database. If restoration fails, retain the source backup and use another empty destination for a retry.

5. Keep Starframe closed. Retain the original app-data folder under a separate name, then place the verified restored data at the original app-data location. Preserve the original `artifacts/` folder there as well; the restore tool does not copy or verify artifact bytes. Start Starframe and check Saved data in Settings.

Do not delete the original directory or backup until the restored records and any artifact files have been checked. If no valid backup exists, retain the damaged data for diagnosis. The app deliberately provides no silent reset to an empty library.
