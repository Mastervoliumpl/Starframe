# Named collections and automatic switching

Issue [#21](https://github.com/Mastervoliumpl/Starframe/issues/21), milestone 0.4.0. Verified on 8 September 2026 with development version `0.4.0-dev.1`. The milestone remains active; sharing and its exit checks are #22–#23.

## Saved collections

Collections stores a name, ordered exact mod references and identity/revision metadata. It contains no mod settings. Creation produces an empty, inactive collection. Use collection selects it; My mods also provides a native active-collection selector. Rename preserves entries and selection. Confirmed deletion removes only the collection record and its references. Deleting the active collection clears selection in the same SQLite transaction, so no other setup becomes active implicitly. Enabling a mod with no active collection retains the existing default-collection behavior.

These operations reuse the existing SQLite collection tables, storage owner and mod-command queue. No schema migration or dependency was added. Commands validate the expected saved-data revision, collection identity and name. Failed or stale changes retain the previous state. A selected collection with an uninstalled exact reference remains visible with an unresolved-order message; it cannot claim launch readiness.

Membership follows the selected collection. Switching neither prepares duplicate library artifacts nor changes settings. The existing single deployment worker applies the acknowledged revision when the game is stopped. During play, the launch area names the pending saved revision. If several edits arrive during play, the next application uses the latest saved state. If Starframe closes first, the next desktop start resumes from SQLite after the game exits. No background service is involved.

## Interaction review

The user requested removal of separate Move up/down buttons on 8 September. Rows now have a left drag handle and a right-aligned order number. Arrow keys, Home/End and select-then-place clicks use that handle and the same revision-checked reorder operation. Escape cancels a picked row. Dependency explanations and collision winners remain beside the effective order.

Collection tiles follow DESIGN.md: the name and mod summary identify a setup; the active setup has a check and an Active label. Selection retains its control in the DOM to preserve focus. Name forms use a native modal dialog and labelled input, keep typed text during refreshes, report invalid names, and restore focus on close. Deleting the focused tile returns focus to New collection. The active list has an action to open My mods for membership edits.

- PASS: keyboard tests cover handle movement, native collection selection, dialog focus, Escape, creation, rename and deletion. Accessible names and pressed states identify the handles and selected collection. A regression assertion verifies focus after Use collection. This review does not claim a manual NVDA or Narrator listening session.
- PASS: drag and select-then-place tests preserve membership and focus. Separate Move up/down buttons are absent.
- PASS: browser screenshots at normal and 200% text show the collection tiles and load order without horizontal page overflow; forced colors and reduced motion remain usable.
- PASS: the existing navy/orange palette and fonts remain in use (ENERGY 2 / RHYTHM 2 / MOTION 2). No decorative asset, animation, palette or library was added. Tiles identify saved setups; the right-hand number keeps order visible without a second control group.

## Verification

Local validation passed Rustfmt, Clippy with warnings denied, **82 Rust tests**, frontend formatting/lint/type checks, **10 frontend tests**, **15 browser tests**, and the Windows debug build. The two affected browser tests passed again after the final focus correction.

The Rust cases cover creation, rename, active/inactive deletion, exact membership after switching, restart persistence, unavailable references, invalid names/IDs, stale operations, repeated reorders and rejection of a payload plan from an older revision. Deletion leaves library artifacts and a settings fixture intact.

The native Windows lifecycle fixture uses an isolated fake Steam/game installation and inert mod payloads. It verifies collection creation and rename, switching to an empty setup and back, settings retention, and automatic game-exit application while the desktop stays open. It then switches repeatedly during a simulated running game and deletes the active empty collection. The current deployment stays intact, the launch area shows the final pending revision, and restart applies that final state after process exit. Selecting the retained original collection restores its membership without a new download. Existing dependency-order, withdrawal and uninstall-retention checks also pass.

Native screenshots and browser screenshots are under ignored `test-results/native` and `test-results/browser`. The separate real-game Lua and managed activation evidence remains in [ordering verification](ordering.md); this issue adds no gameplay, map or multiplayer claim. The milestone PR stays in draft.
