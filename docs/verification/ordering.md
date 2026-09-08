# Dependency ordering and Lua overlays

Issue [#20](https://github.com/Mastervoliumpl/Starframe/issues/20), milestone 0.4.0. Verification date: 8 September 2026. Development version: `0.4.0-dev.1`. The milestone remains active; named collections, sharing and milestone exit checks are #21–#23.

## Requested priority and effective order

The resolver validates exact selected release identities, builds dependency and mandatory before/after edges, then chooses the earliest requested position among ready mods. It reports a cycle as the involved mod IDs in edge order. Missing requirements identify the exact release and the dependent mod. Optional before/after preferences produce warnings; they neither change user priority nor block a valid order.

SQLite collection entries retain requested positions. Activation preparation resolves them without rewriting that list and records the effective sequence in the deployment manifest. Reordering accepts each enabled mod exactly once and requires the current library revision. It cannot add, remove or substitute a mod. Enabling an already selected dependency preserves its priority. Invalid collections retain their requested list for repair and cannot claim an effective order or launch readiness.

Collections shows effective numbers, requested positions and adjustment messages beside affected rows. Move up/down and drag handles submit the same reorder command. Focus returns to a moved control if moving its DOM node caused focus to fall to the document body; later navigation or deliberate focus changes take precedence. Pending edits restrict collection mutations while navigation remains usable. Conventional BepInEx plugins are not accepted by the package format, so these controls cannot imply control over their activation.

## Supported Lua content

Catalog schema 2 adds `starframe_lua_zip`. Packages contain only `.lua` files with complete `LJ/lua/...` paths. The existing archive and deployment validation still apply. Files stay under Starframe's owned payload directories; the adapter changes the in-memory game cache, not shipped Lua files. Managed DLL companion content does not become an overlay automatically.

Read-only inspection of installed build **25135612**, game **0.0.1.15**, Unity **6000.3.22**, identified `EM.Lua.FilesCache.CreateFileCache`, `pathToFileContents` and `TryGetFileContent`. The adapter activates after cache construction. Each Lua mod receives verified bytes captured by the runtime; a later effective mod replaces the earlier cache entry for a case-insensitive path. Original/replaced arrays remain alive until game exit because an earlier mod can retain a reference. The game retains ownership of the current arrays. Collections lists colliding mods and the expected winner; the runtime verifies the game's lookup bytes and logs the winner with its SHA-256 before reporting success.

Target directories must already exist in the game. The game's development file watcher must be disabled, since it can replace cache entries during play. Unsupported paths or an enabled watcher fail activation visibly. AI paths, maps, map-local textures and MapLocalFiles remain unsupported. This preserves the distinction in [map evidence](../planning/map-support.md); it makes no map-playability or multiplayer compatibility claim. The existing runtime limits remain 64 MiB per verified file and 256 MiB total activation content. Arbitrary managed mods and Lua run with the game's privileges; this is not a sandbox.

## Game fixture evidence

`scripts/prepare_ordering_fixture.py` prepares two managed fixtures and two Lua fixtures that collide at `LJ/lua/starframe_order_fixture.lua`. The path is in the existing Lua root and is never written to the game's Lua folder. A second package reverses only the Lua entries. Both packages deploy through the existing ownership journal and use fresh game processes.

| Deployment revision | Effective Lua sequence | Verified final lookup winner | Runtime outcomes |
| --- | --- | --- | --- |
| `400` | `fixture.lua.a`, `fixture.lua.b` | `fixture.lua.b`, SHA-256 `83263df1c1bae0dde7547bffe0fb996f942f3d5ae340a952de31d2729b4b19fa` | Both managed and both Lua entries loaded |
| `401` | `fixture.lua.b`, `fixture.lua.a` | `fixture.lua.a`, SHA-256 `1978773f1487d75db17cd8ad8326037fba801f0bf870b76e7c9424be3e4206d5` | Both managed and both Lua entries loaded |

Reports matched the launched process IDs, separate session IDs and deployment revisions. Managed initialization messages followed the manifest sequence in both launches. These checks establish runtime activation and the game's content lookup; they do not execute a gameplay scenario or verify multiplayer hashes.

The test stopped only its own game processes. Removal initially deferred while Windows still exposed an exiting process, then succeeded on retry after exit. The journal removed 32 owned files; all **1,021** recorded engine-root and original Lua-file hashes matched afterward. Generated configuration, logs and reports remain under the existing retention policy. Local reports, logs, hash records and screenshots are under ignored `test-results/0.4.0`, `test-results/native` and `test-results/browser`.

## Automated and desktop checks

The final local suites passed **79 Rust tests** (plus four subprocess-worker entry points invoked by their parent tests), **87 C# tests**, **10 frontend tests**, **14 browser tests** and **18 Python repository/fixture tests**. Rustfmt, Clippy with warnings denied, frontend formatting/lint/types and the Windows debug build passed. The full browser suite also passed while .NET formatting ran, exercising the watcher correction.

- Rust tests cover stable priority, reversed order, exact identities, missing requirements, cycles, optional preferences, schema compatibility, immutable ordering metadata, stale/invalid reorder requests, restart persistence, collision winners and Lua path preservation. Existing deployment, rollback and interruption checks remain enabled.
- C# tests cover activation order, both Lua winners, unsupported map/AI content and changed bytes. The reference-free runtime suite passes 87 tests. The separate game adapter build succeeds against local compile-only references; MSBuild reports the existing System.IO.Compression/System.Net.Http reference-version conflicts. No game assemblies are copied into the package.
- Browser tests exercise keyboard moves, drag moves, focus retention, 200% text, reduced motion and forced colors. A regression test caught focus loss during row movement and now verifies its restoration.
- Concurrent .NET formatting initially crashed Vite's Windows watcher on a locked `runtime/**/obj` cache file. The watcher now excludes runtime `bin`/`obj` output as well as native Rust output.
- The native Windows lifecycle fixture enables a managed dependency and a Lua mod, reverses requested priority, verifies the dependency-first activation manifest and inspects the live Collections view. Its other checks retain game-running deferral, restart application, installed withdrawal and uninstall/settings retention.

UI checks use the accepted navy/orange direction, ENERGY 2 / RHYTHM 2 / MOTION 2, existing fonts and control styles. PASS: controls use native buttons, visible focus and explicit labels; the keyboard/drag regression passes. PASS: the enlarged-text test has no horizontal page overflow, including forced colors. PASS: native screenshots confirm readable positions and dependency explanations. Existing opaque text pairs retain the contrast values recorded in DESIGN.md; this view adds no new palette or animation.

The milestone PR stays in draft. An installer, public catalog opening, named collection management, sharing and in-game map support are outside this issue.
