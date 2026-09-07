# Starframe
Starframe is a mod manager for Sanctuary: Shattered Sun

The internal `0.2.0` build retains the SQLite desktop foundation from `0.1.1`: a Tauri window with Svelte navigation, typed live state, local SQLite storage and read-only game discovery. Settings can locate Sanctuary, save one installation and show its build and running state. Library and collection records have recoverable migrations; editing them through the interface remains later work. Help & logs includes a diagnostic workload for checking progress, cancellation and responsiveness. Mod management and an installer are not available. Internal builds with staged runtime resources can finish setup and launch the game; see [launch setup and limits](docs/verification/game-launch.md). See [verification and limits](docs/verification/game-discovery.md), [development setup](DEVELOPMENT.md#desktop-development) and [version preparation](DEVELOPMENT.md#versions-and-change-history).

Milestone **0.2.0** is complete. It adds the C# runtime contracts and a guarded development command for reversible BepInEx bootstrap installation/removal. The development runtime loads fixture DLL mods, writes process-bound activation reports, and provides a game-native Mods/settings page with per-mod persistence. Its focused Windows keyboard and in-game persistence checks pass. The owner approved desktop and in-game readability after the Windows scaling procedure; see the [display review](docs/verification/display-scaling.md). Desktop setup, launch and process-bound runtime status are connected for an empty collection. The end-user mod workflow remains later work. See [bootstrap commands and recovery](docs/verification/bootstrap.md). See [activation verification and supported formats](docs/verification/runtime-activation.md) and [settings behavior and verification](docs/verification/runtime-settings.md).

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
- [License](LICENSE): GNU Affero General Public License v3.0 for Starframe code. [Sanctuary artwork has separate ownership and use conditions](docs/notices/Sanctuary-artwork.md).
