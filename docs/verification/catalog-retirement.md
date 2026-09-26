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

## Remaining implementation

The unreferenced old client modules, cache/security storage, source files and dependencies still need removal. They are separate from the completed remote publication cleanup. Registry fixtures use fresh isolated storage and no seeded old catalog; the native signed discovery/install/receipt path passes. App-update verification has separate signed fixtures and passed the native updater case in the preceding increment. #89 and final acceptance #90 remain open until the entire reader and its affected checks are complete.
