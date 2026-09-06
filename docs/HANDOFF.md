# Starframe 0.0.1 implementation handoff

Status: initial 0.0.1 handoff amended for 0.0.2 on 6 September 2026. The logo and first-version desktop layout remain accepted. DESIGN.md revision 0.7 supersedes the earlier in-game screen and specifies the revised launch action. This document defines the first implementation contract; it does not claim a working game integration or start the next implementation milestone.

Read [DESIGN.md](../DESIGN.md), [ARCHITECTURE.md](../ARCHITECTURE.md), [DEVELOPMENT.md](../DEVELOPMENT.md) and the [version roadmap](../ROADMAP.md). The [design specimen](design/review.html) demonstrates the UI with fictional data. [Design review evidence](design/REVIEW.md) distinguishes browser checks from later native verification.

## Initial support and performance target

Target **Windows 11 x64**, starting with version **25H2**, and test supported later Windows 11 releases before listing them as supported. Use the current Evergreen WebView2 runtime through Tauri. Windows 10, ARM64, Linux and macOS are outside the initial support promise. This is a product support choice, not a claim that Tauri cannot run elsewhere. Microsoft lists Windows 11 25H2 within support through October 2027 for Home/Pro; recheck the supported Windows matrix before shipping. [Microsoft lifecycle](https://learn.microsoft.com/en-us/lifecycle/products/windows-11-home-and-pro), [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

Use a modest test machine: four CPU cores, 8 GB RAM, SSD storage, integrated graphics capable of driving a 1280 × 800 display at 60 Hz, and a supported Windows/WebView2 installation. Record the actual CPU, GPU, memory, display scale, OS and WebView2 versions with results. These are manager test targets, not Sanctuary's minimum game requirements. Test the game itself on hardware that meets the game's requirements.

| Check | Initial acceptance target |
| --- | --- |
| Input feedback | Pressed, selected, navigation and pending feedback visible within 100 ms at the 95th percentile over 100 representative interactions. |
| Background load | Exercise 1,000 fixture library rows with three simultaneous bounded downloads, plus active hashing/extraction. Record artifact sizes and transfer rates. |
| Motion | Aim for 60 fps; input and state accuracy take precedence. Report missed-frame behavior under the same load. |
| Startup | Show the shell and cached local state before any network response; network failure must not delay usability. Measure startup rather than inventing a fixed guarantee now. |
| Layout | Review 1280 × 800 and 1024 × 720, 100/150/200% Windows display scale, 200% text size, high contrast and reduced motion. |
| Long work | Keep navigation, search and unrelated edits usable; report progress, cancellation limits and errors. Never show success before the relevant commit/file verification. |

The browser specimen's compact-width checks establish reflow behavior only. They do not establish these performance targets, real Windows scaling, screen-reader behavior, or in-game input capture.

## Desktop interface

Use commands that express intent. The Rust module remains the authority for saved state and file operations; the frontend holds transient drafts, search and selection.

| Request | Inputs | Result and invariant |
| --- | --- | --- |
| Subscribe to state | Frontend channel | Register and capture the first snapshot under the same short synchronization step. |
| Edit collection | Request ID, collection ID, expected revision, new name/ordered references | Return the committed revision or a typed stale-edit/validation error. Automatic deployment is a separately observed operation. |
| Prepare mod | Request ID, approved release/artifact ID or explicit local selection | Return an operation ID immediately; source resolution and path validation stay in Rust. |
| Launch game | Request ID, installation ID | Recheck the latest desired revision and readiness. An accepted request is not evidence of game start. |
| Cancel operation | Operation ID | Cancel safely or report that completion/rollback must finish first. |

A state snapshot contains `sessionId`, `revision`, compact library/collection records, active collection ID, desired/applied revisions, operation summaries, game observations and app/catalog check status. Represent revisions as decimal strings across the JSON interface so JavaScript integer limits cannot change their value. IDs are opaque stable strings, never paths or display labels. A new Rust process starts a new session; the UI must reject old-session results.

Errors use a stable code, readable message, affected IDs and recoverability information. Start with the codes actually needed: `stale_revision`, `missing_dependency`, `unsupported_format`, `game_running`, `file_conflict`, `integrity_failure`, `network_failure` and `repair_required`. Game-version warnings belong in compatibility evidence and do not become a launch-blocking error code.

## Game-integration interface

Keep a single narrow seam between a supported ordered setup and the mechanism that activates it. Implement only the current BepInEx/Starframe adapter. The future native adapter has no guessed paths, endpoints or SDK methods in this contract.

| Operation | Responsibility |
| --- | --- |
| Inspect | Report runtime identity, supported contract versions, game observations and capabilities. |
| Prepare activation | Convert a validated ordered setup into an activation document and an owned-file deployment plan. Do not apply files here. |
| Read activation report | Correlate the report with the observed game session and deployment revision. Report unknown/stale evidence explicitly. |

Capabilities are values, not independent plugin frameworks: `managedLifecycle`, `manualPriority`, `contentOverlays`, `settingsUi` and `activationReport`. Each reports supported/unsupported with a reason where needed. Capabilities can also differ by package. The desktop cannot infer arbitrary ordering support merely because a file is a DLL.

The current adapter supports ordering only for entries activated through the verified Starframe lifecycle. Conventional BepInEx plugins have their own dependency/lifecycle behavior. Support them only through a tested compatibility path and show its limits. Do not manually instantiate arbitrary plugin classes and assume that their expected BepInEx context exists. [BepInEx plugin model](https://docs.bepinex.dev/articles/dev_guide/plugin_tutorial/2_plugin_start.html)

## Activation document, version 1

The desktop writes this document as part of a verified deployment while the game is closed. The runtime reads it at game startup. It is a private local handoff, not a downloadable collection, and contains no network locations or executable installation instructions.

| Field | Meaning |
| --- | --- |
| `schemaVersion` | Integer wire-format version, initially 1. Reject unsupported versions. |
| `deploymentRevision` | Decimal string identifying the confirmed setup. |
| `runtimeContractVersion` | Integer lifecycle/settings contract, initially 1. Independent of app SemVer. |
| `integrationId` | Stable adapter identifier, initially `starframe.bepinex`. |
| `installedMods` | Display-only inventory of installed mods at preparation time: stable `modId`, `name` and display `version`. No executable paths. |
| `mods` | Ordered entries. Array order is the effective activation order. |
| `mods[].modId` | Unique stable mod identity. |
| `mods[].source` | Approved release reference or local content reference; neither can instruct a download. |
| `mods[].root` | Validated relative directory under the owned active-mod root. |
| `mods[].entryAssembly` / `entryType` | Relative managed assembly path and type implementing the supported lifecycle. |
| `mods[].requires` | Required stable mod IDs in this prepared setup. Each must appear earlier and activate successfully. |
| `mods[].files` | Relative immutable payload paths and SHA-256 values for verification. Mutable settings/logs are excluded. |

Illustrative shape only; the zero hash is a placeholder, not an approved artifact:

```json
{
  "schemaVersion": 1,
  "deploymentRevision": "12",
  "runtimeContractVersion": 1,
  "integrationId": "starframe.bepinex",
  "installedMods": [
    {"modId": "example.core", "name": "Example Core", "version": "1.0.0"}
  ],
  "mods": [
    {
      "modId": "example.core",
      "source": {"kind": "catalog", "releaseId": "example.core.1"},
      "root": "example.core/1",
      "entryAssembly": "Example.Core.dll",
      "entryType": "Example.Core.Entry",
      "requires": [],
      "files": [
        {
          "path": "Example.Core.dll",
          "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        }
      ]
    }
  ]
}
```

Validate the whole document before invoking a mod. Reject duplicate IDs, unknown contract versions, missing required entries, invalid order, unsupported kinds and invalid entry declarations. Bound document size, entry count and string lengths in the shared schema when implemented. Resolve and validate paths against the intended root, including reparse points and Windows path aliases; never trust relative-path spelling alone. Every entry assembly and supporting managed payload file must be covered by the validated inventory. Do not let a name resolve a DLL from an uncontrolled external directory.

The deployment module validates files before launch; the runtime validates its handoff and payload integrity before activating entries. Keep this off the game's render loop where the integration permits, and report preparation while hashing. Dependency assemblies required by a listed mod may load, but an unlisted mod entry point must not execute.

The in-game Mods menu uses `installedMods` for its list and derives enabled membership from `mods`; actual loaded/failed state comes from the current runtime session. Require unique inventory IDs and a matching inventory entry for every activation entry. Bound and validate display metadata without interpreting it as markup. A disabled inventory entry never authorizes loading or requires inspecting its DLL. The list is a snapshot of the prepared setup, not live desktop intent; this lets it work after the desktop closes without another process or a database connection.

`source.kind=local` carries an opaque local content ID in place of a release ID. It is not an absolute developer path. Collection matching must compare the imported payload inventory/content identity, not the display version or filename. Define canonical inventory fixtures in issue #11 before treating local content IDs from different machines as interchangeable. The runtime does not need to reproduce the desktop's library database keys.

## Activation report and lifecycle

The runtime writes a bounded report with `schemaVersion`, `deploymentRevision`, `runtimeContractVersion`, a generated `gameSessionId`, observed process identity/start information, and ordered per-mod outcomes. Initial outcomes are `loaded`, `failed`, and `skipped_dependency`; failure data includes a readable message and stable error code. The desktop checks that report against the actual observed game process and requested deployment. A matching revision alone is insufficient because two launches can use the same setup.

Write reports through a temporary file and replacement so readers do not parse a partial update. A missing, stale or unreadable report means unknown activation status, not a successful load. Do not continuously rewrite the report when nothing changes.

Keep the author-facing lifecycle small:

1. Verify entry metadata and resolve declared dependency assemblies.
2. Create the entry through the supported contract and call `Initialize(context)` once in effective order.
3. Supply scoped logging, the mod's config access and settings registration through the context. Avoid a general desktop command or arbitrary file-write interface.
4. Record success or failure. If initialization fails, skip required dependents. Explain that the failed mod may already have made partial changes.
5. If provided, call a best-effort game-exit callback during normal shutdown. It is not a guarantee during process termination and does not promise DLL hot unload.

Mod code still runs inside the game with that process's access. The context is a convenience interface, not a sandbox. No hot reload, live collection switching, or universal cleanup of third-party patches is promised by version 1.

## Settings registration, version 1

Present the list and settings through Sanctuary's existing menu conventions. Add a `Mods` main-menu entry with the monochrome Starframe mark, matching the other icons' tint and states. Reuse game controls and input navigation where possible; verify actual menu hooks, focus, back/close behavior and scaling in issue #14. The old browser in-game panel is not an implementation reference. Settings unavailable for a disabled mod explain the enable/restart requirement without loading that mod to discover its UI.

Keep per-mod configurations outside immutable payload folders so deployments and collection changes cannot overwrite them. Use BepInEx `ConfigFile` and typed entries where possible, including a separate ConfigFile for each managed mod. The library supports custom configuration-file paths and typed binding, so Starframe needs its own presentation and registration contract rather than a new settings persistence engine. [BepInEx configuration](https://docs.bepinex.dev/articles/dev_guide/plugin_tutorial/4_configuration.html)

| Field | Meaning |
| --- | --- |
| `key` | Stable key scoped to the mod; unaffected by translated/display labels. |
| `section`, `label`, `description` | Display grouping and help text; treat as text, not markup. |
| `kind` | `boolean`, `integer`, `number`, `choice` or `text` initially. |
| `defaultValue`, `value` | Values of the declared kind. Default resets only this setting after an explicit action. |
| `constraints` | Bounds/step for numbers, stable options for choices, maximum length for text. Reject nonfinite numbers. |
| `applyMode` | `live` only when supported by the mod; otherwise `restart`. |
| `order` | Display order within the section; independent of mod load order. |

Keyboard shortcuts can use a validated text/choice representation first. Add a dedicated binding widget only with a supported game input contract. Do not invent a generic UI schema for every possible mod. A mod may expose no settings.

The runtime is the only settings writer during play. Validate values before saving. A restart setting stores the desired value while the running mod keeps its effective value until restart; the UI must explain that difference. Configuration values never enter a shared collection. Desktop editing during play would require a single authoritative runtime writer and is excluded from version 1.

## Storage proof and dependency choices

The 0.1.1 decision selects bundled SQLite through rusqlite, with one desktop database owner on the existing background worker. It supersedes the original Turso preference. The 0.1.1 implementation uses bundled SQLite and retains original Turso files during conversion. See [current recovery instructions](verification/sqlite.md). See the [decision and conversion plan](planning/sqlite-transition.md).

Issue #9 proved the original Turso implementation on Windows; retain that evidence. SQLite must pass required transactions/constraints, restart persistence, forced termination, backup/restore, successful/failed migrations, unsupported future schema and offline checks. Add retained-source conversion fixtures for schemas 1 through 3, WAL-bearing data and completed backups. Failure must retain data and expose recovery, never reset the library or silently ship both engines.

Reuse choices for implementation:

- Tauri commands/channels, native pickers, existing async runtime and maintained plugins for desktop integration.
- Svelte reactivity and CSS transitions for presentation; no extra global state framework or animation engine.
- Serde/serde_json for Rust data; an established type generator only if it removes actual cross-language drift.
- reqwest for HTTP, maintained ZIP/SHA-256/version libraries for their existing formats, and notify with bounded reconciliation for local sources.
- Standard collections and a stable topological traversal for order unless an already-selected graph library fits without extra machinery. No general package solver.
- BepInEx bootstrap/log/config and verified Unity facilities for the C# runtime. No custom injection layer.
- Tauri NSIS and the signed updater for distribution. Game cleanup reuses deployment ownership/recovery.

The exact dependency versions must be pinned and checked together when each source project is introduced. Planning does not install a dependency set that has no code to use it.

## Content overlays and SDK licensing

The overlay investigation must identify the game's actual Lua/data lookup paths, when resources are read, and whether a supported interception point can serve mod content without rewriting original game assets. Define collision precedence only for a path whose behavior has been verified. The proposed rule is later effective order wins; list the conflicting mods and resulting winner. Keep unsupported content types explicitly unsupported until that evidence exists. This is implementation research assigned to issues #13 and #20, not a missing promise to implement an imagined API during planning.

The repository's AGPL license remains unchanged. The public mod API raises a separate question: what terms should apply to an SDK that third-party mods reference, and what notices/permissions are needed when distributing the runtime with BepInEx and game references? Document the exact linking and redistribution arrangement before publishing that SDK. No linking exception, alternative license, or blanket statement about every mod's licensing is adopted by this handoff. Keep public SDK publication blocked on that review in issue #11; internal fixture interfaces can be tested without presenting them as a stable released SDK.

## Handoff exit record

| Item | Evidence / state |
| --- | --- |
| Accepted product direction | DESIGN.md, ARCHITECTURE.md and the closed baseline issue #1. |
| Continuous checks and scope | DEVELOPMENT.md and the closed setup issue #2. |
| Identity and visual specimen | Original SVG marks and design/review.html; user approved the handoff on 6 September 2026. |
| Browser interaction checks | design/REVIEW.md records the exercised flows and limits. |
| Windows and responsiveness targets | Defined above; actual app measurement belongs to implementation. |
| Contract and reuse decisions | Defined above; cross-language fixtures and runtime verification belong to #11–#15. |
| Next milestone | 0.1.0 remains planned. Finishing this handoff does not start application implementation. |

VERSION identifies this handoff as 0.0.1, with its completed work recorded in CHANGELOG.md. Close the GitHub milestone after the approved handoff merges and required checks pass. No application implementation has started. A documentation milestone does not require an installer or public app release.
