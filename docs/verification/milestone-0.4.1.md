# Milestone 0.4.1 verification

Milestone [0.4.1](https://github.com/Mastervoliumpl/Starframe/milestone/11) covers issues [#50](https://github.com/Mastervoliumpl/Starframe/issues/50), [#51](https://github.com/Mastervoliumpl/Starframe/issues/51), [#52](https://github.com/Mastervoliumpl/Starframe/issues/52), [#53](https://github.com/Mastervoliumpl/Starframe/issues/53) and [#54](https://github.com/Mastervoliumpl/Starframe/issues/54), through [PR #55](https://github.com/Mastervoliumpl/Starframe/pull/55). This is an internal corrective build, with no published app release or installer.

## Correctness and migration

Schema 10 preserves distinct catalog approvals and local references over the same archive hash. Completion, enablement, dependencies, UI selection and uninstall use exact references. Immutable artifact inventories remain deduplicated. Tests cover schema-9 migration, backup restoration, restart, old pinned references, reapproval, dependency selection and uninstall without removing shared content. Collection JSON remains version 1.

Activation schema 3 adds `omittedDisabledMods`. The session inventory includes all active mods, then disabled entries up to 256 distinct mod IDs. The game menu reports the omitted count. Library growth does not block a valid launch. Readers retain schema-2 support; the desktop and runtime must be rebuilt together to write/read schema 3. Tests cover 255, 256 and 257 library entries, empty and populated collections, restart, payload preparation, active-count rejection and retained settings.

Package preparation, cached reuse and enablement share runtime bounds: 1–1,024 files, 64 MiB per file, 16 MiB per managed assembly and 256 MiB per package. Archive-container safeguards still allow up to 4,096 entries, including directories; those are separate from runtime file limits. Historical oversized inventories remain readable for repair. Managed and Lua fixtures cover 1,024/1,025 files, directory entries, membership, activation and rejection before completion. A combined 256 MiB activation check also runs before payload preparation; the boundary test accepts exactly 256 MiB and rejects one byte more without changing saved records. Existing archive, cancellation, failed-commit and recovery tests remain in place.

Deployment roots hash the logical mod ID, a NUL separator and archive hash. Managed and Lua fixtures verify distinct roots over shared cached bytes, ordered payloads, import, restart and uninstall while another reference retains the artifact. The ownership journal still controls replacement, rollback and cleanup.

## Maintenance and checks

The worker retains one SQLite owner and serial game mutations. Request handling and launch-session transitions have separate modules; transition tests supply observations and time. Archive extraction, artifact storage and shared filesystem guards have separate responsibilities. Storage conversion and the larger storage/deployment test blocks moved to child modules. See [implementation boundaries](../../ARCHITECTURE.md#041-implementation-boundaries).

Desktop management types are generated from Rust with the existing ts-rs dependency. Production transport and fixtures import them. Rust tests reject generated-file drift. Shared [boundary cases](../../contracts/fixtures/activation-boundaries.json) exercise both runtime readers at inventory, active-mod, per-mod-file and aggregate-file limits.

Local verification passed: 102 Rust tests, 91 C# tests, 10 frontend tests, 18 browser tests and 18 Python tests. Four Rust subprocess-worker entry points are marked ignored and invoked by their parent recovery tests. Rustfmt, Clippy, Prettier, ESLint, Svelte checks, the frontend build and all six native Windows modules passed. The native payload assertion now follows the activation manifest's root rather than the previous hash-only directory. PR #55 retains required CI results, the Windows executable artifact and the separate dependency audit.

## Manual game acceptance, 8 September 2026

The rebuilt Unity entry plugin and runtime were checked against Sanctuary Playtest build 25135612, game 0.0.1.15 and Unity 6000.3.22, with BepInEx 5.4.23.5. Only an isolated acceptance library and managed fixture packages were used. The initial harness attempted concurrent preparation of one archive and read activation.json before setup completed; serialization and polling corrected those harness errors.

The final run installed new approvals for cached bytes, enabled two logical managed mods over one archive plus a Lua overlay, and launched from a library of 258 distinct mod IDs. The manifest contained 256 inventory entries and reported two omitted disabled mods. Process 9540, revision 31 and session `69769b29-db27-4f1a-91b6-e85087396b0c` reported `fixture.first`, `fixture.shared` and `fixture.lua.a` loaded in order. The desktop confirmed all three outcomes.

Exact export retained the new references. Selecting an empty collection during play left the current activation unchanged; the worker applied it after game exit. Switching back restored all three entries. Uninstalling the withdrawn old approval retained the enabled references and shared content. Per-mod settings were byte-identical after these changes. Guarded runtime removal completed, and all 1,021 recorded original game-file hashes matched their pre-test values. The test game and desktop processes were stopped.

The local entry-plugin build reports the existing Unity reference conflicts for System.IO.Compression and System.Net.Http. The game also emits its existing UnityLogWriter/Mono warning. The successful runtime report and guarded cleanup are the acceptance evidence; these warnings are not a claim that general mod compatibility is established. The new omitted-count menu copy was compiled; its layout was not separately captured in-game. Existing accessibility/browser checks cover the desktop, not Unity controls.

Conventional BepInEx plugin adapters, broader Lua/map support and representative author-mod acceptance remain in [#30](https://github.com/Mastervoliumpl/Starframe/issues/30). Catalog signing, advisory delivery and Windows distribution requirements remain unchanged.
