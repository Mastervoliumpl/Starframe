# Legacy catalog retirement

The owner authorized removal of the obsolete GitHub catalog under #89, without a migration, compatibility window or old-publication availability requirement. Registry consumers and isolated native acceptance are implemented on PR #78. This record identifies the retirement targets; it does not expose private keys or credential values.

## Publication resources, 26 September 2026

Inventory found workflow `360619810` at `.github/workflows/catalog.yml`, environment `catalog` containing only `CATALOG_SIGNING_KEY`, environment branch policy `59557610` allowing `main`, repository variable `CATALOG_PUBLICATION_ENABLED`, and publication branch `codex/catalog-published` at `b7f80667210c43058b98d9ff1f975c81ea90abb9`. No catalog run was active or queued before cleanup.

The workflow is now `disabled_manually`. The catalog environment, its key and branch policy, the repository enable variable and the publication branch were deleted. Post-cleanup inventory contains only the `release` environment and no repository variables. That environment still contains `APP_UPDATE_SIGNING_KEY` with the unchanged update timestamp `2026-09-16T17:19:49Z`. Application releases, update metadata, release workflow and repository protections were not changed. Local signing backups and original mod sources were not read or removed. Git history was not rewritten.

The branch removes the publisher workflow, publishing/renewal scripts, catalog signing example and catalog-only validator. Required CI retains Rust/frontend/runtime/native checks, independent application signature rejection and executable builds; it no longer invokes the retired catalog validator. Historical publication instructions are explicitly retired. The last source snapshot is [53bbac7](https://github.com/Mastervoliumpl/Starframe/tree/53bbac7bf5f0c9ff318f50dfa14abe77b38de264).

## Checks for the publication increment

All-target Clippy, 34 Python script tests, workflow formatting/actionlint and repository references/privacy checks passed. Searches found no publisher/signing/validator consumer in active scripts, CI or Rust source. This increment changes publication tooling and documentation, so unchanged app-update/native registry evidence is retained. Dependency head `53bbac7` passed audits 36268744813; required checks 36268745066 are still pending. A later final head needs its own complete gate.

## Remaining implementation

The old runtime reader, cache/security plumbing, source files and dependencies still need removal. They are separate from the completed remote publication cleanup. Registry fixtures use fresh isolated storage and no seeded old catalog; the native signed discovery/install/receipt path passes. App-update verification has separate signed fixtures and passed the native updater case in the preceding increment. #89 and final acceptance #90 remain open until the entire reader and its affected checks are complete.
