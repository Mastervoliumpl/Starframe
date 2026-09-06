# Starframe version roadmap

Completed handoff: **[0.0.1](https://github.com/Mastervoliumpl/Starframe/milestone/1)** with **[0.0.2 design amendments](https://github.com/Mastervoliumpl/Starframe/milestone/9)**. Current development version: **0.1.0-dev.1**. Milestone **0.1.0** is active; the user has authorized **only issue #6**. Complete that issue and stop before starting any other issue. No app release has been published.

Work through one milestone at a time. Later milestones remain planned even though their GitHub state is open. Each issue lists prerequisites, acceptance criteria and verification. Milestone completion does not override the planning-only phase or required design review. See [DEVELOPMENT.md](DEVELOPMENT.md) for checks, version preparation, commits and GitHub CLI use.

## Delivery sequence

| Version | Outcome | Status |
| --- | --- | --- |
| [0.0.1](https://github.com/Mastervoliumpl/Starframe/milestone/1) | Design and development handoff | Complete |
| [0.0.2](https://github.com/Mastervoliumpl/Starframe/milestone/9) | Game-native menu and desktop launch design amendments | Complete |
| [0.1.0](https://github.com/Mastervoliumpl/Starframe/milestone/2) | Desktop foundation | Active; only #6 authorized |
| [0.2.0](https://github.com/Mastervoliumpl/Starframe/milestone/3) | Starframe in-game runtime | Planned |
| [0.3.0](https://github.com/Mastervoliumpl/Starframe/milestone/4) | Curated mod management | Planned |
| [0.4.0](https://github.com/Mastervoliumpl/Starframe/milestone/5) | Ordered and shared collections | Planned |
| [0.5.0](https://github.com/Mastervoliumpl/Starframe/milestone/6) | Local mod development | Planned |
| [0.6.0](https://github.com/Mastervoliumpl/Starframe/milestone/7) | Windows alpha distribution | Planned |
| [0.7.0](https://github.com/Mastervoliumpl/Starframe/milestone/8) | Native game integration | Blocked on official game API |

Versions describe bounded outcomes, not dates. The native integration target may move when the official API becomes available; it must not block corrective releases to existing features. Add a patch milestone such as `0.6.1` when a released version needs fixes. Finish or explicitly pause the active milestone before changing focus.

## Issue index

The issue pages are the source of task status. This index records scope and order; do not duplicate completion checkboxes here.

### 0.0.1: Design and development handoff

Complete. The user approved the identity and visual specimen on 6 September 2026. The technical handoff, issue plan, versioning and documentation CI are recorded. See [handoff](docs/HANDOFF.md) and [review evidence](docs/design/REVIEW.md). No application implementation was part of this milestone.

- [#1: Record the accepted Starframe product and architecture baseline](https://github.com/Mastervoliumpl/Starframe/issues/1)
- [#2: Establish continuous checks, versioning and milestone rules](https://github.com/Mastervoliumpl/Starframe/issues/2)
- [#3: Design the Starframe wordmark and application icon](https://github.com/Mastervoliumpl/Starframe/issues/3)
- [#4: Review desktop screens, load-order controls and in-game settings design](https://github.com/Mastervoliumpl/Starframe/issues/4)
- [#5: Finalize implementation contracts and the 0.0.1 handoff](https://github.com/Mastervoliumpl/Starframe/issues/5)

### 0.0.2: Design amendments

Complete. Retained the logo and first-version desktop layout, replaced the in-game direction with a Sanctuary-native Mods menu, specified the artwork-backed launch action, and added scoped UI skill guidance. This does not start app implementation.

- [#34: Amend the in-game menu and desktop launch design](https://github.com/Mastervoliumpl/Starframe/issues/34)

### 0.1.0: Desktop foundation

Active. Issue #6 introduces the minimal shell and language CI. Issues #7–#10 remain planned and require a further user instruction. The full milestone will deliver typed Rust state, verified local Turso storage and read-only game discovery. Exit: restart persistence, slow-work interaction and Windows build checks pass.

- [#6: Create the desktop projects with lint, build and test checks](https://github.com/Mastervoliumpl/Starframe/issues/6)
- [#7: Keep desktop and build versions synchronized](https://github.com/Mastervoliumpl/Starframe/issues/7)
- [#8: Build typed live state and responsive desktop navigation](https://github.com/Mastervoliumpl/Starframe/issues/8)
- [#9: Verify and implement embedded Turso persistence](https://github.com/Mastervoliumpl/Starframe/issues/9)
- [#10: Discover Sanctuary and observe its running state](https://github.com/Mastervoliumpl/Starframe/issues/10)

### 0.2.0: Starframe in-game runtime

Planned; starts after 0.1.0 closes. Deliver reversible bootstrap deployment, a Starframe-owned C# runtime and settings UI, and a verified launch path. Exit: a fixture mod loads through BepInEx, settings persist, and cleanup/recovery are demonstrated.

- [#11: Add the C# runtime project and shared activation contracts](https://github.com/Mastervoliumpl/Starframe/issues/11)
- [#12: Deploy and remove the Starframe bootstrap with recovery](https://github.com/Mastervoliumpl/Starframe/issues/12)
- [#13: Activate manifest-listed mods and report runtime results](https://github.com/Mastervoliumpl/Starframe/issues/13)
- [#14: Implement Starframe's in-game mod settings UI](https://github.com/Mastervoliumpl/Starframe/issues/14)
- [#15: Launch the prepared game and verify runtime readiness](https://github.com/Mastervoliumpl/Starframe/issues/15)

### 0.3.0: Curated mod management

Planned; starts after 0.2.0 closes. Deliver independently refreshed catalog data, verified package preparation, install/enable/disable/uninstall, version warnings and Downloads. Exit: an approved release completes the full lifecycle with interruption and file-ownership checks.

- [#16: Publish and refresh the curated release catalog independently](https://github.com/Mastervoliumpl/Starframe/issues/16)
- [#17: Download and prepare approved packages safely](https://github.com/Mastervoliumpl/Starframe/issues/17)
- [#18: Manage installed mods through automatic safe deployment](https://github.com/Mastervoliumpl/Starframe/issues/18)
- [#19: Connect My mods, Catalog and Downloads to live operations](https://github.com/Mastervoliumpl/Starframe/issues/19)

### 0.4.0: Ordered and shared collections

Planned; starts after 0.3.0 closes. Deliver deterministic dependency ordering, accessible manual priority, automatic collection application and exact-content sharing. Exit: reproduce an ordered collection on a second library, reusing matching artifacts and explaining unresolved entries.

- [#20: Resolve dependency constraints and manual load priority](https://github.com/Mastervoliumpl/Starframe/issues/20)
- [#21: Build named ordered collections and automatic switching](https://github.com/Mastervoliumpl/Starframe/issues/21)
- [#22: Share and recreate collections using exact mod references](https://github.com/Mastervoliumpl/Starframe/issues/22)
- [#23: Verify ordered collection behavior across desktop and game](https://github.com/Mastervoliumpl/Starframe/issues/23)

### 0.5.0: Local mod development

Planned; starts after 0.4.0 closes. Deliver local DLL/folder imports, normal management controls and stable watched build copies. Exit: local rebuilds apply after game exit, incomplete builds retain usable content, and source files survive uninstall.

- [#24: Import local mods with normal management controls](https://github.com/Mastervoliumpl/Starframe/issues/24)
- [#25: Follow local builds and apply stable copies after game exit](https://github.com/Mastervoliumpl/Starframe/issues/25)
- [#26: Verify the local developer workflow and document its limits](https://github.com/Mastervoliumpl/Starframe/issues/26)

### 0.6.0: Windows alpha distribution

Planned; starts after 0.5.0 closes. Deliver signed user-initiated updates, NSIS install/uninstall, release checks and user documentation. Exit: whole-app Windows verification passes and a draft installer release is reviewable. Publishing remains a maintainer action.

- [#27: Package Windows installation and owned-file cleanup](https://github.com/Mastervoliumpl/Starframe/issues/27)
- [#28: Check GitHub releases and install signed updates on request](https://github.com/Mastervoliumpl/Starframe/issues/28)
- [#29: Build and verify draft releases through GitHub Actions](https://github.com/Mastervoliumpl/Starframe/issues/29)
- [#30: Complete Windows alpha acceptance and user documentation](https://github.com/Mastervoliumpl/Starframe/issues/30)

### 0.7.0: Native game integration

Blocked on the game's official mod API and completion of 0.6.0. Replace the current integration with native loading where supported, retain collections and library identities, and provide recoverable migration. No speculative adapter implementation or release date.

- [#31: Evaluate the official mod API when the game exposes it](https://github.com/Mastervoliumpl/Starframe/issues/31)
- [#32: Integrate native mod loading with recoverable migration](https://github.com/Mastervoliumpl/Starframe/issues/32)

## Working boundaries

Tests accompany each feature. Installer and game acceptance checks at the end of a milestone add evidence across modules; they do not postpone unit or integration tests.

Independent issues in the active milestone can be delegated after their shared contracts are agreed. A dependency in an issue body is a work prerequisite, not a suggestion. Check its current status with `gh` before starting.

The current CI checks repository documents and version format. Language-specific checks arrive with their projects, and release automation arrives with distribution. No app code, loader installation, game-file changes, or public app release is part of the current setup.
