# Use BepInEx plugins with Starframe

Issue [#57](https://github.com/Mastervoliumpl/Starframe/issues/57) adds BepInEx 5 `BaseUnityPlugin` entries to the internal 0.5.0 runtime. Authors do not need to implement Starframe's experimental `IMod` interface. Starframe uses the plugin's existing BepInEx metadata, configuration and Unity callbacks.

## Import Ladder Reporter 0.3.0

Use Remmy's [Ladder Reporter 0.3.0 release](https://github.com/Remmyboy/sanctuary-mods/releases/tag/LadderReporter-0.3.0). The catalog approves its ModManager ZIP, which contains the unchanged mod DLL and a README. No local JSON is needed for catalog installation. See the [catalog review](verification/ladder-reporter-catalog.md).

For a local import from either download, select the `SanctuaryMods/LadderReporter` folder. The standalone root also includes BepInEx and Remmy's loader; do not import that root or copy it over Starframe's setup. Its bundled runtime DLLs would fail Starframe's assembly-conflict checks.

Put this `starframe.local.json` beside `LadderReporter.dll`:

```json
{
  "schemaVersion": 1,
  "modId": "com.sanctuarydb.ladderreporter",
  "name": "Ladder Reporter",
  "author": "Remmy",
  "version": "0.3.0",
  "layout": {
    "kind": "starframe_managed_zip",
    "root": "",
    "entryAssembly": "LadderReporter.dll",
    "entryType": "SanctuaryHud.LadderReporterPlugin"
  }
}
```

The existing layout name describes Starframe's managed package storage. It now accepts both internal `IMod` entries and supported BepInEx plugin entries. A local folder does not need to be zipped. For a DLL selected directly, name its metadata `LadderReporter.starframe.json` instead.

1. Close Sanctuary and open a Starframe build that includes the 0.5.0 runtime resources.
2. Select the game installation in Settings and finish setup if required.
3. In My mods, choose **Import local mod**, select `SanctuaryMods/LadderReporter`, and choose **Import copy**.
4. Enable Ladder Reporter in the collection you want to use. Wait for the saved setup to be ready, then launch from Starframe. Keep Steam online for this playtest.
5. Confirm that the desktop reports the active mod and that the game's **Mods** page lists Ladder Reporter as loaded. Open its row to inspect the BepInEx settings.
6. For a dry run, keep **Report / DryRun** on. For a live ladder test, set **Report / Enabled** on, **Report / DryRun** off and **Matchmaking / Enabled** on. Leave **Matchmaking / LocalPort** at `27555`, as expected by Remmy's site, and restart the game before testing.
7. Use the ladder's normal workflow to test joining and reporting a match. A successful Starframe activation report establishes plugin startup; it does not establish that a remote service accepted a match result.

Starframe does not translate Remmy's F8 manager shortcut. Use Starframe's Mods sidebar entry. Configuration remains in `engine/BepInEx/config/com.sanctuarydb.ladderreporter.cfg`. Uninstall removes Starframe's managed copy and retains that file, the downloaded DLL and its local metadata.

## Other BepInEx plugins

Use a reviewed mod folder with its required companion files. Set the metadata's mod ID to the exact `BepInPlugin` GUID and the entry type to the full plugin class name. The current catalog ID format requires a lowercase GUID of at most 128 characters. Starframe does not infer these fields from arbitrary downloaded DLLs.

Declare required mods as exact references in local or catalog metadata so the desktop can enable and order them. The runtime also checks the DLL's `BepInDependency` minimum versions, process filters and incompatibility attributes. Hard dependencies must have started successfully first. A soft dependency that is enabled in the same setup must also appear earlier; an absent soft dependency is permitted. Incorrect order produces a load error rather than silently changing the saved collection. [Local metadata and ordering](verification/local-imports.md)

The supported plugin runs on BepInEx's protected Unity host, registers in `Chainloader.PluginInfos` and uses BepInEx's normal logging and configuration. Disabled plugins are not instantiated. Starframe verifies the selected payloads, prepares the enabled entries, then creates their components in effective collection order. Duplicate registered GUIDs and conflicting assemblies fail visibly. An assembly can contain other plugin types, but only the declared entry is activated.

The settings page displays up to 128 bool, integer, float, text and named-enum entries bound during startup. Plugin constraints still apply. Other types and settings registered later remain accessible through the plugin's own configuration file. Setting changes run on the game thread because plugin handlers may call Unity. BepInEx handles its normal synchronous configuration save. The plugin decides whether a changed value is used immediately; restart to ensure it takes effect.

## Limits

This is a verified path for BepInEx 5 Unity/Mono component plugins. It does not restart BepInEx's chainloader or scan arbitrary folders. BepInEx 6, preloader patchers, plugins that require an earlier loading phase, and plugins relying on a file-backed `Assembly.Location` are outside this path. Verified DLLs are loaded from bytes; `PluginInfo.Location` identifies the managed entry file, but `Assembly.Location` is empty.

Startup reports cover component creation and unhandled `Awake`/`OnEnable` failures. They cannot prove later `Start`/`Update` behavior, gameplay, networking, or success inside an exception handler written by the mod author. Dependency order constrains initialization; it cannot prescribe every later Harmony patch or event handler.

Rebuilds follow [the local development workflow](local-development.md): keep the current game files during play and apply saved changes after exit. No DLL hot reload is provided. Existing external loaders and plugins remain outside Starframe's ownership; do not install a second copy of the same plugin through another loader. Maps and AI content remain unsupported by this change.
