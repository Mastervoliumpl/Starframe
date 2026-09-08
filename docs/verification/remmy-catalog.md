# Remmy catalog additions

The owner requested these six releases on 8 September 2026 and previously reported Remmy's permission for catalog inclusion. Catalog revision 3 adds them alongside the existing Ladder Reporter release. Downloads remain at the [author's GitHub releases](https://github.com/Remmyboy/sanctuary-mods/releases); no binaries are committed or repackaged. Publication follows merge to `main`. This catalog change has no issue, milestone or app-version bump.

## Artifact review

Each downloaded ModManager ZIP matches both the owner's SHA-256 and the GitHub release API's digest and size. The exact URLs, hashes, sizes and entry layouts are in [the catalog](../../catalog/releases.json).

| Archive | Bytes | SHA-256 |
| --- | --- | --- |
| SanctuaryHud-0.8.0-ModManager.zip | 1,349,764 | `9097bdfffe3b6750934ecec08e4703acdef6d65ed7210c7da1e68f145f2aaf59` |
| BuildHotkeys-0.1.1-ModManager.zip | 37,674 | `eaa9d75f87623ab85a998f18beb342bcee0102125ba3691aa6f5a1985cca320a` |
| CameraUtilities-0.1.2-ModManager.zip | 28,459 | `2859c8c164312de20e12379d7c3a3472b95b621a8fe1e7c1617cb28b1d06fee6` |
| EcoManager-0.3.0-ModManager.zip | 24,482 | `350f976ba60a6454dd32a2d72c3a29ecb5a5b737e7009146abc31cea23682d7b` |
| ReplayManager-0.2.0-ModManager.zip | 30,160 | `0c7d0ab6732f3bfd7e3035ee0d00ba1d17a18972b41df1656b9087fd4881d410` |
| IdleEngineers-0.1.0-ModManager.zip | 21,343 | `3c71e1ccdcc1aad207826df7d9102c1492f630f991804dc0c8b5de2c52a7a094` |

The DLLs were inspected with ILSpy without executing the plugins. Each declared entry derives from BepInEx 5 `BaseUnityPlugin`. Catalog IDs and versions match `BepInPlugin`; no `BepInDependency`, `BepInProcess` or `BepInIncompatibility` attributes were found. Each archive contains one DLL with a distinct assembly name and no bundled loader or runtime. No companion mod requirement is declared.

| Mod | Plugin GUID | Entry type | Files in ZIP |
| --- | --- | --- | --- |
| Sanctuary HUD 0.8.0 | `com.sanctuarydb.hud` | `SanctuaryHud.SanctuaryHudPlugin` | 15 |
| Build Hotkeys 0.1.1 | `com.sanctuarydb.buildhotkeys` | `SanctuaryHud.BuildHotkeysPlugin` | 2 |
| Camera Utilities 0.1.2 | `com.sanctuarydb.camerautilities` | `SanctuaryHud.CameraUtils.CameraUtilitiesPlugin` | 2 |
| Eco Manager 0.3.0 | `com.sanctuarydb.ecomanager` | `SanctuaryHud.EcoManagerPlugin` | 2 |
| Replay Manager 0.2.0 | `com.sanctuarydb.replaymanager` | `SanctuaryHud.ReplaysPlugin` | 2 |
| Idle Engineers 0.1.0 | `com.sanctuarydb.idleengineers` | `SanctuaryHud.IdleEngineersPlugin` | 2 |

Entry DLLs reside at `SanctuaryMods/<mod>/<mod>.dll`. All archives include a root `README.txt`. Sanctuary HUD also includes a sounds README and twelve WAV files across three voice packs. Starframe retains all these files; layout `root` locates the entry DLL and does not filter the archive. Authors do not need a local-import JSON for these catalog entries.

## Known limitation: Sanctuary HUD voice packs

Sanctuary HUD 0.8.0's `Alerts.SoundsDir` uses `Path.Combine(Paths.GameRootPath, "SanctuaryMods", "SanctuaryHud", "sounds")`. Starframe deploys the archive under its private managed mod root and does not overlay companion files into that fixed game folder. A clean Starframe installation therefore does not expose the bundled voice packs at the path this release searches.

The inspected code returns no available packs when that folder is absent and generates tones when a voice file cannot be resolved. Alert sound is off by default. This establishes the fallback in code, not an in-game audio test. Existing files installed outside Starframe could change what the mod discovers. The catalog description discloses the limitation. This PR does not patch the author's DLL or add companion-file deployment behavior.

## Validation and limits

The catalog validator checks revision 3 against revision 2 on `main`, retaining the existing Ladder Reporter identity. Repository checks and catalog formatting also pass. Local downloads, extracted files and inspection output remain under ignored `test-results/remmy-catalog`.

These six releases have not been launched or tested in a match through Starframe. Their `testedGameBuilds` arrays are empty. Descriptions summarize the author's release notes; archive and metadata checks do not establish gameplay, Harmony patch compatibility or compatibility when all seven mods are enabled together. The earlier Ladder Reporter test does not supply evidence for these releases. Use a Starframe 0.5.0 build with runtime resources for subsequent game tests and record an observed game build only after those checks pass.
