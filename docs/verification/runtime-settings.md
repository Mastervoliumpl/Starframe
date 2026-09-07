# Runtime settings verification

Issue #14 is implemented on the 0.2.0 branch. It remains open for physical keyboard input and Windows display-scale acceptance checks. The desktop launch issue #15 has not started.

## Implemented behavior

`IModContext.Settings.Bind<T>` registers a stable key, display label, description, default and optional `live: true`. The internal contract supports `bool`, `int`, finite `float`, single-line `string` and named enum values. Keys use lowercase ASCII letters, digits, dots, hyphens and underscores; the first character is a letter. Labels can change without resetting saved values. A mod can register at most 128 settings; strings can contain at most 4096 characters. This is an internal contract experiment, not a published SDK or compatibility promise.

A binding's `Value` is the effective value for this game session. Mod code that opts into live changes reads that property when it needs the value. The UI shows the saved value. For a restart-only setting these can differ until the next game process starts. Returning the saved value to the effective value clears the restart requirement. Reset uses the registered default and follows the same live/restart rule.

The bootstrap stores each mod under `engine/BepInEx/config/Starframe/mod.<mod-id>.cfg`. BepInEx `ConfigFile` and `ConfigEntry<string>` provide the file format. Each value contains a JSON-encoded string so empty strings, punctuation and escaped characters round-trip without conflating them with missing entries. Typed parsing and defaults belong to the reference-free runtime core. The config path contains no collection identifier, deployment revision or display label.

Writes run on a worker, serialize access to the configuration files, save a temporary copy, flush it and atomically replace the original. Invalid typed saved values use the default with an explanation; they are not overwritten until an explicit save/reset. A failed save leaves the effective and pending values unchanged and displays an error. Reparse-point paths and files over 2 MiB are rejected. Generated configuration is retained by bootstrap removal. The desktop has no settings copy or merge operation.

The Mods entry and page reuse Sanctuary's menu, panel, button, switch, input and scroll components. The monochrome mark is an embedded 128-pixel PNG rendered from `docs/design/starframe-mark-mono.svg`; its geometry is unchanged, and the game controls apply their normal state tints. The label follows the adjacent menu entries' hover/focus behavior. The page uses the game's body font and panel background. Rows grow with text rather than clipping enlarged labels.

The list comes from the session's prepared metadata inventory. Enabled membership and activation results are shown separately. Disabled entries can be inspected without loading a DLL. Failed entries show their activation error; loaded entries show only settings registered by that successful activation. Back restores the list selection and scroll position, then returns to the main menu and Mods entry. Game-driven screen transitions hide the page. Membership changes remain a desktop operation followed by a game restart.

## Evidence on 7 September 2026

The local build and game checks used the installation recorded in [activation verification](runtime-activation.md): Steam build 25135612, game 0.0.1.15, Unity 6000.3.22 and BepInEx 5.4.23.5.

- All 86 core runtime tests passed. The added checks cover supported values, invariant numeric parsing, stable keys, defaults, reset, live/restart behavior, invalid data and failed saves. Activation checks also ensure disabled/failed mods do not appear in the editable registry.
- The Unity bootstrap built against the installed game references. MSBuild reports two existing framework-reference version conflicts from the game's dependency graph (`System.Net.Http` and `System.IO.Compression`). There are no C# compiler errors. The affected bootstrap then ran in the actual Mono process; this does not establish compatibility with future game builds.
- A temporary BepInEx probe captured the original main menu and Settings page before UI implementation. API/hierarchy inspection also used the installed assemblies and [Remmy's menu source at the inspected revision](https://github.com/Remmyboy/sanctuary-mods/blob/709562f3de3afc227cdc1c3b247fc9d808f1b0b0/ModManager/ModsPage.cs). No game assemblies or Remmy source were copied into the product.
- The game-side probe opened Mods through Unity submit events, selected a fixture, edited all five supported types, reset them to defaults, inspected a disabled mod, and verified Back and focus restoration. Reset also preserved the effective value of restart-only settings. It ran with the desktop absent. Config values survived fresh game processes. Assertions inside the game confirmed that a changed live float reached `Value`, while a changed restart-only integer retained its initial `Value`.
- Captures were inspected at 2560 × 1440 and 1280 × 720. A synthetic 200% text enlargement exposed clipped native row labels; after the fix, labels and controls remained visible and the lower settings could be reached by focus scrolling. This is text-layout evidence, not a Windows DPI test.
- Development captures, logs, probe source and prepared packages remain under ignored `test-results/ui-probe` and `test-results/ui-menu-*`. The probe is excluded from the product build and must not ship in a runtime package. Guarded removal cleaned up the recorded test integration afterward; original engine-root hashes matched the saved baseline, and generated settings remained.

## Remaining acceptance checks

Use the [guarded fixture preparation commands](runtime-activation.md#local-build-and-smoke-commands), with only product DLLs in the bootstrap output. Close the desktop before testing the game.

1. Reach Mods with the keyboard. Use Tab and Shift+Tab through settings, Enter/Space for controls and Escape/Back to return. Verify visible focus, input capture and focus restoration. Check that input cannot activate the menu behind the page.
2. Enter and correct invalid numeric text. Verify the error is visible and the last saved value remains intact. Test reset and the distinction between saved and effective restart-only values.
3. Check Windows display scaling at 100%, 150% and 200%, including narrow/windowed sizes, enlarged text and long mod names/descriptions. Verify wrapping, scrolling, selection and unfinished text edits.
4. Repeat after closing the desktop, then restart the game and verify the settings. Check the native appearance against the adjacent game menus.

The available desktop-control tool was not exposed in this session. Unity event dispatch and screenshot probes do not substitute for physical keyboard, OS display scaling, controller or screen-reader checks. No accessibility compliance claim is made. Keep #14 open until its remaining checks pass; main stays on the completed 0.1.1 milestone.
