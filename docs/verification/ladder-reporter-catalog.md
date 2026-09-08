# Ladder Reporter catalog approval

The owner approved Remmy's Ladder Reporter 0.3.0 as Starframe's first catalog mod on 8 September 2026, after reporting a successful game played with Remmy through Starframe. The owner previously reported Remmy's permission for catalog inclusion. Starframe links to the author's release assets and does not redistribute the binaries in this repository.

Catalog revision 2 approves release ID `com.sanctuarydb.ladderreporter.0.3.0`, under the plugin GUID `com.sanctuarydb.ladderreporter`. Publication follows the merge of PR #56. This approval does not publish an installer or complete the public alpha distribution gates.

## Exact artifact

Source: [Ladder Reporter 0.3.0](https://github.com/Remmyboy/sanctuary-mods/releases/tag/LadderReporter-0.3.0).

| Artifact | Size | SHA-256 |
| --- | --- | --- |
| Approved `LadderReporter-0.3.0-ModManager.zip` | 41,596 bytes | `b04adaf8a80ad86966fb4809188f3827f356799af4b089cee80a4174a6a115f2` |
| Reviewed `LadderReporter-0.3.0-Standalone.zip` | 699,019 bytes | `7e5194ae337de9b6bfdb78bf1768f752870add9a0dd07cde7b5880fc69d8b206` |
| `SanctuaryMods/LadderReporter/LadderReporter.dll`, identical in both ZIPs and the tested local copy | 94,208 bytes | `9f1a662d83fe1cb49b5d1f5c00a0d847e05b4fd196f5aca093e06e11449186b9` |

Both ZIPs were downloaded from the author's GitHub release. Their bytes match the release API's size and digest metadata; the standalone digest also matches the owner's supplied value.

The approved ModManager ZIP contains the DLL and `README.txt`. Starframe supplies BepInEx itself and activates `SanctuaryHud.LadderReporterPlugin`. No companion mod dependency or local import JSON is required. The archive is retained without repackaging.

The standalone ZIP is unsuitable as a complete managed package: it also includes BepInEx, Harmony, Remmy's `ModLoader.dll`, Doorstop configuration and `winhttp.dll`. Starframe inventories all DLLs and rejects identities that conflict with the game/runtime. A layout `root` identifies the entry location; it does not filter these files. The earlier local test imported only the LadderReporter subfolder, so it did not encounter those bundled assemblies. Selecting the author's mod-only asset avoids the conflict without weakening verification.

## Verification

- The catalog validator accepts revision 2 against `main`'s revision 1: one mod and one release.
- An isolated native Starframe desktop read the proposed catalog, downloaded the approved ZIP through its normal package operation, verified it and added a catalog-origin library record. The extracted DLL hash matches the tested local copy. This check did not write game files or launch Sanctuary.
- The local import had already loaded in Sanctuary Playtest build `25135612`, game `0.0.1.15`, Unity `6000.3.22`, with BepInEx `5.4.23.5`. Starframe's process-bound report confirmed Ladder Reporter loaded. The owner then reported that the mod worked while playing with Remmy. This is owner-reported gameplay evidence, not an independent audit of SanctuaryDB's stored results.

The checks retain downloads, package-operation output and a catalog screenshot under ignored `test-results/0.5.0-bepinex/catalog`. No third-party binaries or private game logs are committed.

This release needs Starframe 0.5.0's BepInEx compatibility from #57. The catalog description states that requirement; the current catalog schema has no minimum-Starframe-version gate, so older clients can still see the entry and cannot activate its BepInEx class. Merge the catalog with its runtime changes. See [BepInEx support and configuration](../bepinex-mods.md) for supported lifecycle behavior and limits.
