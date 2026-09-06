# Changelog

## Unreleased

Development version `0.1.0-dev.1`; milestone 0.1.0 is in progress.

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

Mod management, storage and game discovery remain planned. No app release or installer is published.

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
