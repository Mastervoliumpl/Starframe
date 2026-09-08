# Changelog

## Unreleased

## 0.5.0 - 2026-09-08

- Verify the local developer workflow in Sanctuary: partial DLL rejection, queued rebuilds, desktop restart, latest game launch, retained settings, exact sharing and source-preserving uninstall. Add [local development instructions](docs/local-development.md) and [milestone evidence](docs/verification/milestone-0.5.0.md) (#26).

- Follow imported local sources while the desktop is open. Verify settled snapshots, retain the previous build on invalid output, and advance matching active local collections. Queue game-file changes until the game closes. Schema 12 persists the watched source; startup/resume reconciliation and bounded fallback scans recover missed notifications. Shared and inactive collections retain exact builds; see [watcher behavior and checks](docs/verification/local-watching.md) (#25).
- Import local DLLs and build output folders into managed storage, retain source metadata in SQLite schema 11, and use normal enable, order, details, collection and uninstall controls. Local builds use exact content references without catalog update or game-version checks. Uninstall retains the author's files. See [supported inputs and checks](docs/verification/local-imports.md) (#24).

## 0.4.1 - 2026-09-08

Internal correctness and maintenance milestone; no published installer or app release. See [verification](docs/verification/milestone-0.4.1.md).

- Extract worker request handling and testable launch state, archive preparation, shared filesystem guards, legacy storage conversion and large inline test modules. Generate desktop management wire types with the existing ts-rs dependency and check shared Rust/C# contract boundaries (#54).

- Give each logical mod a deployment root derived from its mod ID and archive hash. Mods can share cached bytes and activate together; uninstall retains artifacts referenced by another mod (#53).

- Reject packages above the runtime's 1,024-file, 64 MiB file, 16 MiB assembly or 256 MiB package and activation limits before success. Cached reuse and enablement apply the same checks; older oversized inventories remain readable for repair (#52).

- Bound the runtime inventory independently of library size, retain all active entries, and report omitted disabled mods in the in-game menu. Activation schema 3 adds the omitted count; readers retain schema 2 support (#51).

- Preserve distinct approval references over shared archive bytes, including exact enable/uninstall selection and collection references. Schema 10 retains existing records and deduplicates local content (#50).

## 0.4.0 - 2026-09-08

Internal ordered-collection milestone; no published installer or app release.

- Verified mixed cached/missing sharing across two libraries, actual game order, compatibility warnings, game-exit application and per-mod settings retention (#23). See [exit evidence and the four known 0.4.1 limits](docs/verification/milestone-0.4.0.md).
- Added ordering limits and recovery guidance to Help & logs.

- Added version-1 collection JSON import/export, review before one acceptance, verified local reuse, queued exact approved packages and persistent unresolved references (#22).
- Added schema-9 import progress, restart recovery and retry; incomplete imports cannot apply. Collection dialogs use the existing management dialog styles.

- Added named collection creation, rename, confirmed deletion and active selection, sharing existing mod files and settings (#21).
- Added visible pending collection revisions during play, with latest-revision application after exit.
- Replaced Move up/down buttons with a drag handle and right-aligned order number; the handle retains keyboard and select-then-place operation.
- Added deterministic dependency ordering, mandatory before/after constraints, optional preference warnings and separate requested/effective priority (#20).
- Added keyboard and drag reorder controls, revision checks, adjustment messages and visible Lua collision winners.
- Added verified Lua overlays for existing game directories, with later effective order winning. Maps, AI packages and conventional BepInEx plugins remain unsupported.
- Excluded .NET build output from Vite's watcher after a Windows file-lock failure during concurrent checks.

## 0.3.0 - 2026-09-07

Internal curated-mod milestone; no published installer or app release. The metadata-only catalog starts empty.

- Added live My mods, Catalog and Downloads screens with search, details, selection, bulk actions, separate enable switches and confirmed uninstall (#19).
- Added distinct maintenance, compatibility and withdrawal metadata, author/source links and persistent exact-release download failures. Compatibility warnings allow enable/launch.
- Added mod enable/disable/uninstall commands, exact dependency preparation, automatic stopped-game deployment and schema-8 uninstall cleanup (#18).
- Added streamed mod deployment/recovery content, disk-space estimates and lifecycle/lock/interruption fixtures. Ordinary withdrawal preserves verified installed copies and settings.
- Added separate scheduled/dependency-change audits and a phased security plan. Catalog/advisory signing and Windows release signing remain pre-distribution work.

- Added bounded downloads, exact hash verification, guarded ZIP extraction and immutable package preparation for approved releases (#17).
- Added schema-7 package history, atomic library completion, cancellation, verified-content reuse and interruption recovery.
- Added catalog schema 1, stable release validation, exact artifact metadata and withdrawal checks (#16).
- Added independent catalog refresh with conditional HTTP requests, bounded transfers, automatic retries and a validated SQLite cache. Catalog changes preserve installed releases and collection references.
- Added live catalog check status and catalog validation to the required Windows CI job. The initial metadata file contains no approved releases.

## 0.2.0 — 2026-09-07

Internal runtime milestone; no published installer or app release.

- Added the .NET Standard runtime contract library, matching Rust validation, shared fixtures and internal lifecycle interfaces (#11).
- Added locked C# builds, formatting/analyzers, fixture CI and synchronized informational versions.
- Added pinned BepInEx bootstrap preparation, journaled installation/removal, schema-5 deployment backups, process guards and restart recovery (#12).
- Added the BepInEx entry plugin, ordered managed activation, failure propagation and process-bound reports (#13). Activation schema 2 represents content-only packages without executable metadata; their activation adapters remain unsupported.
- Verified fixture activation inside the current playtest. The core runtime targets .NET Standard 2.0 with explicit Mono dependencies; its Unity entry plugin targets 2.1.
- Added internal typed settings registration, per-mod BepInEx persistence with atomic replacement, and the game-native Mods/settings page (#14). Runtime tests, game-side save/restart checks and the focused Windows keyboard smoke check pass. The owner approved the desktop and in-game scaling review.
- Added desktop runtime setup/removal, revision-checked executable launch, process-bound runtime results and the credited Sanctuary artwork launch control (#15). Non-empty collection preparation remains later work.
- Scoped DLL, Lua-only, map-only and mixed packages; deferred AI support pending game facilities.
- Matched the sidebar wordmark and launch label to the Oxanium brand font, removed the launch label’s separate background, and recorded nine owner-supplied display review screenshots.

## 0.1.1 — 2026-09-06

SQLite corrective milestone; internal build with no published installer or app release.

- Replaced local Turso persistence with pinned bundled SQLite through rusqlite 0.40.2, retaining the background owner and native state contract (#40).
- Added schema-4 conversion of legacy schemas 1–3 on retained copies, with validated directory promotion, restart recovery, preserved artifacts and legacy backup restoration (#39, #40).
- Used SQLite's backup API with completion markers, WAL, FULL synchronization, immediate transactions and bounded busy handling (#40).
- Added pinned legacy fixtures without a Turso build dependency, production interruption tests and native conversion/responsiveness checks (#41).
- Kept one branch per milestone and completed work on main.

## 0.1.0 — 2026-09-06

Desktop foundation; internal build with no published installer or app release.

- Added the minimal Tauri desktop shell with plain Svelte, TypeScript and Vite.
- Added locked dependencies, frontend and Rust check commands, and configuration/rendering fixture tests.
- Extended required CI with frontend checks and a Windows executable build; added grouped npm/Cargo dependency updates.
- Deferred TypeScript 7 dependency updates until the Svelte and ESLint checkers support that major version.
- Kept text file line endings consistent on Windows so clean checkouts pass formatting checks.
- Added version checks and preparation for npm, Cargo, Tauri and root lockfile entries, with tests for malformed input, drift and failed writes (#7).
- Added CI version enforcement and Windows executable artifacts named with the product version and commit SHA (#7).

- Added six desktop destinations, the full reserved launch label, keyboard navigation and text resizing (#8).
- Added revisioned native state, reconnect handling, generated TypeScript contracts and bounded diagnostics with progress, cancellation and failure states (#8).
- Added a single-instance guard, restricted native capabilities, browser interaction tests and a Windows integration check (#8).

- Added pinned local Turso persistence for library entries, ordered collection references and active selection, with revision checks and record constraints (#9).
- Added migration backups, restore into a new directory, and Windows tests for rollback, forced termination, busy handling and corrupt/newer data retention (#9).
- Added nonblocking storage startup and visible saved-data status; native tests now use isolated temporary data directories (#9).

- Added Steam discovery, native folder selection, executable/layout/build validation and one saved game installation (#10).
- Added live process observation, unknown-state handling and periodic selected-build validation without repeated Steam library scans (#10).
- Verified native picker cancellation/errors, external process start/exit, restart persistence, changing builds and read-only inspection of the installed playtest (#10).

Mod management, game launch and runtime integration remain planned. Database schema 3 preserves earlier records through backed-up migrations.

## 0.0.2 — 2026-09-06

Design amendments; no app binary released.

- Retained the logo and accepted the desktop layout as the first-version baseline.
- Replaced the in-game mockup with requirements for Sanctuary's own menu style and a monochrome Starframe Mods icon.
- Specified the full launch label with faded, orange-tinted game artwork; artwork selection and in-game rendering remain implementation checks.
- Added accessibility and motion skill guidance, updated runtime inventory requirements, and revised the future implementation issues.

## 0.0.1 — 2026-09-06

Design and development handoff; no app binary released.

- Completed the original frame-and-sun identity and interactive desktop/in-game specimen, approved by the user for handoff.
- Defined initial Windows support, responsiveness targets and runtime handoff formats.

- Recorded Starframe's design, desktop architecture, owned in-game runtime, load-order policy, collections and local development behavior.
- Selected embedded Turso and Tauri's Windows distribution tools, subject to implementation verification.
- Defined continuous checks, versioning and delivery through versioned GitHub milestones.
- Added documentation checks and checker regression tests through GitHub Actions.

The earlier `0.0.0` value identified the initial planning baseline. No app release has been published.
