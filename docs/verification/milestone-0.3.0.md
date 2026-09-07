# Milestone 0.3.0 exit evidence

Version 0.3.0 completes the internal curated-mod workflow in issues #16–#19. The owner authorized finishing the last issue and merging the milestone into main. Required hosted checks must pass on the final PR revision before that merge. This is an internal build with an empty metadata-only catalog, not a public installer or app release.

## Delivered

- [Catalog](catalog.md): validated exact release identities, independent refresh, offline cache and atomic metadata replacement. [Schema additions](../../catalog/README.md) distinguish maintenance, compatibility findings and withdrawal reasons.
- [Package preparation](packages.md): bounded downloads, hash verification, guarded ZIP extraction, immutable library content, cancellation and persistent failures.
- [Mod lifecycle](mods.md): revision-checked enable/disable, automatic stopped-game deployment, streamed recovery, uninstall confirmation, settings retention and retryable cleanup.
- [Management screens](management.md): live lists/details, search and selection, bulk actions, source links, compatibility messages, download progress and exact-release retry.
- [Dependency checks](dependencies.md): separate npm, RustSec and NuGet audits. Signing and broader security work follow [the security scope](../../SECURITY.md).

## Verification and limits

Local feature verification passed 72 Rust checks, 10 frontend tests, 13 browser checks and the complete Windows native suite. Four ignored Rust worker entry points run through parent interruption tests. Frontend formatting/lint/types, Rustfmt/Clippy and native debug builds passed. The native search fixture recorded a 39.7 ms p95 over 100 searches at 150% Windows display scale. Native list/details screenshots were inspected; browser checks covered 200% text, reduced motion, forced colors and 1,200 fixture releases with 100 rendered rows per page.

The final synchronized 0.3.0 version passed the locked, optimized Windows Tauri build and 18 Python repository/version tests. Hosted checks repeat the browser, native, Rust and C# suites on the final PR revision. Their successful result is required before merging.

Native UI controls enable an inert installed fixture, defer changes while a copied test process is running, apply the saved revision on restart and uninstall while retaining settings. Package tests separately exercise HTTP transfers, integrity failures and interrupted preparation. Existing runtime evidence establishes managed fixture activation inside the playtest. These checks do not establish arbitrary author-mod compatibility or guarantee malware-free code.

The merge publishes only the empty catalog metadata at the fixed raw GitHub endpoint. No author archive is hosted or silently substituted. Catalog authentication and advisory delivery remain requirements before opening a live catalog to general users. Windows/updater signing remains distribution work; no signing key or YubiKey configuration changes are included.

The storage schema is 8. Deployment recovery content is retained without automatic garbage collection. A database-only backup does not include external artifact/recovery files. Named collection editing, ordering/sharing and local imports retain their 0.4.0/0.5.0 scope. Conventional BepInEx plugins, maps and Lua still need verified adapters; AI support remains deferred.
