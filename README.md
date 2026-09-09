# Starframe
Starframe is a mod manager for Sanctuary: Shattered Sun

Milestone **0.6.0** is in development. It adds Windows packaging, signed app updates, catalog authentication and public-alpha verification. These features are not yet ready for distribution; the completed internal build below remains the verified baseline.

The development installer now supports automatic runtime setup and cleanup. Full uninstall deletes managed app data unless **Keep my library, collections and settings** is selected; upgrades preserve it. Original local mod sources and unowned game settings remain untouched. Windows publisher signing is deferred to [#61](https://github.com/Mastervoliumpl/Starframe/issues/61); installer/update artifact signatures and catalog authentication remain release requirements. See [installer verification and limits](docs/verification/windows-installer.md).

The internal **0.5.0** build adds local DLL and folder imports to curated mod downloads, named collections, dependency ordering, drag priority and exact-reference sharing. Local imports keep managed copies, follow settled source rebuilds while Starframe is open, and retain the author's source files. See [local import instructions and limits](docs/verification/local-imports.md). Saved collection changes deploy automatically while the game is closed, and collection switches retain mod settings. [BepInEx 5 plugin support](docs/bepinex-mods.md) runs compatible author DLLs without an `IMod` rewrite. Catalog revision 3 adds [six more Remmy mods](docs/verification/remmy-catalog.md) alongside [Ladder Reporter 0.3.0](docs/verification/ladder-reporter-catalog.md), with verification limits recorded in their reviews. No installer or app release has been published.

The desktop uses Tauri, Svelte, Rust and local SQLite storage. Settings locates Sanctuary, saves one installation and observes its build and running state. Library records and deployment operations have recovery checks. Help & logs includes a diagnostic workload for progress, cancellation and responsiveness. Internal builds with staged runtime resources can finish setup and launch the game; see [launch setup](docs/verification/game-launch.md), [development setup](DEVELOPMENT.md#desktop-development) and [version preparation](DEVELOPMENT.md#versions-and-change-history). Milestone [0.4.1 corrections](docs/verification/milestone-0.4.1.md) are complete. See [local build watching](docs/verification/local-watching.md) for rebuild behavior. The [developer/game workflow](docs/verification/milestone-0.5.0.md) and [BepInEx session/settings checks](docs/verification/bepinex-plugins.md) passed. Follow the [local development guide](docs/local-development.md) to import, rebuild and share a mod. Installer delivery remains planned.

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
