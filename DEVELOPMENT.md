# Starframe development and checks

Status: development policy adopted on 6 September 2026. Milestone 0.1.0 is complete; 0.1.1 is active.

Read [DESIGN.md](DESIGN.md) for behavior and presentation, [ARCHITECTURE.md](ARCHITECTURE.md) for structure and recovery rules, and [ROADMAP.md](ROADMAP.md) for the current milestone. GitHub issues hold task scope, dependencies, acceptance criteria and verification evidence.

## Work one milestone at a time

The design handoff and desktop foundation **0.1.0** are complete. The user authorized milestone 0.1.1 on 6 September 2026. Work through #39, #40 and #41 before 0.2.0. Every project issue belongs to one version milestone. Give a new issue a milestone before starting it. Bugs found during a milestone belong there if they prevent its intended outcome; otherwise assign a later version explicitly.

Work on an issue only when its milestone is active and its prerequisites are complete. Keep issue dependencies in a `Depends on` section with issue links. Each issue must state its scope, observable completion criteria and checks. Split an issue when it contains independently reviewable outcomes; avoid splitting one small change into tasks that cannot be tested separately.

Subagents may work on independent issues within the active milestone when delegated. Agree on shared contracts before parallel implementation, give each worker distinct files where practical, and integrate with the same checks as other changes. A blocked issue does not permit starting a later milestone. This policy does not start agents or tasks by itself.

Close an issue only after its acceptance criteria and relevant checks pass. Closing a milestone also requires its exit checks, recorded unresolved limitations, and the user's review where the design calls for it. Update the active milestone in ROADMAP.md and these instructions in the same change that advances the plan. Do not carry unfinished work forward silently. Future native integration remains blocked until the game developer publishes a usable interface.

## GitHub and local work

Use the installed GitHub CLI (`gh`) for repository inspection, issues, milestones, pull requests, workflow runs and releases. Use `git` for local branches, commits, fetching and pushing. Inspect existing objects before creating new ones. Use `--body-file` or an API JSON input file for multiline text; do not construct shell commands from issue text.

Keep `main` current with completed, verified work. Use one branch per active milestone or corrective version, named `codex/<version>-<purpose>`, and one draft pull request while that milestone is in development. Make small, coherent commits for its issues on that branch. Merge only after all milestone acceptance criteria and exit checks pass. An unfinished milestone stays on its branch. Completed independent documentation or design work can use a separate pull request.

Reference issues in commits or the pull request. Preserve unrelated changes and stage explicit paths. After merging, delete the local and remote milestone branches. Remove superseded branches only after verifying that their changes are retained. Machine-local AGENTS.md instructions stay out of commits and out of .gitignore.

Pull requests identify the issue, target milestone, behavior changed, verification performed, and any data-migration or release impact. Tests and fixes belong in the same change. Use the CLI to inspect CI failures; do not merge a known failing change. Require the stable `Required checks` status on `main`, without imposing a second-reviewer requirement on a sole maintainer. The maintainer retains administrative recovery access; routine work must pass checks.

## Test as features are built

Add useful tests with implementation, not at the end of the project. Each bug fix gets a regression test where the failure is reproducible. A milestone's final verification checks how its pieces work together; it does not replace tests for those pieces.

Prefer observable behavior over tests of private function shapes. Use unit tests for deterministic rules such as ordering, version comparison and validation. Use temporary-file integration tests for storage, deployment, archive handling and recovery. Use frontend interaction tests for stale replies, pending states, navigation and accessible controls. Avoid snapshots of entire screens, blanket coverage percentages, and tests that only repeat a constant or a CSS declaration.

Run `python scripts/check_repository.py` and `python -m unittest discover -s scripts -p 'test_*.py'` for repository checks and their regression tests. The desktop commands below check Svelte/TypeScript and Rust. C# tooling starts with its first source project. Do not report absent language tests as passing.

| Area | Checks when that area is introduced |
| --- | --- |
| Svelte and TypeScript | Prettier, ESLint with Svelte/TypeScript support, `svelte-check`, production build, and Vitest for meaningful interaction/state behavior. Use compiler accessibility diagnostics. |
| Rust | `cargo fmt --check`, Clippy for the actual supported targets/features, `cargo test --locked`, and a Windows build. Treat enabled Clippy warnings as errors; do not enable every pedantic lint by default. |
| C# runtime | `dotnet format --verify-no-changes`, compiler/.NET analyzers, build against the verified Unity/Mono target, and `dotnet test` for lifecycle, configuration and contract behavior. Prefer SDK analyzers before adding another analyzer package. |
| Cross-language formats | Read shared fixtures in Rust and C#; check generated TypeScript types for drift. Reject unsupported schemas and exercise migrations. |
| Catalog and collections | Validate schema, stable IDs, approved artifact hashes, dependency references, cycles and format versions. Exercise missing/withdrawn releases and local-content matches. |
| Files and database | Test paths, archive limits, ownership, interrupted transactions, interrupted deployment, rollback, migration and backup restoration using isolated fixtures. Never point automated tests at the user's game installation. |
| UI and Windows integration | A small Playwright suite for critical browser interactions, plus real Windows/Tauri smoke checks. Manually verify game loading, keyboard operation, scaling, high contrast and reduced motion. Browser tests cannot verify native behavior. |
| Releases | Check version agreement, build NSIS installer, verify updater signatures and metadata, test clean install/update/uninstall and game cleanup, and review source/license contents. |

The C# code must compile without checking proprietary game assemblies into the repository. Keep testable runtime logic independent of those references. Resolve a lawful, repeatable reference acquisition path before enabling a required game-runtime build. Do not disguise a skipped runtime build as a successful one.

## GitHub Actions

Use pull-request and `main` push workflows for required checks. The repository job validates tracked documentation links, fenced blocks, SVG XML, product version agreement and exclusion of machine-local instructions. The frontend job runs a clean npm install, formatting, lint, Svelte/TypeScript checks, fixture tests and a production build on Linux. After repository checks pass, the Windows job runs Rustfmt, Clippy, Rust tests and a Tauri executable build. It retains the executable for seven days as `starframe-<VERSION>-windows-x64-<commit SHA>`, using the full SHA of the checked-out revision (the merge revision for pull requests). All three run on documentation changes too and feed `Required checks`; no path filters can leave it pending. None accesses game files.

Extend CI as source projects arrive. Keep one stable final status, `Required checks`, that fails if any applicable job fails or is canceled. If language jobs use path filtering, a small final job must still report a result for documentation-only changes; skipped workflows must not leave a required check pending forever. Shared contracts, lockfiles and workflow changes trigger all affected jobs.

Run fast independent checks concurrently. Cancel superseded PR runs, use finite timeouts and cache dependencies using lockfile keys. Start with one supported Windows target and a Linux runner for portable checks; expand the matrix only for a supported platform or a demonstrated failure. Upload useful failure logs and test results, omitting secrets and personal paths. Avoid requiring live external downloads for deterministic tests; use fixtures and separate scheduled/explicit source-availability checks.

Use read-only permissions by default, immutable action commit pins, and isolated release credentials. Never run untrusted pull-request code with release secrets or a privileged `pull_request_target` job. Dependency update PRs should be grouped weekly, with a small open-PR limit. Add npm, Cargo and NuGet updates when their manifests exist. Triage advisory findings by affected version, reachability and impact; record a reason and expiry for exceptions. Do not ignore applicable security findings just to make CI green. [GitHub workflow security](https://docs.github.com/en/actions/reference/security/secure-use), [Dependabot configuration](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference)

Continuous delivery first creates reviewable artifacts and draft releases. Publishing a release remains a maintainer action. Build and sign only from trusted repository revisions after required checks pass. Add the release workflow with the installer/updater milestone; no placeholder workflow should claim to publish a working app now.

## Versions and change history

[VERSION](VERSION) is the source of the product version. The current value is `0.1.1-dev.1`, the SQLite-transition development build, not a shipped installer. The initial planning baseline was `0.0.0`. [CHANGELOG.md](CHANGELOG.md) records completed changes under `Unreleased` until a version is finalized. [The version command](scripts/versions.py) checks npm, Cargo and Tauri metadata, including the root package entries in both lockfiles. CI rejects missing fields, malformed files and version drift.

Use three-part versions: `0.MINOR.PATCH` during initial development. A capability milestone advances the minor version; a corrective release advances the patch version. The planning handoff uses `0.0.1`. Published content is immutable; never replace a release with different bytes under the same version. The `0.x` series makes no stable public API promise, but format migrations and compatibility changes still need explicit notes. [Semantic Versioning](https://semver.org/)

At the start of an implementation milestone, use its target version with a development suffix, such as `0.2.0-dev.1`. Increment the suffix for a newly distributed preview build, not every local commit; use the commit SHA to identify ordinary CI artifacts. Release candidates can use `0.2.0-rc.1`. Strip the suffix only when that version's release checks pass. Planned milestone titles are targets, not evidence of releases. Internal milestones need not publish installers.

To prepare a version, edit VERSION, then run these commands with Python 3.11 or later:

```sh
python scripts/versions.py --write
python scripts/versions.py
python -m unittest discover -s scripts -p 'test_*.py'
```

The command checks by default. `--write` copies VERSION into the product fields after parsing every required file. It preserves dependency versions and lockfile format versions. JSON files that need changes use two-space indentation; Cargo files retain their comments and formatting. Each file is replaced atomically. If preparation stops between files, repeat the command to finish synchronization. Inspect the diff and update the changelog and any documented version values before committing. The command does not create commits, tags, installers or releases.

When the C# project arrives, add its `InformationalVersion` to `version_updates` using the standard-library XML parser, with fixtures in [the version tests](scripts/test_versions.py). Reuse `read_version` and the existing preflight/write path. Keep installer-specific numeric representations consistent with their platform requirements when packaging arrives. Database, catalog, collection and runtime-contract versions remain separate from the app version. Catalog edits advance catalog revision without an app-version bump.

For a release, finalize its changelog entry, validate version agreement, and create an immutable `vX.Y.Z` tag from the checked commit. Use `gh` to create or inspect the draft release and workflow results. Publish after its acceptance checks and maintainer authorization. Do not fabricate changelog entries for work that is only planned.

## Desktop development

Use Node.js **24.19.0** (see [.node-version](.node-version)) with npm, and Rust **1.98.1** with Rustfmt and Clippy (see [rust-toolchain.toml](rust-toolchain.toml)). On Windows, install the MSVC C++ build tools with the Windows SDK and the WebView2 runtime required by [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/). Target Windows x64. A Node-only machine can check and preview the frontend; Rust compilation needs the native build prerequisites too.

From the repository root:

```sh
npm ci
npm run check
npm run check:rust
npm run tauri -- build --no-bundle -- --locked
```

`npm run check` runs Prettier, ESLint, Svelte/TypeScript diagnostics, Vitest and Vite's production build. It builds `dist` before Rust checks, which need those frontend assets. `npm run check:rust` runs Rustfmt, Clippy with warnings denied, and locked Cargo tests. The final command builds the Windows executable without an installer; packaging and updates are later issues. It writes `src-tauri/target/release/starframe.exe`.

Use `npm run tauri dev` for the native development window, or `npm run dev` for the browser frontend at `http://127.0.0.1:1420`. Use `npm run format` to format frontend/configuration files and `cargo fmt --manifest-path src-tauri/Cargo.toml` for Rust. Vitest watch mode is `npm run test:watch`. Stop the development command when finished.

The shell has six destinations and five focused native commands for live state, diagnostics, fixed external pages and game discovery. The 0.1.1 build opens bundled SQLite at sqlite/state.db under Tauri's per-user local app-data directory. It converts supported Turso data on retained copies before promotion. See [SQLite verification and recovery](docs/verification/sqlite.md). The frontend has no SQL or game access. Help & logs provides an explicitly labelled 1,000-row fixture and a diagnostic workload. The launch action remains unavailable until game setup and launch are implemented.

Vitest checks stale revisions, retired subscriptions, lost acknowledgements, duplicate actions and cancellation state. Rust tests operation transitions, command validation and the generated contract. To regenerate types after editing `model.rs`, set `UPDATE_BINDINGS=1` for a Cargo test run, then unset it. Normal tests compare the checked-in types without writing them.

Run `npx playwright install chromium` once, then `npm run test:browser` for navigation, reconnect, keyboard, text resizing and diagnostic checks. Browser fixtures require the development URL `/?fixture`; production builds omit the fixture transport. These tests do not establish native behavior. On Windows, build with `npm run tauri -- build --debug --no-bundle -- --locked`, then run `npm run test:native`. These checks open and close their own debug app, temporarily enable WebView test connections on ports 9223 and 9224, and test real IPC, permissions, single-instance behavior, cancellation, reconnect, process exit, saved-data failures, game selection, the native folder picker and live process/build changes. They use new temporary data directories through `STARFRAME_TEST_DATA_DIR` and temporary Steam metadata through `STARFRAME_TEST_STEAM_ROOT`; release builds ignore both. The native picker test uses Windows UI Automation and messages scoped to its own app process. The ports must be free and no other Starframe instance may be running. Screenshots and timing data go under ignored `test-results/native`. No test connection is configured in the shipped app. CI builds the release executable and a separate debug executable for these isolated native tests. See [desktop results](docs/verification/desktop-state.md), [storage results and recovery](docs/verification/storage.md) and [game discovery/milestone exit](docs/verification/game-discovery.md).

For 0.1.1, retain the existing storage/native checks and add populated legacy conversion and interruption cases. Verify that the shipped Cargo graph excludes Turso. Record comparable clean and warm Windows builds with identical toolchain, target, profiles and commands; identify cache state and separate compile time from tests. The implementation milestone uses VERSION 0.1.1-dev.1. Do not weaken CI to obtain a faster result.

Conversion regression tests run with `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib storage::sqlite_proof`. They use [retained pinned Turso fixtures](src-tauri/tests/fixtures/turso-0.7.2/README.md), so Turso is absent from both current production and test graphs. Three ignored Rust test entries are subprocess workers invoked by parent interruption tests. The historical [proof](docs/verification/sqlite-conversion.md) records #39; [current verification](docs/verification/sqlite.md) records production behavior. The debug-only native startup gate requires STARFRAME_TEST_DATA_DIR and a hold-storage-startup file in that temporary directory. Release builds omit the gate.

Storage tests run as part of `npm run check:rust`. The process-interruption test starts the ignored `storage::tests::crash_worker` test in child processes, waits for a committed fixture and flushed unfinished work, then forcibly terminates each child. The ignored test is a worker entry point, not a skipped recovery check. Run `cargo test --manifest-path src-tauri/Cargo.toml --offline --locked --lib` to repeat the storage gate without registry access. Tests use temporary databases and never use the user's app-data or game directory.

The build icons were generated from [the approved SVG](docs/design/starframe-mark.svg) using Tauri's icon command. Only the PNG and Windows ICO needed for this build are retained. Final installer/taskbar asset review remains part of packaging.

## Tool references

- [Svelte tooling](https://svelte.dev/packages)
- [Clippy usage](https://doc.rust-lang.org/stable/clippy/usage.html)
- [.NET formatting checks](https://learn.microsoft.com/en-us/dotnet/core/tools/dotnet-format)
- [GitHub CLI issue creation](https://cli.github.com/manual/gh_issue_create)
