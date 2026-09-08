# Ordered and shared collection verification

Issue [#23](https://github.com/Mastervoliumpl/Starframe/issues/23), milestone 0.4.0. Acceptance date: 8 September 2026. This completes the internal collection workflow. No installer or public app release is published.

## Two-library and game acceptance

Two independent SQLite libraries used the same approved fixture catalog. The source contained one managed fixture and two Lua overlays. Its requested sequence was `fixture.lua.a`, `fixture.first`, `fixture.lua.b`; Lua A required the exact managed release. The exported version-1 document contained only the collection name and ordered references.

The destination initially contained the managed package and Lua A. Both had unavailable download URLs, so successful preparation required verification and reuse of their existing files. Lua B was absent. One import acceptance queued all three references, downloaded the exact **189-byte** Lua B ZIP over HTTPS, verified its archive and file hashes, and marked all entries ready. The HTTPS echo endpoint received only the synthetic Lua fixture archive encoded in the request URL. No game assemblies, user files or catalog publication were involved. Re-export from the destination matched the source export byte for byte.

The desktop deployed the destination collection through its normal ownership journal. Its effective order was `fixture.first`, `fixture.lua.a`, `fixture.lua.b`. Launching the installed Sanctuary playtest from the desktop produced a matching runtime report for deployment revision **11**, process **15820** and session `2ac3e8a6-877f-49c8-8d26-003a6f6a7076`. All three outcomes were `loaded`. The desktop displayed “Runtime confirmed 3 active mods.” The runtime verified Lua B as the final lookup winner for `LJ/lua/starframe_order_fixture.lua`, SHA-256 `83263df1c1bae0dde7547bffe0fb996f942f3d5ae340a952de31d2729b4b19fa`.

The game was installed build **25135612**, game **0.0.1.15**, Unity **6000.3.22**. Fixture metadata listed an older tested build. My mods displayed the untested-version warning while launch remained available; runtime activation succeeded. The native regression also changes a temporary Steam build from `111` to `222`, verifies the warning updates, and checks that launch remains enabled. It never changes the real game's Steam metadata.

While the real game was running, selecting an empty collection saved the change and displayed the waiting message. The deployed activation document remained unchanged. After stopping that test process, automatic application produced an empty activation. Selecting the imported collection again restored its three entries. The real per-mod settings file, `BepInEx/config/Starframe/mod.fixture.first.cfg`, retained identical bytes through both switches.

The desktop's Remove runtime action removed its owned integration and payload files after game exit. All **1,021** recorded original engine-root and Lua files retained their SHA-256 values. Generated settings, logs and reports remain under the existing retention policy. The run stopped only its own game processes.

Local evidence is under ignored `test-results/0.4.0/exit`: source export, destination review and operations, activation, process-bound report, game log, settings hash, before/after file checks and native screenshots. The acceptance harness needed corrections for fixture JSON serialization, startup waits and exact UI text; these were harness errors, not production changes. The game emitted Mono/Unity logging warnings on standard output, but the BepInEx log and process-bound report confirmed successful activation. These smoke checks do not establish general gameplay or multiplayer compatibility.

## Checks and help

The final change adds a native regression for live build-change warnings and launch availability. Existing native checks cover collection creation, rename, deletion, restart application, withdrawal, settings retention and unresolved imports. [Ordering](ordering.md), [named collections](collections.md) and [sharing](sharing.md) record the implementation checks, including keyboard/drag operation, forced colors, 200% text, exact references, cancellation and restart recovery.

Help & logs now explains requested versus effective order, keyboard use of the drag handle, Lua collision precedence, game-exit application, retained settings, compatibility warnings and current runtime limits. Its existing sharing help explains retry and repair of unresolved references without replacing them with newer releases.

Final validation includes repository/version checks and Python tests, frontend formatting/lint/type checks, frontend tests/build, the browser suite and the updated native lifecycle fixture. The milestone PR's required checks run the full Rust, C#, browser and native suites and the Windows executable build. Dependency audits remain a separate workflow. CI results are recorded on [PR #49](https://github.com/Mastervoliumpl/Starframe/pull/49) before merge.

## Known limits assigned to 0.4.1

The owner explicitly scheduled these four review findings after the 0.4.0 merge, in [milestone 0.4.1](https://github.com/Mastervoliumpl/Starframe/milestone/11):

- [#50](https://github.com/Mastervoliumpl/Starframe/issues/50): identical archive bytes reapproved under a new release ID can retain the old library reference. Sharing reports the unresolved identity; it does not repair storage or substitute another release.
- [#51](https://github.com/Mastervoliumpl/Starframe/issues/51): runtime inventory includes disabled library entries and rejects more than 256 mods.
- [#52](https://github.com/Mastervoliumpl/Starframe/issues/52): package preparation accepts up to 4,096 files, while runtime activation accepts only 1,024 per mod.
- [#53](https://github.com/Mastervoliumpl/Starframe/issues/53): two mod IDs sharing one archive have overlapping deployment roots and cannot activate together.

The external review reproduced these against the 0.3.0 merge. They remain unresolved here and must be reproduced and fixed on the patch branch. General BepInEx plugin compatibility, broader Lua packages and maps still require adapters and representative author-mod tests under [#30](https://github.com/Mastervoliumpl/Starframe/issues/30). Internal fixtures establish only the supported Starframe package workflow. Catalog/advisory signing and Windows distribution checks remain required before public access.
