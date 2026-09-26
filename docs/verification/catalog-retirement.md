# Legacy catalog retirement

The owner authorized removal of the obsolete GitHub catalog under #89, without a migration, compatibility window or old-publication availability requirement. Registry consumers and isolated native acceptance are implemented on PR #78. This record identifies the retirement targets; it does not expose private keys or credential values.

## Publication resources, 26 September 2026

Inventory found workflow `360619810` at `.github/workflows/catalog.yml`, environment `catalog` containing only `CATALOG_SIGNING_KEY`, environment branch policy `59557610` allowing `main`, repository variable `CATALOG_PUBLICATION_ENABLED`, and publication branch `codex/catalog-published` at `b7f80667210c43058b98d9ff1f975c81ea90abb9`. No catalog run was active or queued before cleanup.

The workflow is now `disabled_manually`. The catalog environment, its key and branch policy, the repository enable variable and the publication branch were deleted. Post-cleanup inventory contains only the `release` environment and no repository variables. That environment still contains `APP_UPDATE_SIGNING_KEY` with the unchanged update timestamp `2026-09-16T17:19:49Z`. Application releases, update metadata, release workflow and repository protections were not changed. Local signing backups and original mod sources were not read or removed. Git history was not rewritten.

The branch removes the publisher workflow, publishing/renewal scripts, catalog signing example and catalog-only validator. Required CI retains Rust/frontend/runtime/native checks, independent application signature rejection and executable builds; it no longer invokes the retired catalog validator. Historical publication instructions are explicitly retired. The last source snapshot is [53bbac7](https://github.com/Mastervoliumpl/Starframe/tree/53bbac7bf5f0c9ff318f50dfa14abe77b38de264).

## Checks for the publication increment

All-target Clippy, 34 Python script tests, workflow formatting/actionlint and repository references/privacy checks passed. Searches found no publisher/signing/validator consumer in active scripts, CI or Rust source. This increment changes publication tooling and documentation, so unchanged app-update/native registry evidence is retained. Dependency head `53bbac7` passed audits 36268744813; required checks 36268745066 were cancelled by the subsequent push. Publication head `f4cfa29` passed required checks 36269127797 and audits 36269127613. A later final head needs its own complete gate.

## Desktop consumer removal

The desktop game worker no longer loads or polls the old refresh client, and the app updater no longer waits on it. Snapshot and management responses omit obsolete catalog/advisory fields. The frontend removes the dead catalog mode, direct-download retry and advisory-history controls. My mods uses exact installed registry/local references, persisted tested-build metadata and retained registry block reasons. Fixed opener routes reject old catalog/advisory links; author-source opening uses authenticated registry metadata.

The browser fixture now contains native display records and exact references. Its shared-collection transfer uses a synthetic registry-install operation rather than the obsolete prepare command. Blocked and cleared registry-decision cases replace old advisory-state cases; both retain disable/uninstall, compatibility warnings, keyboard focus and Settings access. No legacy fixture fallback remains in frontend source.

Affected checks: frontend formatting/lint/types/build and 19 Vitest tests; Rustfmt, all-target Clippy, 172 library tests (seven documented ignored), 11 executable tests and the configuration/headless/runtime-contract suites; 16 browser cases across registry, security, local import, collections, sharing and Downloads. The embedded debug build and native registry/local-import/updater cases verify the changed worker and wire boundary. Installer, C# and real-game behavior are unchanged, so their prior evidence is reused.

## Direct-transfer removal

The old unauthenticated author HTTP client, its retry/redirect downloader, catalog prepare queue entry, catalog completion branch and headless `prepare-package` command are removed. Package actions now accept only local import, cancellation and list; registry downloads enter the separate authenticated signed-approval boundary. Local verification rechecks retained registry hash blocks before completion. There is no old-ID mapping or alternate network reader.

Local layout and ID validation now live under `local_import`, with the same path/DLL/type constraints. ZIP extraction still verifies exact bytes, expansion/file limits, Windows path aliases, links, junctions and content before promotion. The archive/path mutation smoke remains; the obsolete catalog parser corpus is removed. Catalog-only transfer/freshness/storage tests are replaced by the existing signed-registry queue/transport and local atomic-commit/recovery cases. A local fixture now checks three concurrent workers, request idempotency, cancellation before commit and the next available queue slot. Obsolete prepare wire/CLI actions are rejected.

This increment passed frontend gates/19 Vitest, Rustfmt/Clippy/166 library tests (seven documented ignored), 11 executable tests and configuration/headless/runtime-contract suites. The embedded debug build and native exact-reference packages, signed registry, local import and source-watch fixtures cover the changed queue. Browser layout, updater, installer, C# and game acceptance from the prior increments are reused because their implementation is unchanged.

## Local metadata and ordering

Local activation and ordering now read exact local metadata directly. The catalog release-to-manifest adapter, ordering wrapper and payload-advisory gate are removed. The stable dependency/priority graph and registry/mixed ordering remain. Local setup checks retained registry decisions before accessing prepared content.

A replacement isolated test retains a registry archive-hash block across restart, permits disable, rejects enable/setup, and accepts an explicit later clear without changing the source. Signed decision ingestion and rollback remain covered separately. The old catalog payload-hash advisory format is retired; this test does not claim detection across renamed or repacked payloads.

Rustfmt, all-target Clippy, 165 library tests (seven documented ignored), 11 executable tests and configuration/headless/runtime-contract suites passed. The embedded debug build passed. The affected fake-game source-watch fixture covers changed local activation and ordering. Frontend, registry transport, installer and C# behavior are unchanged, so prior evidence is reused.

## Client, storage, source and dependency removal

The unreferenced catalog reader, TUF authentication/refresh/advisory modules, storage cache/security API, catalog source files and embedded RSA root are removed. Catalog origin is no longer a supported local reference. `tough`, test-only `aws-lc-rs` and 20 other catalog-only lockfile packages are removed; no dependency version was added. The exact Windows dependency notices are refreshed and checked.

Schema 20 creates current local/registry storage directly. Schema 19 cleanup first backs up records, then removes obsolete catalog library rows, tables and direct-download history in one transaction. Current native collections and their selection, registry installation/trust/security/receipt records, local metadata/watch state, prepared content, app-update preferences and deployment journals remain. Tests compare those retained rows and normal restart/restore. No old ID or export is converted. Schemas before 19 and legacy root databases are rejected with a clear error and their files retained. Original sources, settings, artifact bytes and game files are not removed. Old unused trust folders are retained as unrecognized data; no reader opens them.

The Turso converter, historical migration chain and obsolete conversion/installer fixtures are removed. Current integrity/foreign-key/reference validation, bounded writer contention, backups, empty-destination restore, stale writes and transaction/migration rollback remain. Subprocess tests terminate current startup and backup workers before/after completion, and transaction/migration workers before commit. Interrupted attempts retain evidence and recover committed state. Historical documentation links resolve to the retained source snapshot rather than removed files.

Local checks passed: Rustfmt/all-target Clippy, 144 library tests (six documented ignored workers/fixtures), 11 executable tests and configuration/headless/runtime-contract suites; frontend formatting/lint/types/build and 19 Vitest; 34 script tests; exact dependency-notice validation; PowerShell installer-fixture syntax. The embedded debug executable passed. All eleven isolated native Windows cases passed against the rebuilt executable, including current storage/restart, signed registry install/receipts/offline/revocation, local import/watch, collections, cleanup and separate signed updater. The new obsolete-storage test initially expected diagnostic progress without starting its workload; the fixture was corrected and the full native gate passed. Installer, C# and gameplay acceptance are reused only for their unchanged paths. The old installer schema-12 conversion mode is removed, while ordinary NSIS metadata upgrade, repair and retention cases remain.

Prior metadata head `9c1bad4` passed required checks 36270878411 and audits 36270878172. Transfer required run 36270411114 was cancelled by the metadata push; audits 36270410845 passed. Pending/cancelled runs are not passing final evidence.

## Activation format retirement

Both Rust and C# activation readers now reject catalog sources. Activation schema 3 remains for current local preparation; schema 4 carries registry/mixed Code identities. Obsolete schemas 1/2 have no reader. The empty launch/setup document and runtime staging/preflight use schema 3 with explicit omitted-inventory count. Runtime fixture preparation uses canonical local content IDs; no catalog fallback remains in fixture tooling.

Shared positive/negative safety fixtures now use local identities. Catalog sources in schemas 3/4 and obsolete schema 2 have explicit rejection cases. Bounds, path aliases, overlapping roots, exact dependencies, file hashes, source identity and process-bound reports retain shared Rust/C# coverage. Managed and Lua lifecycle tests run Starframe's inert fixtures, including changed bytes and dependency failure; no game is launched.

The changed Rust gate passed its 144 library/11 executable/configuration/headless cases; after correcting the oversized boundary generator, all three runtime-contract integration cases passed. C# formatting/build and 113 tests passed using the prepared pinned SDK. A payload-failure fixture now recalculates its local content ID when changing the declared hash, preserving the intended on-disk hash rejection. All 35 Python script tests passed; fixture canonicalization matches the shared expected content ID. The rebuilt embedded debug executable passed affected fake-game activation and full cleanup/retry/retention cases. These fixture corrections do not weaken production validation.

## Final acceptance

Catalog client/publication/source/dependency removal is implemented. The affected activation and cleanup checks pass locally. The current desktop executables and provenance-recorded runtime/resources form a local review kit. The exact candidate hosted gate remains; build/gameplay limits and closeout are recorded in [milestone acceptance](milestone-0.7.0.md). Application-update signatures retain their independent verifier, fixture and native updater evidence. Production registry provisioning and gameplay limits remain separate from this local acceptance.

Storage head `208d1f5` passed audits 36271841987. Required run 36271842325 passed repository/frontend/C#/Rust checks and ten native cases, then timed out in the updater startup assertion after five seconds while its view still held initial state. The startup assertions now use the same bounded 30-second wait as the other asynchronous updater checks; signature and network assertions are unchanged. The isolated updater case passes locally. The replacement head must pass the full hosted gate.
