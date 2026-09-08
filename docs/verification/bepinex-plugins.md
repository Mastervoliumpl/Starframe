# BepInEx plugin verification

Issue [#57](https://github.com/Mastervoliumpl/Starframe/issues/57) adds existing BepInEx 5 Unity/Mono plugins to PR #56 and milestone 0.5.0. The [usage guide](../bepinex-mods.md) describes local metadata, configuration and compatibility limits.

The bridge uses BepInEx metadata, the plugin registry, configuration and Unity component lifecycle. It adapts the declared entry after Starframe verifies the payload. It does not scan other plugin folders or restart BepInEx's chainloader. The integration remains pinned to BepInEx 5.4.23.5; its internal `PluginInfo` setters are accessed through the existing Harmony dependency.

## Completed checks

- The reference-free runtime suite passes all 91 tests. The activation regression covers preparation before initialization, the exact verified entry path, adapter rejection, startup failure, dependent skipping and cleanup.
- The Unity bootstrap builds against the installed game references. Two existing framework-reference warnings remain; the build has no errors.
- Local BepInEx fixture components compile from [their test project](../../runtime/Starframe.BepInExFixtures/Starframe.BepInExFixtures.csproj). They require lawful local game and pinned bootstrap references and are not substituted with stubs in CI.
- Installed-game checks loaded a valid plugin and its dependency. Missing dependencies, reversed dependency order, a process filter, a mismatched GUID and symmetric incompatibility produced the expected failures. An unhandled `Awake` exception appeared in the activation result with its fixture message.
- The in-game settings page rejected count `51` against a `0–50` constraint, saved `41` and retained `41` on restart. BepInEx setting callbacks run on the game thread because plugins may call Unity APIs. BepInEx performs its normal synchronous configuration save.
- Remmy's unchanged Ladder Reporter 0.3.0 loaded from a local import. The desktop confirmed one active mod and the game's Mods page listed it as loaded. The owner later reported successful play with Remmy. The [catalog review](ladder-reporter-catalog.md) records exact identities and distinguishes that gameplay report from catalog download verification.

These game checks used Sanctuary Playtest build `25135612`, game `0.0.1.15`, Unity `6000.3.22` and BepInEx `5.4.23.5`. Local evidence remains under ignored `test-results/0.5.0-bepinex`. No game assemblies, mod binaries or private logs are distributed.

## Remaining acceptance

The owner stopped the automated game walkthrough to play with Remmy. Its final Ladder-specific settings restart and guarded uninstall/cleanup steps did not complete. The earlier #26 workflow's 1,021-file cleanup verification remains recorded separately; it is not evidence that the interrupted #57 cleanup passed. PR #56 remains draft while remaining acceptance and checks are completed.

Startup confirmation covers component creation and unhandled `Awake`/`OnEnable` failures. It cannot establish every later callback, network operation or game result. Broader author-plugin and public alpha acceptance remains in #30.
