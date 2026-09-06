# Starframe
Starframe is a mod manager for Sanctuary: Shattered Sun

The desktop foundation now uses SQLite in `0.1.1`: a Tauri window with Svelte navigation, typed live state, local SQLite storage and read-only game discovery. Settings can locate Sanctuary, save one installation and show its build and running state. Library and collection records have recoverable migrations; editing them through the interface remains later work. Help & logs includes a diagnostic workload for checking progress, cancellation and responsiveness. Mod management, game launch and an installer are not available in this internal build. See [verification and limits](docs/verification/game-discovery.md), [development setup](DEVELOPMENT.md#desktop-development) and [version preparation](DEVELOPMENT.md#versions-and-change-history).

Milestone **0.1.1** completes the SQLite migration. The application converts supported legacy data on retained copies and uses bundled SQLite. Recovery, native checks and Windows build measurements are recorded. See [SQLite recovery and verification](docs/verification/sqlite.md). See the [decision and data-preservation plan](docs/planning/sqlite-transition.md).

- [Design direction](DESIGN.md): visual style and interaction requirements.
- [Architecture draft](ARCHITECTURE.md): proposed project structure, diagrams, and module behavior.
- [Visual overview](docs/architecture-overview.svg): desktop, storage, downloads, and game integration.
- [Interactive design review](docs/design/review.html): original identity and simulated desktop/in-game screens; open the local HTML file in a browser.
- [Implementation handoff](docs/HANDOFF.md): initial support targets, contracts and verification limits.
- [Terminology](CONTEXT.md): shared definitions for the project.
- [Development and checks](DEVELOPMENT.md): continuous tests, CI, versioning and contribution rules.
- [Version roadmap](ROADMAP.md): the active milestone and scoped GitHub issues.
- [Changelog](CHANGELOG.md): completed changes; [VERSION](VERSION) identifies the current development version.
- [Storage dependency notices](docs/THIRD_PARTY_NOTICES.md).
- [License](LICENSE): GNU Affero General Public License v3.0.
