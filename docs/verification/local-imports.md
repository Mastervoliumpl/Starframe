# Local mod imports

Issue [#24](https://github.com/Mastervoliumpl/Starframe/issues/24), milestone 0.5.0, internal version `0.5.0-dev.1`. This issue adds one-time imports. Automatic source watching remains in #25, and the complete developer/game workflow remains in #26. No installer or public release is produced.

## Import a build

In My mods, choose **Import local mod**, then choose a DLL or its build output folder. You can also paste an absolute source path. Choose **Import copy** and check Downloads for the result. After completion, use the normal switch to enable the mod in the active collection.

A folder must contain `starframe.local.json` at its root. A standalone `Mod.dll` must have `Mod.starframe.json` beside it. A DLL import copies that DLL only; use a folder when the mod needs other assemblies or content files. Starframe reads the metadata without executing the DLL. The same supported layout rules apply to catalog packages and local builds.

Example metadata for an internal Starframe-managed fixture DLL:

```json
{
  "schemaVersion": 1,
  "modId": "fixture.local",
  "name": "Local fixture",
  "author": "Test fixture",
  "version": "dev.1",
  "layout": {
    "kind": "starframe_managed_zip",
    "root": "",
    "entryAssembly": "Mod.dll",
    "entryType": "Fixture.Mod"
  },
  "requires": [],
  "loadBefore": [],
  "loadAfter": [],
  "preferBefore": [],
  "preferAfter": []
}
```

The layout names are shared with the existing package format. A local import does not need a ZIP archive. For a Lua-only folder, set `layout` to `{"kind":"starframe_lua_zip"}` and put only supported `.lua` payloads under `LJ/lua/`. AI and map content remain unsupported. A managed folder may include supporting assemblies and supported Lua content; its entry assembly must exist and be nonempty. These checks do not establish that an arbitrary DLL implements the runtime interface or will load successfully. Conventional BepInEx plugin adapters and representative author-mod acceptance remain in #30.

`modId` remains stable across builds. `requires` contains exact collection-style references with `modId`, `hash`, `origin` and `releaseId`. Local references use `origin: "local_import"` and `releaseId: null`; catalog dependencies name their exact approval. Import or install those dependencies before enabling the dependent mod. Enabling adds the required builds already present in the library. Missing builds, conflicting versions and cycles remain errors. Required ordering wins over manual priority; optional preferences produce explanations.

## Managed files and recovery

Imports read source files through the shared filesystem guards, reject links/reparse points and case-colliding or unsafe paths, and keep directory handles while copying. Windows read handles deny writes and deletion during the snapshot. A locked compiler output fails the import; retry after the build finishes. Source enumeration, hashing and copying run in the package worker, outside the UI and database thread. Copy hashes and the final file list must match the inspected source.

Limits are 64 KiB of metadata, 4,096 directory entries, 1,024 runtime files, 64 MiB per file, 16 MiB per DLL and 256 MiB per build. Activation also retains the existing combined 256 MiB and 256 active mod limits. Disk-space checks preserve the existing working margin. Copies stage under app data, then promote into `artifacts/<hash>`. Existing content must verify before reuse; Starframe does not overwrite a changed artifact.

The local hash covers the normalized manifest and sorted file paths, sizes and SHA-256 values, with a `starframe-local-v1` format prefix. JSON formatting and source locations do not change the hash. Changing payload or activation metadata changes it. The runtime's existing `source.contentId` still hashes the activation file inventory; no runtime contract change is required.

SQLite schema 11 stores source paths and manifests in `local_sources`. The library reference, source record, prepared inventory, operation completion and saved-data revision commit together. A failed commit retains verified managed bytes for a later retry. Closing the app cancels current workers; startup marks unfinished operations failed. Uninstall removes the exact library/source record and uses the existing managed inventory for cleanup. It never uses the source path as a deletion target. Other library references retain shared artifacts, other collections retain unresolved references, and per-mod settings stay in place.

Shared collections contain exact references and order. They exclude source paths, manifests, settings and binaries. Matching imported builds are verified locally; unmatched builds remain local requirements. Local-only enablement, ordering, sharing and activation preparation work with no catalog cache. The app's independent catalog refresh remains unchanged.

## Verification

The focused Rust tests are `packages::local::tests` in [the import tests](../../src-tauri/src/packages/local/tests.rs). They cover DLL and folder imports without a catalog, dependency enablement/order, activation and managed payloads, restart persistence, matching content across folders, exact re-export, unresolved local requirements, invalid metadata/layouts, cancellation, commit rollback/retry, junctions, locked outputs and source retention after uninstall.

[Browser tests](../../tests/browser/local-import.spec.ts) exercise native-dialog semantics, both source choices through the fixture transport, retained errors, local details without catalog compatibility UI, ordinary enable/uninstall, focus return, source copying, and reflow at 1280, 853 and 640 CSS pixels with forced colors and reduced motion. Browser fixtures do not read local files.

[The native test](../../tests/native/local-import.mjs) uses an isolated app-data directory, a temporary Lua source and blocked outbound HTTP. It checks the Windows folder picker, import through the UI, confirmed enablement, details, restart persistence and uninstall while retaining the modified author source and its metadata. It does not select, modify or launch the user's game.

Run the frontend checks, Rustfmt, Clippy and locked Rust tests using [DEVELOPMENT.md](../../DEVELOPMENT.md). Build the debug Tauri executable, then run `node tests/native/local-import.mjs`. The native check is also part of `npm run test:native`.

Local results on 8 September 2026:

| Check | Result |
| --- | --- |
| Repository links, versions and instruction exclusion | Passed; 18 script regression tests |
| Frontend formatting, lint, Svelte/TypeScript and build | Passed; no compiler warnings; 10 unit tests |
| Local import, management, ordering and sharing browser tests | 9 passed |
| Rustfmt, Clippy and locked Rust suite | Passed; 108 tests, plus 4 subprocess entry points run by their parent tests |
| Focused import safety and recovery tests | 6 passed |
| Windows debug Tauri executable | Built successfully |
| Existing native desktop, storage, game, catalog, package and mod workflows | All 6 scripts passed with isolated fixtures |
| Native local import workflow | Passed after changing the test to wait for confirmed switch state |

The native source-picker helper uses the same process-scoped PowerShell policy as the existing game-picker test. Neither test changes the machine's execution policy. Logs and reviewed screenshots are retained in the ignored `test-results` directory. CI results belong to the milestone draft PR.

## UI review

The import uses the accepted desktop manager design: navy surfaces, orange primary action, system body text and existing dialog/control spacing. Energy 1, rhythm 1, motion 1. Source selection is the dialog's focus; the existing frame-and-sun identity and library rows remain the surrounding context. No new animation, color, icon or layout system is introduced.

The dialog uses native buttons, a labelled text input and a modal `dialog` for keyboard focus and Escape. Local details show a wrapping path with copy/open actions. The source path opens its folder through a saved local reference. Downloads retains failed/cancelled import messages and tells the user how to retry.

Review result: pass for this issue's desktop controls. The forced-colors dialog at 640 CSS pixels and native details at a 1280 by 800 CSS viewport fit without clipped controls. Keyboard opening, Escape, focus return, source selection and retained errors passed. The native WebView2 run used a device pixel ratio of 1.5. A repeated local-origin label found in the screenshot was removed and the related browser tests passed again.

Native OS display/text scaling, screen-reader behavior and game loading of these imported builds are not established by the browser tests. The full milestone exit workflow remains in #26.
