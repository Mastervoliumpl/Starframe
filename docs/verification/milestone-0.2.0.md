# Milestone 0.2.0 exit evidence

Version 0.2.0 completes issues #11–#15 through PR #43. The owner authorized the merge after approving the desktop and game settings screenshots. This is an internal build, without a published installer, release or public SDK.

## Delivered and verified

- [Contracts](runtime-contracts.md): shared manifest/report validation and internal C# lifecycle interfaces.
- [Bootstrap](bootstrap.md): pinned BepInEx preparation, ownership-aware installation/removal, process guards and journal recovery.
- [Activation](runtime-activation.md): managed fixtures load in the installed playtest, dependency failures propagate and runtime reports identify the game process/session.
- [Settings](runtime-settings.md): native game controls, five setting types, atomic per-mod persistence, live/restart semantics, reset and Windows keyboard smoke checks.
- [Desktop launch](game-launch.md): runtime setup/removal, revalidated executable dispatch and separate process/runtime confirmation; closing Starframe leaves the game independent.
- [Visual review](display-scaling.md): nine owner-supplied screenshots following the Windows scale procedure, with readable desktop and fixture settings. Oxanium 800 matches the brand assets; the launch label no longer has a background rectangle. The seven rendered artwork states exceed 4.5:1 text contrast (minimum 5.33:1).

The affected frontend passed formatting, lint, types, six unit tests, eight browser checks and a Windows debug build. Existing feature evidence records 44 Rust, 86 C# and 18 Python checks, plus native desktop and real-game checks. Required GitHub checks must pass on the final PR revision before merging.

## Exit findings and limits

The manual instructions set `STARFRAME_TEST_DATA_DIR` without `STARFRAME_TEST_STEAM_ROOT`. Debug isolation deliberately disables real Steam discovery in that case. For an isolated test against the local Steam library, also set the latter variable to its Steam root; ordinary app starts use registry discovery. Manual folder selection succeeded.

After the manual review, no Starframe or Sanctuary process was found, the test activation file was absent and the six original engine-root files matched their saved SHA-256 baseline. Earlier guarded removal and recovery checks remain the evidence for file ownership and interruption behavior.

The earlier external-game Windows-close check stalled and needed a verified test-process stop; a clean external-game Windows-close path is not claimed. Runtime reports do not establish that the main menu has loaded. Desktop collection preparation currently supports an empty collection; managed mod activation is verified through the development fixtures. Conventional BepInEx plugin adapters and Lua/map activation still need tested integration. AI remains deferred. Installer delivery and dependency notice inventory belong to 0.6.0.

Milestone 0.3.0 remains planned: independent curated catalog refresh, verified downloads/package preparation, mod lifecycle operations and live My mods/Catalog/Downloads screens. No 0.3.0 implementation is included here.
