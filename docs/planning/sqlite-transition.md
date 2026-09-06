# SQLite transition plan

Decision: select bundled SQLite through rusqlite for milestone 0.1.1, before 0.2.0. The user delegated this choice on 6 September 2026. This replaces the Turso preference on workload and dependency grounds; it does not invalidate the successful Turso checks in issue #9. The 0.1.1 development application now uses bundled SQLite. Production conversion and native checks pass locally; exit measurements and CI remain in progress. See [current verification and recovery](../verification/sqlite.md). Planning is complete. The user authorized implementation on 6 September 2026; VERSION is 0.1.1-dev.1. See the [issue #39 proof](../verification/sqlite-conversion.md).

## Why switch

The accepted product has one desktop database owner, short local transactions and ordinary relational records. Catalog refresh uses HTTP/JSON; collection sharing uses portable JSON files. Downloads and imported binaries stay in files. The C# runtime reads prepared manifests and never opens the desktop database. Native game support in 0.7.0 changes the integration adapter, not that ownership model.

SQLite covers these requirements, including constraints, joins, ordered reads, immediate transactions, migrations and recovery. There is no accepted requirement for database replication, cloud accounts, vector queries, change-data capture or concurrent writers. No specified post-0.7.0 feature needs them either. The unpublished game API is an integration uncertainty, not a reason to couple the desktop database to Turso. A future product change introducing a shared remote database would require its own decision.

Use rusqlite 0.40.2 as the evaluated baseline, default features disabled, with bundled and backup. Pin the implementation and commit its lockfile after Windows validation. Bundled SQLite compiles its C source and links it into the application; users need no SQLite installation or service. Use the existing background worker and bounded request queue for synchronous database calls. Keep SQL and validation in storage.rs; no ORM, connection pool or second async database layer.

The work from 0.1.0 remains useful: record types, SQL constraints, revision checks, ordered migrations, ownership, recovery tests and the native state interface. Most engine-specific code is in storage.rs; game_service.rs now calls synchronous methods from its existing blocking worker. Switching before catalog, deployment and import records grow limits conversion scope.

## Dependency evidence and limits

A metadata-only comparison at e33dbef400d4708657643b4721f6d977ade59a5b kept the application dependency declarations and copied lockfile, changing only the database declaration in a temporary manifest. Cargo 1.98.1 resolved the x86_64-pc-windows-msvc graph. See [package comparison](sqlite-dependencies.json).

| Reachable unique package versions, excluding Starframe | Count |
| --- | --- |
| Turso 0.7.2, defaults disabled | 385 |
| rusqlite 0.40.2, defaults disabled, bundled + backup | 283 |
| Removed / added | 107 / 5 |
| Net reduction | 102 (about 26%) |

This graph includes normal, build and development dependencies. It is not the number printed by a particular build step or a count of compiler invocations. Feature unification and target selection affect it. Removed packages include Turso core/sync support, bindgen/clang-sys, vector helpers and several crypto packages. Disabling Turso defaults already did not remove these transitive dependencies in this graph. Added packages are rusqlite, libsqlite3-sys, fallible-streaming-iterator, pkg-config and vcpkg.

No clean-build timing, executable-size improvement or runtime speedup is claimed by this comparison. SQLite's C compilation still has a cost; Tauri, WebView2, frontend work, debug/release profiles, tests and caches remain. The lengthy previous task included repeated CI/debugging work, so its total elapsed time is not a database benchmark. Measure comparable clean and warm Windows builds in the exit issue without weakening the checks.

## Preserve existing data

The current internal build writes schemas 1 through 3, with application_id 0x53544652 and an engine='turso' constraint. Schema 3 also contains the selected game installation. No public installer has shipped, but existing internal data still matters.

Before replacing the engine, prepare populated fixtures using pinned Turso 0.7.2, including WAL-bearing data and completed backups. Verify conversion on isolated copies; never assume SQLite compatibility makes it safe to open the user's original database with another engine.

Issue #39 proves that bundled SQLite can read isolated copies of pinned Turso 0.7.2 schemas 1–3 and their committed WAL. The chosen path copies under the existing source lock, validates the copy, and inserts its records into a fresh canonical SQLite schema. It does not retain the legacy SQL schema or require a shipping Turso reader. Issue #40 integrates this path into startup and backup restoration. Do not ship two runtime engines. Preserve originals, WAL/sidecars, backup markers and artifact directories.

Conversion must preserve library rows, exact origins/hashes/release IDs, collection IDs/names/order/revisions, active collection, database revision and selected game ID/path. Validate ownership, supported schema, constraints, integrity and every logical record before promotion. Use engine='sqlite' and schema 4 in the fresh validated destination. The proof builds schemas 1–3 with their existing constraints, imports all records, and sets the new marker/schema within the same transaction.

Stage the destination separately, retain the source, and define restart behavior for interruption before and after promotion. Repeated startup/conversion must not duplicate records, reset revisions or overwrite an existing destination. Unknown/newer/corrupt data fails with a recovery message while the original bytes remain available. Old Turso backups need a tested conversion path or a documented legacy restore-then-convert path.

Use SQLite's backup API through rusqlite's backup feature, retaining completed-backup and fresh-destination guarantees. Preserve foreign_keys=ON, synchronous=FULL, immediate transactions, bounded busy handling and the one-owner lock. Verify the chosen journal mode and checkpoint behavior; do not copy a live SQLite file as an unverified backup.

## Milestone 0.1.1

Created [milestone 0.1.1](https://github.com/Mastervoliumpl/Starframe/milestone/10) after updating the existing issue plan:

1. [#39: Prove recoverable Turso-to-SQLite conversion](https://github.com/Mastervoliumpl/Starframe/issues/39). Depends on completed #9 and #10. Own legacy fixtures, conversion evidence and restart/rollback rules.
2. [#40: Replace Turso persistence with bundled SQLite](https://github.com/Mastervoliumpl/Starframe/issues/40). Depends on the conversion issue. Own storage.rs, worker call sites, migrations/backup/restore, dependencies and affected tests. Preserve the native state contract.
3. [#41: Verify SQLite recovery and Windows build costs; close 0.1.1](https://github.com/Mastervoliumpl/Starframe/issues/41). Depends on the replacement issue. Own native/recovery evidence, comparable dependency/build measurements, notices and version preparation.

Exit requires preserved populated legacy data and backups, passing storage/game/native checks with SQLite, no Turso in the shipped dependency graph, and recorded build measurements. Prepare 0.1.1 with the existing version tool during implementation/exit; schema and app versions stay separate. No release publication, game-file changes or later features belong to this plan.

Milestone 0.2.0 waits for 0.1.1. Keep completed issues/milestones as historical evidence; append this decision to affected closed issues rather than rewriting their completed criteria.

## Primary sources

- [SQLite's intended uses](https://sqlite.org/whentouse.html): local application storage.
- [rusqlite 0.40.2 API](https://docs.rs/rusqlite/0.40.2/rusqlite/): Rust access and backup API.
- [rusqlite build options](https://github.com/rusqlite/rusqlite/blob/master/README.md): bundled SQLite and pregenerated bindings.
- [Turso features and status](https://github.com/tursodatabase/turso/blob/main/README.md): concurrent writes, CDC, vector support and experimental features.

Sources checked 6 September 2026. The decision uses Starframe's repository and issue requirements; upstream feature lists do not establish application performance.
