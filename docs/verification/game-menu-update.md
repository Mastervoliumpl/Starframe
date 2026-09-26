# Current-game Mods menu correction

Issue [#93](https://github.com/Mastervoliumpl/Starframe/issues/93), corrective milestone [0.7.1](https://github.com/Mastervoliumpl/Starframe/milestone/12), development version `0.7.1-dev.1`.

## Failure and scope

The owner supplied Sanctuary's 0.0.1.20 changelog. The current game's AI version file reports V62, and the isolated live probe confirms `Application.version` is `0.0.1.20`. Its `Trebuchet.dll` hash is `6409eae81b10bb0ef15aa1cd22a360b413e8cfd20978ab7829f524a162dc683e`.

The old adapter references `MainMenuInterface`, which is absent from the current assemblies, and `InterfaceManager.Window.Main`, now replaced by `Home`. The sidebar is a separate `SideBarInterface`. The serialized scene retains the inspected settings control paths but places Settings under `RightSide Windows`, beside the sidebar.

The fix uses `SideBarInterface.settingsButton` and `SettingsInterface.Instance.transform`. It creates clones beneath an inactive temporary parent, removes the native Settings component before activation, and retains the original singleton. It validates templates before construction and removes partial or superseded pages/entries. Constructor patch failure removes installed hooks. The menu lifecycle records a construction attempt so the same broken owner is not retried every frame.

Both the game's pause-menu action and Starframe's Update can receive Escape. While Mods is open, the game toggle is intercepted and routes through the same once-per-frame Back handler. A second callback in that frame is suppressed even if the first closes the page. The next press returns from the list to Home and restores focus to Mods. Other game transitions retain the existing hide behavior.

No registry, storage, deployment destination, Lua class implementation or dependency changes are included. The inspected Lua cache API still matches the adapter; the game still references AI packages beneath `AI/mods` and discovers `.sanmap` files. Those facts do not establish individual mod or content gameplay compatibility.

## Checks and evidence

The game UI and its API changed, so the former menu smoke evidence cannot establish this correction. Focused current-game verification is required. Existing registry, desktop, installer and file-recovery evidence remains applicable because those implementations and dependencies are unchanged.

- The reference-free runtime solution builds with the pinned .NET 10.0.400 SDK. All 116 tests pass, including three new menu lifecycle regressions: partial-construction cleanup with no same-owner retry, cleanup before owner replacement, and once-per-frame Escape in either callback order.
- Runtime solution and game-specific Bootstrap formatting checks pass. The Bootstrap builds against the current local game references with zero errors and the two existing framework-reference warnings. Proprietary references are not copied into the product or repository.
- The initial test run passed 115 tests but failed the existing subprocess activation test because its child resolved the system SDK instead of the pinned SDK. Setting the pinned SDK root and PATH corrected the environment; the full rerun passed unchanged.

- The controlled Unity probe passes on an isolated copy of Sanctuary 0.0.1.20. It checks the original Settings singleton/sidebar control, loaded and failed session entries, disabled-entry exclusion, supported settings construction, save/reset, the native Escape callback in the same and subsequent frames, restored entry focus, ordinary Settings/Home/Play transitions, owner recreation without duplicate entries, an injected empty-session list, and menu disposal. These checks invoke native controls and transitions; they do not establish physical keyboard or rendered visual acceptance.
- A fresh game process retains the fixture's saved setting. Conventional BepInEx good/dependent fixtures start in order. The Lua fixture is available through the initialized game's file cache with the expected winner hash; no shipped Lua file is modified. No match or gameplay is exercised.
- The game probe redirects the game settings path to its ignored fixture directory. Only the copied game receives guarded test deployment; the owner's installation and settings are not modified. Probe source, game copy, logs and captures remain ignored.
- Physical keyboard automation could not acquire the test game as the foreground window. It sent no input and is recorded as skipped. Captured frames remained on the connection/loading view and do not prove the added menu's appearance. A short owner visual/keyboard smoke remains before marking the corrective issue complete: open Mods, inspect the list/settings, use Tab/Shift-Tab and two Escape presses, then open ordinary Settings. Current UI scale readability also remains part of that check; no broad display matrix is required.
- A fresh source-bound runtime archive builds from commit `fa4a305`, verifies against its source/hash manifest and closed DLL inventory, and stages through the existing packaging scripts. Runtime resource preflight and dependency notices pass. Desktop notice input hashes are refreshed only after verifying that all four manifest differences are product-version substitutions; dependencies and notice texts are unchanged.

Local logs, tooling, game-copy fixtures and test-only probe code remain ignored. The packaged/staged product runtime contains no fixture or probe DLL. The PR remains in draft until the owner smoke and required hosted checks are satisfied.

## Limits

The former review kit's reused 0.6.0 Bootstrap is superseded only by a freshly built and verified candidate. No installer, release or tag is published by this PR. Production registry trust provisioning and hosted Steam readiness remain separate requirements. Map/AI gameplay and author plugins that patch orders, replay serialization or rendering remain outside this corrective acceptance.
