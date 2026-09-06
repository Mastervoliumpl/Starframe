# Starframe development and checks

Status: development policy adopted on 6 September 2026. The 0.0.1 handoff is complete; creating issues and CI does not authorize application implementation.

Read [DESIGN.md](DESIGN.md) for behavior and presentation, [ARCHITECTURE.md](ARCHITECTURE.md) for structure and recovery rules, and [ROADMAP.md](ROADMAP.md) for the current milestone. GitHub issues hold task scope, dependencies, acceptance criteria and verification evidence.

## Work one milestone at a time

The **0.0.1** handoff is complete. No implementation milestone is active; **0.1.0** is next when the user starts development. Later milestones are planned work, not permission to begin implementation. Every project issue belongs to one version milestone. Give a new issue a milestone before starting it. Bugs found during a milestone belong there if they prevent its intended outcome; otherwise assign a later version explicitly.

Work on an issue only when its milestone is active and its prerequisites are complete. Keep issue dependencies in a `Depends on` section with issue links. Each issue must state its scope, observable completion criteria and checks. Split an issue when it contains independently reviewable outcomes; avoid splitting one small change into tasks that cannot be tested separately.

Subagents may work on independent issues within the active milestone when delegated. Agree on shared contracts before parallel implementation, give each worker distinct files where practical, and integrate with the same checks as other changes. A blocked issue does not permit starting a later milestone. This policy does not start agents or tasks by itself.

Close an issue only after its acceptance criteria and relevant checks pass. Closing a milestone also requires its exit checks, recorded unresolved limitations, and the user's review where the design calls for it. Update the active milestone in ROADMAP.md and these instructions in the same change that advances the plan. Do not carry unfinished work forward silently. Future native integration remains blocked until the game developer publishes a usable interface.

## GitHub and local work

Use the installed GitHub CLI (`gh`) for repository inspection, issues, milestones, pull requests, workflow runs and releases. Use `git` for local branches, commits, fetching and pushing. Inspect existing objects before creating new ones. Use `--body-file` or an API JSON input file for multiline text; do not construct shell commands from issue text.

Make small, coherent commits as work reaches a checked state. Reference the issue in the commit or pull request. Use an issue branch for implementation and a focused pull request into `main`. Preserve unrelated changes. Stage explicit paths. Machine-local AGENTS.md instructions stay out of commits and out of .gitignore.

Pull requests identify the issue, target milestone, behavior changed, verification performed, and any data-migration or release impact. Tests and fixes belong in the same change. Use the CLI to inspect CI failures; do not merge a known failing change. Require the stable `Required checks` status on `main`, without imposing a second-reviewer requirement on a sole maintainer. The maintainer retains administrative recovery access; routine work must pass checks.

## Test as features are built

Add useful tests with implementation, not at the end of the project. Each bug fix gets a regression test where the failure is reproducible. A milestone's final verification checks how its pieces work together; it does not replace tests for those pieces.

Prefer observable behavior over tests of private function shapes. Use unit tests for deterministic rules such as ordering, version comparison and validation. Use temporary-file integration tests for storage, deployment, archive handling and recovery. Use frontend interaction tests for stale replies, pending states, navigation and accessible controls. Avoid snapshots of entire screens, blanket coverage percentages, and tests that only repeat a constant or a CSS declaration.

The initial repository contains documentation and its CI tooling. Run `python scripts/check_repository.py` and `python -m unittest discover -s scripts -p 'test_*.py'` for the checker regression tests. Add each application's language tooling when its first source project arrives, and make the applicable checks pass before that project's first merge. Do not report absent language tests as passing.

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

Use pull-request and `main` push workflows for required checks. The current workflow validates tracked documentation links, fenced blocks, SVG XML, the version format and exclusion of machine-local instructions. It runs without project dependencies or access to game files.

Extend CI as source projects arrive. Keep one stable final status, `Required checks`, that fails if any applicable job fails or is canceled. If language jobs use path filtering, a small final job must still report a result for documentation-only changes; skipped workflows must not leave a required check pending forever. Shared contracts, lockfiles and workflow changes trigger all affected jobs.

Run fast independent checks concurrently. Cancel superseded PR runs, use finite timeouts and cache dependencies using lockfile keys. Start with one supported Windows target and a Linux runner for portable checks; expand the matrix only for a supported platform or a demonstrated failure. Upload useful failure logs and test results, omitting secrets and personal paths. Avoid requiring live external downloads for deterministic tests; use fixtures and separate scheduled/explicit source-availability checks.

Use read-only permissions by default, immutable action commit pins, and isolated release credentials. Never run untrusted pull-request code with release secrets or a privileged `pull_request_target` job. Dependency update PRs should be grouped weekly, with a small open-PR limit. Add npm, Cargo and NuGet updates when their manifests exist. Triage advisory findings by affected version, reachability and impact; record a reason and expiry for exceptions. Do not ignore applicable security findings just to make CI green. [GitHub workflow security](https://docs.github.com/en/actions/reference/security/secure-use), [Dependabot configuration](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference)

Continuous delivery first creates reviewable artifacts and draft releases. Publishing a release remains a maintainer action. Build and sign only from trusted repository revisions after required checks pass. Add the release workflow with the installer/updater milestone; no placeholder workflow should claim to publish a working app now.

## Versions and change history

[VERSION](VERSION) is the source of the product version. The current value is `0.0.1`, the completed design handoff, not a shipped app. The initial planning baseline was `0.0.0`. [CHANGELOG.md](CHANGELOG.md) records completed changes under `Unreleased` until a version is finalized.

Use three-part versions: `0.MINOR.PATCH` during initial development. A capability milestone advances the minor version; a corrective release advances the patch version. The planning handoff uses `0.0.1`. Published content is immutable; never replace a release with different bytes under the same version. The `0.x` series makes no stable public API promise, but format migrations and compatibility changes still need explicit notes. [Semantic Versioning](https://semver.org/)

At the start of an implementation milestone, use its target version with a development suffix, such as `0.2.0-dev.1`. Increment the suffix for a newly distributed preview build, not every local commit; use the commit SHA to identify ordinary CI artifacts. Release candidates can use `0.2.0-rc.1`. Strip the suffix only when that version's release checks pass. Planned milestone titles are targets, not evidence of releases. Internal milestones need not publish installers.

When build manifests exist, derive or check package.json, Cargo package metadata, Tauri's app version, and C# informational version against VERSION. Keep installer-specific numeric representations consistent with their platform requirements. A tested release-preparation command should update all required representations; CI rejects drift. Keep database, catalog, collection and runtime-contract versions separate from the app version. Catalog edits advance catalog revision without an app-version bump.

For a release, finalize its changelog entry, validate version agreement, and create an immutable `vX.Y.Z` tag from the checked commit. Use `gh` to create or inspect the draft release and workflow results. Publish after its acceptance checks and maintainer authorization. Do not fabricate changelog entries for work that is only planned.

## Tool references

- [Svelte tooling](https://svelte.dev/packages)
- [Clippy usage](https://doc.rust-lang.org/stable/clippy/usage.html)
- [.NET formatting checks](https://learn.microsoft.com/en-us/dotnet/core/tools/dotnet-format)
- [GitHub CLI issue creation](https://cli.github.com/manual/gh_issue_create)
