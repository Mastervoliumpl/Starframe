# Starframe
Starframe is a mod manager for Sanctuary: Shattered Sun

Planning and design are in progress. The desktop stack is Tauri, Svelte, TypeScript, and Rust, with Windows first. A Starframe-owned C# runtime will provide in-game loading and settings, initially using BepInEx for bootstrap.

- [Design direction](DESIGN.md): visual style and interaction requirements.
- [Architecture draft](ARCHITECTURE.md): proposed project structure, diagrams, and module behavior.
- [Visual overview](docs/architecture-overview.svg): desktop, storage, downloads, and game integration.
- [Interactive design review](docs/design/review.html): original identity and simulated desktop/in-game screens; open the local HTML file in a browser.
- [Implementation handoff](docs/HANDOFF.md): initial support targets, contracts and verification limits.
- [Terminology](CONTEXT.md): shared definitions for the project.
- [Development and checks](DEVELOPMENT.md): continuous tests, CI, versioning and contribution rules.
- [Version roadmap](ROADMAP.md): the active milestone and scoped GitHub issues.
- [Changelog](CHANGELOG.md): completed changes; [VERSION](VERSION) starts at the `0.0.0` planning baseline.
- [License](LICENSE): GNU Affero General Public License v3.0.
