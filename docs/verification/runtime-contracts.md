# Runtime contracts and build target

Issue [#11](https://github.com/Mastervoliumpl/Starframe/issues/11), milestone 0.2.0, checked on 6 September 2026. This change builds the owned runtime contract library and tests its file formats. It does not install BepInEx, invoke a game entry point or establish in-game compatibility.

The later #13 [game verification](runtime-activation.md) supersedes the original core target below: the core uses .NET Standard 2.0 with complete Mono dependency output, while the Unity entry plugin uses 2.1. Activation documents now use schema 2. The original #11 results below remain historical evidence.

## Inspected target

The installed Sanctuary playtest (Steam app 4511930, build 25135612) contains Unity 6000.3.22f1, MonoBleedingEdge and `netstandard.dll` 2.1.0.0. The read-only inspection recorded these SHA-256 values:

| File beneath the game engine directory | Version | SHA-256 |
| --- | --- | --- |
| UnityPlayer.dll | 6000.3.22f1 (1c726e1fb402) | ccb4ff1c1bff882e6acc84d98ef992ff94dfa7685173e5c7669619127ee20335 |
| MonoBleedingEdge/EmbedRuntime/mono-2.0-bdwgc.dll | No file version resource | d17f60ae00d8ac2879d1be92dcc701f72b2a273f91424af19310ada84952e3b9 |
| Sanctuary_Data/Managed/mscorlib.dll | 4.6.57.0 | 6c76e06e6d44cf0f39bf59e6a90770968b70093ad4c1572dfbc5e46e431c5fbf |
| Sanctuary_Data/Managed/netstandard.dll | 2.1.0.0 | 6ae62e082dc494a2433984177f60ca4db5fae69b1f360a8b33754172b310b8c5 |

The runtime project targets `netstandard2.1`. The installed .NET 10.0.400 SDK compiles it; .NET 10 is only the fixture test host. Tests assert the compiled runtime target and informational version. This follows the [BepInEx target-selection guidance for modern Unity](https://docs.bepinex.dev/master/articles/dev_guide/plugin_tutorial/2_plugin_start.html). BepInEx 5.4.23.5 is the bootstrap candidate; its [official release](https://github.com/BepInEx/BepInEx/releases/tag/v5.4.23.5) includes a Unity 6 logging fix. No BepInEx build is installed in this inspected game. Candidate selection is not a successful bootstrap test; #12/#13 must verify loading and dependency resolution on this exact game build.

The game also ships Newtonsoft.Json 13.0.2. The contract reader uses pinned System.Text.Json 10.0.11 because its strict JSON parser matches serde_json without a custom JSON lexer. NuGet selects its .NET Standard assets and locks transitive hashes. Packaging must verify these dependencies alongside the game's assemblies before claiming in-game compatibility. No game DLL is copied, referenced by the project or redistributed.

## Build references and licensing

The contract library needs only the SDK's .NET Standard reference pack and packages from the official NuGet feed. A clean CI runner can build the target without access to Sanctuary. Package versions and hashes are in the runtime lockfiles. The later BepInEx adapter must use official BepInEx build references. Any direct Sanctuary/Unity reference must come from a developer's lawful installation with copying disabled, or an explicitly redistributable reference source; an unofficial community game DLL package is not assumed lawful. Required game-specific builds must wait for that acquisition path rather than silently skip.

Repository AGPL-3.0-only terms remain unchanged. Internal `IMod`, `IModContext` and optional `IModShutdown` interfaces are compiled for experiments; `IsPackable=false` prevents SDK packing, and CI publishes test results only. Initialize/logging/shutdown signatures establish the small lifecycle boundary. Scoped configuration and settings registration are added with their implementations in #13/#14. No stable mod-author API, linking exception or alternative license is adopted. Public SDK publication remains blocked on a separate review of mod linking and the bootstrap/reference redistribution arrangement. [Dependency notices](../THIRD_PARTY_NOTICES.md) record the current packages; installer notices remain a later deliverable.

## Verification and limits

Both Rust and C# read the same 73 positive/negative JSON fixtures. They cover versions, disabled inventory, activation consistency, dependency order, file/root aliases, local content identity, capability reasons, report fields and malformed JSON. Canonical inventory IDs agree across languages and reordered inventories. Separate checks reject oversized input and assert the runtime target/version. The C# fixture host runs 75 tests; none needs a game or database.

The runtime CI job restores locked packages, verifies formatting, builds with compiler/analyzer warnings treated as errors and runs the fixture tests. The stable Required checks status includes this job. Rust's existing checks run the shared fixtures as well. Local validation commands are in [DEVELOPMENT.md](../../DEVELOPMENT.md#c-runtime-development).

Filesystem containment, hash verification against disk, BepInEx startup, assembly resolution, lifecycle execution, report correlation and game settings remain later issues in this milestone. The milestone stays on its branch until those exit checks pass. The public SDK remains unpublished.
