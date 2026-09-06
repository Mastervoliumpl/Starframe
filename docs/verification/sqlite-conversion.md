# Turso-to-SQLite conversion proof

Issue [#39](https://github.com/Mastervoliumpl/Starframe/issues/39), 6 September 2026. Milestone 0.1.1 is active at 0.1.1-dev.1. This is an executable test-only proof, not an application migration command. The production storage module still uses Turso.

## Decision and pinned inputs

SQLite can read isolated copies of the tested Turso 0.7.2 databases, including committed WAL records. Select a copy, validate and rebuild conversion for #40. No legacy Turso reader needs to ship in the migrated application for these supported inputs.

The proof uses Turso 0.7.2 with defaults disabled and rusqlite 0.40.2 with bundled/backup, resolved to libsqlite3-sys 0.38.2 and SQLite 3.53.2. SQLite is currently a dev dependency only. The normal app dependency graph still contains Turso and excludes rusqlite.

[Executable proof](../../src-tauri/src/storage/sqlite_proof.rs) is a test-only child module of storage.rs. It reuses the existing migration SQL and record validators. The fixture worker creates schemas 1, 2 and 3 with the pinned Turso engine, writes a completed checkpoint backup, commits additional changes to the WAL, captures a logical snapshot through Turso, flushes and waits. The parent forcibly terminates it before SQLite sees any files.

Fixtures include empty and populated libraries, local/catalog origins, an unavailable catalog artifact reference, Unicode names/paths, empty and ordered collections, active selection, selected game ID/path, and a database revision above JavaScript's safe-integer range. Each fixture has a small artifact file that conversion must not alter. Fixtures are generated in temporary directories, not from personal app data.

## Chosen conversion

1. Require an existing, quiescent source with Starframe's OS file lock, or a completed immutable legacy backup. Reject occupied destinations and destinations nested in the source.
2. Copy state.db and any WAL into staging inside a newly created destination. Reject nonregular/reparse-point input files. Keep all source files and artifacts unchanged.
3. Open only the copy with SQLite, with trusted_schema disabled and query_only enabled. Check application_id 0x53544652, supported legacy schema 1–3, engine='turso', integrity, exact known tables/columns and absence of views/triggers. Read every logical record.
4. Create a fresh SQLite candidate from Starframe's canonical schema and constraints. Insert the validated records in a single immediate transaction; preserve revisions rather than bumping them. Older schemas receive the existing empty preference/game-selection defaults.
5. Validate domain fields, foreign keys and contiguous collection order. Compare every record with the copied source before commit and after reopening. Set engine='sqlite' and user_version=4 within the transaction.
6. Close and flush the validated candidate, rename it to the new destination's state.db, then write and flush the complete marker. The proof uses DELETE journal mode for this standalone candidate; the application journal policy remains part of #40.

The proof imports fixed columns into a fresh schema rather than trusting or executing the copied database's SQL definitions. It retains the original source, staged copy and artifacts. Completed SQLite output is also backed up through SQLite's backup API and reopened for record comparison.

## Restart and recovery rules

| Interruption | Result and next action |
| --- | --- |
| After database copy but before WAL copy, or after both copies | Source remains unchanged. Partial staging has no promoted database. Retain it for diagnosis and retry into a new empty destination. |
| During the import transaction | The unfinished candidate is never promoted. Source remains usable; retry from it in a new destination. |
| After validation, before promotion | A validated candidate remains in staging. The proof conservatively rejects reuse of the occupied destination; retry from retained source. |
| After promotion, before complete marker | A complete SQLite database exists but completion is unconfirmed. Do not reset or overwrite it. Validate it before any explicit recovery/promotion decision in #40; a fresh retry remains possible from source. |
| After the complete marker | The new SQLite file is valid. A repeated conversion into that destination is rejected without changing it. |

The proof promotes only inside a separate new directory. It does not replace the live app-data directory or copy artifacts into a new app root. Issue #40 must integrate this into startup while retaining the original database/WAL, artifact location and rollback evidence. Automatic in-place promotion is not established by this test-only tool.

Existing completed Turso backups pass the same conversion after marker validation. Incomplete backups are rejected and retained. Unconverted originals still use the [Turso recovery procedure](storage.md). Converted files must not be opened with the old application.

## Verification

Five parent tests exercise the conversion. The two ignored functions are subprocess entry points invoked by those tests, not omitted recovery checks.

| Check | Evidence |
| --- | --- |
| Legacy schemas and optional data | Six generated fixtures: schemas 1–3, each empty and populated. Each live WAL source and its completed backup converts, for twelve successful conversions. |
| Exact preservation | Every target record matches a snapshot independently read through Turso. Source trees, including WAL, artifacts and backups, compare byte-for-byte before/after. This is stronger than comparing file hashes alone. |
| SQLite backup | Every successful target is backed up through the SQLite backup API; reopened backup records match. |
| Uncommitted legacy WAL | A pinned Turso writer is killed with flushed unfinished deletes/revision changes. SQLite conversion retains the committed snapshot. |
| Failure boundaries | Child conversion processes are forcibly terminated at six boundaries: partial copy, complete copy, transaction, validated candidate, promotion and completion. Retry into a fresh destination succeeds. |
| Invalid input | Newer schema, wrong owner/engine, broken order, orphan preference, invalid game path, negative revision, missing preference singleton, unknown table/column/trigger, corrupt header and incomplete backup fail without source reset. Specific validation errors are asserted for the structured malformed records. |
| Ownership and retry | An active writer, nested destination and occupied destination are rejected. Repeated conversion retains the completed destination exactly. |
| Offline | The Rust suite can run with --offline --locked after dependencies are cached. There is no remote database. |

Run:

~~~powershell
cargo test --manifest-path src-tauri/Cargo.toml --offline --locked --lib storage::sqlite_proof
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --offline --locked
cargo build --manifest-path src-tauri/Cargo.toml --locked
~~~

Local validation passed on Windows 11: Rustfmt, Clippy with warnings denied, the full offline Rust suite, the Windows development build, 12 repository/version regression tests, repository links/XML checks and package metadata formatting. GitHub checks are recorded on the pull request; this local evidence does not substitute for hosted CI.

## Limits and follow-up

This establishes compatibility for the pinned engine and Starframe schemas tested on Windows. It does not establish compatibility with arbitrary Turso extensions, unknown schemas or future engine versions. Physical power loss, disk-full faults, hostile concurrent directory replacement and actual startup promotion need their own handling/verification in #40/#41.

No user or game data was opened or changed. The app has no conversion UI/command yet. Production SQLite backup/journal policy, removal of Turso, native startup/recovery integration and comparable build timings remain #40/#41. Preserve the pinned legacy fixture definitions/provenance when #40 changes the application migrations; do not regenerate “legacy” fixtures from the new schema.

The first local build after the repository move found stale Tauri permission paths in cached build output. Refreshing the affected Tauri packages fixed that environment issue. It was not a database compatibility failure.
