# Starframe
Starframe is a mod manager for Sanctuary: Shattered Sun

The completed internal **0.3.0** build supports the curated-mod workflow: [independent catalog refresh](catalog/README.md), verified package downloads, My mods/Catalog/Downloads screens, enable/disable and confirmed uninstall. Saved changes deploy automatically while the game is closed. Compatibility warnings allow continued use; settings survive uninstall. See [milestone evidence and limits](docs/verification/milestone-0.3.0.md). Development of **0.4.0** starts with dependency ordering, manual priority and verified Lua overlays in [issue #20](docs/verification/ordering.md). Named collection editing and switching are implemented in [issue #21](docs/verification/collections.md); sharing remains planned. The initial catalog is empty, and no installer or app release has been published.

The desktop uses Tauri, Svelte, Rust and local SQLite storage. Settings locates Sanctuary, saves one installation and observes its build and running state. Library records and deployment operations have recovery checks. Help & logs includes a diagnostic workload for progress, cancellation and responsiveness. Internal builds with staged runtime resources can finish setup and launch the game; see [launch setup](docs/verification/game-launch.md), [development setup](DEVELOPMENT.md#desktop-development) and [version preparation](DEVELOPMENT.md#versions-and-change-history). Collection sharing, local imports and installer delivery remain planned.

Milestone **0.2.0** delivered the C# runtime, reversible BepInEx bootstrap installation/removal and the game-native Mods/settings page with per-mod persistence. Managed fixtures load and write process-bound activation reports. The owner approved desktop and in-game readability; see the [display review](docs/verification/display-scaling.md), [bootstrap recovery](docs/verification/bootstrap.md), [supported activation formats](docs/verification/runtime-activation.md) and [settings verification](docs/verification/runtime-settings.md).

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
