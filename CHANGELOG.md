# Changelog

## Unreleased

- Added mod enable/disable/uninstall commands, exact dependency preparation, automatic stopped-game deployment and schema-8 uninstall cleanup (#18). The management screens remain #19.
- Added streamed mod deployment/recovery content, disk-space estimates and lifecycle/lock/interruption fixtures. Ordinary withdrawal preserves verified installed copies and settings.
- Added separate scheduled/dependency-change audits and a phased security plan. Catalog/advisory signing and Windows release signing remain pre-distribution work.

- Added bounded downloads, exact hash verification, guarded ZIP extraction and immutable package preparation for approved releases (#17).
- Added schema-7 package history, atomic library completion, cancellation, verified-content reuse and interruption recovery. Package operations have a focused native command; their screens remain issue #19.
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
