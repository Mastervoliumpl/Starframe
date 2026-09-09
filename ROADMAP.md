# Starframe version roadmap

**[0.6.0](https://github.com/Mastervoliumpl/Starframe/milestone/7)** is active from 9 September 2026. Version `0.6.0-dev.1` starts Windows alpha distribution on `codex/0.6.0-windows-alpha`. Work begins with installer packaging (#27); catalog authentication (#46) and bounded fuzzing (#47) are also in this milestone. Updates (#28), release automation (#29) and alpha acceptance (#30) follow their prerequisites. No app release or installer is published. Milestone 0.5.0 is complete; see its [exit evidence](docs/verification/milestone-0.5.0.md).

Work through one milestone at a time. Later milestones remain planned even though their GitHub state is open. Each issue lists prerequisites, acceptance criteria and verification. Milestone completion does not start later work or replace required design review. See [DEVELOPMENT.md](DEVELOPMENT.md) for checks, version preparation, commits and GitHub CLI use.

## Delivery sequence

| Version | Outcome | Status |
| --- | --- | --- |
| [0.0.1](https://github.com/Mastervoliumpl/Starframe/milestone/1) | Design and development handoff | Complete |
| [0.0.2](https://github.com/Mastervoliumpl/Starframe/milestone/9) | Game-native menu and desktop launch design amendments | Complete |
| [0.1.0](https://github.com/Mastervoliumpl/Starframe/milestone/2) | Desktop foundation | Complete |
| [0.1.1](https://github.com/Mastervoliumpl/Starframe/milestone/10) | Replace Turso with bundled SQLite and preserve existing data | Complete |
| [0.2.0](https://github.com/Mastervoliumpl/Starframe/milestone/3) | Starframe in-game runtime | Complete |
| [0.3.0](https://github.com/Mastervoliumpl/Starframe/milestone/4) | Curated mod management | Complete |
| [0.4.0](https://github.com/Mastervoliumpl/Starframe/milestone/5) | Ordered and shared collections | Complete |
| [0.4.1](https://github.com/Mastervoliumpl/Starframe/milestone/11) | Exact approval identities, package/runtime limits and maintenance | Complete |
| [0.5.0](https://github.com/Mastervoliumpl/Starframe/milestone/6) | Local mod development | Complete: #24–#26, #57–#58 verified |
| [0.6.0](https://github.com/Mastervoliumpl/Starframe/milestone/7) | Windows alpha distribution | Active: installer packaging |
| [0.7.0](https://github.com/Mastervoliumpl/Starframe/milestone/8) | Native game integration | Blocked on official game API |

AI-package support is deferred until the game provides suitable AI extension/selection facilities. Automated replacement of the shipped AI is outside the current scope. Assign that work to a future milestone after those facilities can be verified; it is not a promised 0.7.0 feature.

[Security scope](SECURITY.md) keeps 0.3.0 focused on the internal mod lifecycle, accurate status messages, disk/write recovery checks and separate dependency auditing. Catalog signing/advisory delivery must precede public catalog access; Installer/update artifact signatures and hosted release approval remain in 0.6.0. Windows publisher certificates are deferred to [#61](https://github.com/Mastervoliumpl/Starframe/issues/61), without a milestone, at the owner's request. Broader fuzzing and detailed compatibility reporting are follow-up work. Publishing an empty development catalog is not approval to open a live catalog to general users.

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

Complete. Issues #6–#10 deliver the shell, synchronized versions, responsive live state, local Turso persistence and read-only game discovery. Restart persistence, slow-work interaction and Windows build checks pass. [Desktop verification](docs/verification/desktop-state.md), [storage verification](docs/verification/storage.md) and [game discovery/milestone exit](docs/verification/game-discovery.md) record evidence and remaining limits. Required GitHub checks must pass before the completion change merges. No loader, mod download, game launch or installer is included.

- [#6: Create the desktop projects with lint, build and test checks](https://github.com/Mastervoliumpl/Starframe/issues/6)
- [#7: Keep desktop and build versions synchronized](https://github.com/Mastervoliumpl/Starframe/issues/7)
- [#8: Build typed live state and responsive desktop navigation](https://github.com/Mastervoliumpl/Starframe/issues/8)
- [#9: Verify and implement embedded Turso persistence](https://github.com/Mastervoliumpl/Starframe/issues/9)
- [#10: Discover Sanctuary and observe its running state](https://github.com/Mastervoliumpl/Starframe/issues/10)

### 0.1.1: SQLite transition

Complete. Bundled SQLite preserves supported legacy records and backups through validated conversion, while retaining original files and artifacts. The internal product version is 0.1.1. [Recovery/native checks and Windows build measurements](docs/verification/sqlite.md) passed. Turso is absent from the application and test dependency graphs. No release or installer was published and no game files were changed.

- [#39: Prove recoverable conversion from Turso to SQLite](https://github.com/Mastervoliumpl/Starframe/issues/39)
- [#40: Replace Turso persistence with bundled SQLite](https://github.com/Mastervoliumpl/Starframe/issues/40)
- [#41: Verify SQLite recovery and Windows build costs; close 0.1.1](https://github.com/Mastervoliumpl/Starframe/issues/41)

### 0.2.0: Starframe in-game runtime

Complete through PR #43. Runtime contracts and [bootstrap deployment/recovery](docs/verification/bootstrap.md) are implemented; managed fixture activation and reports are implemented. The game-native settings page and persistence are complete, including the [focused Windows keyboard smoke check](docs/verification/runtime-settings.md). Desktop setup, executable launch and runtime observation are implemented for an empty collection; [verification and build limits](docs/verification/game-launch.md) record the game smoke check and shutdown limitation. Content/plugin compatibility limits are recorded in [activation verification](docs/verification/runtime-activation.md). Issue #11 explicitly depends on the SQLite exit issue [#41](https://github.com/Mastervoliumpl/Starframe/issues/41). Deliver reversible bootstrap deployment, a Starframe-owned C# runtime and settings UI, and a verified launch path. Exit: a fixture mod loads through BepInEx, settings persist, and cleanup/recovery are demonstrated. The owner approved the desktop and game settings views after the Windows 100%/150%/200% scaling procedure; [display review](docs/verification/display-scaling.md) records the supplied evidence and limits.

The package investigation in #13 explicitly includes managed DLLs, conventional BepInEx plugins, Lua-only content, map-only content and mixed packages. [Map source evidence](docs/planning/map-support.md) distinguishes content-only maps from DLL dependencies and records current-build verification gaps. Review the DLL-required activation format before implementing map support; no wrapper DLL should be required solely for package metadata. AI support remains deferred.

- [#11: Add the C# runtime project and shared activation contracts](https://github.com/Mastervoliumpl/Starframe/issues/11)
- [#12: Deploy and remove the Starframe bootstrap with recovery](https://github.com/Mastervoliumpl/Starframe/issues/12)
- [#13: Activate manifest-listed mods and report runtime results](https://github.com/Mastervoliumpl/Starframe/issues/13)
- [#14: Implement Starframe's in-game mod settings UI](https://github.com/Mastervoliumpl/Starframe/issues/14)
- [#15: Launch the prepared game and verify runtime readiness](https://github.com/Mastervoliumpl/Starframe/issues/15)

### 0.3.0: Curated mod management

Complete. Version 0.3.0 delivers catalog refresh, verified downloads, installed-mod lifecycle and live management screens. [Exit evidence](docs/verification/milestone-0.3.0.md) records checks and limits. The initial catalog is empty; public catalog access and installer delivery remain separate gates.

- [#16: Publish and refresh the curated release catalog independently](https://github.com/Mastervoliumpl/Starframe/issues/16)
- [#17: Download and prepare approved packages safely](https://github.com/Mastervoliumpl/Starframe/issues/17)
- [#18: Manage installed mods through automatic safe deployment](https://github.com/Mastervoliumpl/Starframe/issues/18)
- [#19: Connect My mods, Catalog and Downloads to live operations](https://github.com/Mastervoliumpl/Starframe/issues/19)

### 0.4.0: Ordered and shared collections

Completed on 8 September 2026, covering #20 through #23. [Milestone exit evidence](docs/verification/milestone-0.4.0.md) records the two-library and real-game checks, including exact missing-package preparation, runtime order, game-exit application and settings retention. [Sharing verification](docs/verification/sharing.md) records import recovery. The four known identity/runtime limits remain assigned to 0.4.1.

- [#20: Resolve dependency constraints and manual load priority](https://github.com/Mastervoliumpl/Starframe/issues/20)
- [#21: Build named ordered collections and automatic switching](https://github.com/Mastervoliumpl/Starframe/issues/21)
- [#22: Share and recreate collections using exact mod references](https://github.com/Mastervoliumpl/Starframe/issues/22)
- [#23: Verify ordered collection behavior across desktop and game](https://github.com/Mastervoliumpl/Starframe/issues/23)

### 0.4.1: Approval identities and runtime limits

Completed on 8 September 2026. The four correctness fixes preserve exact approval identities, separate library size from runtime inventory, validate package bounds before readiness and assign distinct deployment roots to logical mods sharing archives. Issue #54 extracts the reviewed Rust responsibilities and generates the desktop management contracts. [Exit evidence](docs/verification/milestone-0.4.1.md) records migration, shared boundary tests and real-game acceptance.

- [#50: Preserve distinct approval references when archive bytes are reapproved](https://github.com/Mastervoliumpl/Starframe/issues/50)
- [#51: Keep large disabled libraries from blocking valid game launches](https://github.com/Mastervoliumpl/Starframe/issues/51)
- [#52: Validate package file counts against supported runtime limits](https://github.com/Mastervoliumpl/Starframe/issues/52)
- [#53: Resolve overlapping deployment roots for mods sharing an archive](https://github.com/Mastervoliumpl/Starframe/issues/53)
- [#54: Refactor Rust modules and enforce desktop/runtime contract consistency](https://github.com/Mastervoliumpl/Starframe/issues/54)

These fixes do not establish general compatibility with conventional BepInEx plugins, Lua packages or maps. Adapters and tests with representative author mods remain part of the route to a usable public alpha.

### 0.5.0: Local mod development

The owner added [#57: BepInEx plugin compatibility and Ladder Reporter verification](https://github.com/Mastervoliumpl/Starframe/issues/57) to this milestone and PR #56. The [compatibility and session-list checks](docs/verification/bepinex-plugins.md) record completed game, settings and guarded cleanup acceptance for #57 and #58. [Ladder Reporter 0.3.0](docs/verification/ladder-reporter-catalog.md) is the first approved catalog mod. This remains an internal milestone, with public alpha distribution gates in 0.6.0.

Complete through PR #56. Issue #24 adds local DLL/folder imports with normal management controls; [local import guidance](docs/verification/local-imports.md) defines the supported inputs and checks. Issue #25 adds [stable watched copies](docs/verification/local-watching.md), and #26 verifies the [developer/game workflow](docs/verification/milestone-0.5.0.md). Exit: local rebuilds apply after game exit, incomplete builds retain usable content, and source files survive uninstall.

- [#24: Import local mods with normal management controls](https://github.com/Mastervoliumpl/Starframe/issues/24)
- [#25: Follow local builds and apply stable copies after game exit](https://github.com/Mastervoliumpl/Starframe/issues/25)
- [#26: Verify the local developer workflow and document its limits](https://github.com/Mastervoliumpl/Starframe/issues/26)
- [#57: Support existing BepInEx plugins and verify Ladder Reporter](https://github.com/Mastervoliumpl/Starframe/issues/57)
- [#58: Show current-session mods clearly in the game menu](https://github.com/Mastervoliumpl/Starframe/issues/58)

### 0.6.0: Windows alpha distribution

Pre-publication follow-ups: [#46: Catalog authentication and security advisory delivery](https://github.com/Mastervoliumpl/Starframe/issues/46) and [#47: Broader archive/path fuzzing](https://github.com/Mastervoliumpl/Starframe/issues/47). Move the relevant gate forward if public distribution starts earlier; neither blocks internal 0.3.0 implementation.

The #47 bounded targets and local campaign are implemented, with [results and remaining coverage limits](docs/verification/fuzzing.md). Installer #27 remains open for clean-Windows, actual cross-build upgrade and interactive/process-interruption acceptance. The owner has no disposable Windows VM and authorized continuing other checks while clean-machine acceptance remains pending.

Active. Deliver signed user-initiated updates, NSIS install/uninstall, release checks and user documentation. Exit: whole-app Windows verification passes and a draft installer release is reviewable. Publishing remains a maintainer action. Windows Authenticode is deferred to #61. Private/public-key signatures for installer/update artifacts and catalog metadata remain required. The owner approved daily catalog renewal with thirty-day validity; expired metadata pauses new catalog downloads. Lawful hosted acquisition of game build references remains unresolved. Runtime installation, upgrade and uninstall are automatic; full app uninstall deletes managed app data by default with an explicit keep-data option (DESIGN.md revision 0.10).

- [#27: Package Windows installation and owned-file cleanup](https://github.com/Mastervoliumpl/Starframe/issues/27)
- [#28: Check GitHub releases and install signed updates on request](https://github.com/Mastervoliumpl/Starframe/issues/28)
- [#29: Build and verify draft releases through GitHub Actions](https://github.com/Mastervoliumpl/Starframe/issues/29)
- [#30: Complete Windows alpha acceptance and user documentation](https://github.com/Mastervoliumpl/Starframe/issues/30)

### 0.7.0: Native game integration

Blocked on the game's official mod API and completion of 0.6.0. Replace the current integration with native loading where supported, retain collections and library identities, and provide recoverable migration. No speculative adapter implementation or release date.

- [#31: Evaluate the official mod API when the game exposes it](https://github.com/Mastervoliumpl/Starframe/issues/31)
- [#32: Integrate native mod loading with recoverable migration](https://github.com/Mastervoliumpl/Starframe/issues/32)

## Unscheduled backlog

- [#61: Add trusted Windows publisher signing when eligible](https://github.com/Mastervoliumpl/Starframe/issues/61). No milestone; does not block the initial alpha.

## Working boundaries

[Mod submissions and hosting](docs/planning/mod-hosting.md) records the owner's future interest in an upload/submission website, richer mod pages, download counts and ratings. No implementation milestone is assigned; 0.5.0 remains focused on the verified local development workflow.

Tests accompany each feature. Installer and game acceptance checks at the end of a milestone add evidence across modules; they do not postpone unit or integration tests.

Independent issues in the active milestone can be delegated after their shared contracts are agreed. A dependency in an issue body is a work prerequisite, not a suggestion. Check its current status with `gh` before starting.

CI checks repository documents, versions, the frontend and the Windows build. Release automation arrives with distribution. Milestone 0.1.0 does not install loaders, change game files or publish an app release.
