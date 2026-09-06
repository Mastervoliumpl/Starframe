# Starframe: design direction

Status: revision 0.7, 6 September 2026. Product name: Starframe. The user retained the logo and accepted the desktop layout for the first version. This revision supersedes the earlier in-game presentation and plans the revised launch action.

This is the accepted design handoff. The user authorized milestone 0.1.0 on 6 September 2026, including desktop navigation and live state in issue #8. Later feature screens remain planned. The user accepted the visual direction, including the fonts, added neutral shades, component treatments, and layout. Measurements and motion timings are starting targets to validate in representative visual screens. Open product decisions remain identified below.

## 1. Confirmed direction

Build Starframe, an installable desktop mod manager for Sanctuary: Shattered Sun, Windows first. Users can download approved releases, manage installed mods, create and share collections, and launch the game. The owner curates individual releases. Downloads come from authors' locations; the app does not host mod binaries.

Local imports are managed mods too. Developers must be able to load and manage their own local builds. Label their origin clearly as `Local import`; provide the same management controls without online catalog version checks. Watch imported sources and prepare updated copies as described in [ARCHITECTURE.md](ARCHITECTURE.md).

Starframe owns its in-game mod runtime and settings UI, initially using BepInEx for bootstrap. Keep the desktop manager usable above the game's planned native mod support when that becomes available. Its interface is not yet published; supported controls must follow the active integration's capabilities.

The desktop visual direction is dark navy with orange accents. It should feel modern and related to Sanctuary, with its own identity. The in-game Mods menu follows Sanctuary's existing interface. Game content and artwork are allowed where appropriate; the earlier restriction is withdrawn. Keep Starframe's original logo. Motion and rounded corners must have a purpose.

Responsiveness is the leading interaction requirement. The interface must respond while downloads, file operations, compatibility checks, and other background work take time. Data changes appear live without requiring a refresh. Unavailable actions explain their restriction. Visual effects must yield to input responsiveness, while file safety and accurate status remain required.

The official repository is [Mastervoliumpl/Starframe](https://github.com/Mastervoliumpl/Starframe), which contains the [GNU AGPL v3 license](LICENSE). Distribute the app through that repository's GitHub Releases page. Check for a newer app release on launch and periodically while the app runs. Notify the user and let them choose when to update. This is separate from catalog checks, mod updates, and game updates.

The user supplied these six colors:

| Color | Value | Intended role |
| --- | --- | --- |
| Background | `#0F172A` | Main canvas |
| Contrast orange | `#F97316` | Primary action and selected emphasis |
| Pale orange | `#FB923C` | Accent hover and selected text on raised surfaces |
| Gray | `#334155` | Selected surfaces and quiet dividers |
| Light gray | `#CBD5E1` | Body text and secondary labels |
| White | `#F8FAFC` | Titles and strongest text |

Game compatibility and mod releases are separate concepts. Warn when a catalog mod was made for an older game version, while still allowing the user to enable it and try launching. Collections contain a name and an ordered list of mod references. Sharing preserves those releases and their order, without mod settings. Refresh catalog data independently on launch and every five minutes while the desktop app is open; this does not require an app update.

## 2. Visual character

Reading this as a desktop mod-management tool for Sanctuary players and mod developers: precise, game-informed, dark navy, with concentrated orange emphasis. Dials: ENERGY 2 / RHYTHM 2 / MOTION 2.

Use broad, flat regions with aligned edges and readable lists. The contrast between the dark work area and the orange action gives the interface its focal point. Density should support managing many mods without making the app feel like a spreadsheet.

Game influence belongs in the launch artwork, collection artwork, a restrained heading face, and the composition of major headers. The game's large engineered structures suggest strong alignment and deliberate divisions. This is a design interpretation of the official visual reference, not a claim that the game uses this palette.

The current desktop layout is an accepted first-version baseline. Improve its character through the launch artwork, careful spacing and typography, distinct interaction states, and motion that explains changes. Animation alone does not resolve a generic layout. Keep later visual refinements within these requirements and review them using concrete screens.

Identity treatment: collection detail headers may have one 8-unit cut at the trailing corner of their artwork frame. It separates collection identity from ordinary controls. Keep inputs, rows, and action buttons rectangular with the radii below. Validate this motif in the first visual review; it is not a logo decision.

Keep decorative effects out of the working list. No background grids, orbiting elements, idle glows, particle effects, or translucent layers behind text. Shadows identify temporary elevation, such as a menu or dialog. A mod with no artwork gets a plain name or initial treatment, not invented game art.

## 3. Reference interpretation

| Reference | What to study | Application here |
| --- | --- | --- |
| [Beautiful UI](https://www.beautifului.dev/) | Records Table, Filter Table, Sidebar Nav, Search, Task Rows | Aligned information, compact controls, clear selection, expandable detail, and useful task status |
| [Transitions.dev](https://transitions.dev/) | Menu dropdown, panel reveal, modal open/close, accordion, tabs sliding, spinner-to-check | Motion that preserves location or communicates a state change |
| [Sanctuary official site](https://www.sanctuaryshatteredsun.com/) | Dark framing, engineered forms, strong wordmark, large-scale game imagery | Restrained game identity in headers and collection artwork |
| [Skyve](https://github.com/JadHajjar/Skyve) | Mod details, playsets, compatibility information, pre-launch management | Useful information hierarchy for a desktop manager |

These are references, not component or code commitments. A reference's marketing page, AI-specific controls, blue accents, or pill shapes do not transfer automatically. Check licensing before reusing code, fonts, or artwork. Prefer existing platform controls where they can express the approved design.

## 4. Color tokens and contrast

Preserve the six supplied colors. Two additional neutral shades are part of the accepted direction: `#1E293B` for raised surfaces and `#94A3B8` for low-emphasis text and functional control outlines. These add depth and readable secondary states without introducing another accent hue.

| Token | Value | Use |
| --- | --- | --- |
| canvas | `#0F172A` | Main workspace and ordinary rows |
| surface | `#1E293B` | Sidebar, menus, dialogs, hover fill |
| surface-selected | `#334155` | Selected row or navigation destination |
| divider | `#334155` | Nonessential section and row separators |
| control-outline | `#94A3B8` | Input, checkbox, and secondary-button boundaries |
| text-primary | `#F8FAFC` | Headings, names, key values |
| text-body | `#CBD5E1` | Descriptions, labels, messages |
| text-muted | `#94A3B8` | Metadata on canvas or surface only |
| accent | `#F97316` | Primary action, focus, progress |
| accent-hover | `#FB923C` | Hovered primary action; accent text on selected surface |
| text-on-accent | `#0F172A` | Labels and icons on orange fills |

Measured with the installed Anti-slop contrast checker, using opaque sRGB colors:

| Foreground / background | Ratio | Design rule |
| --- | --- | --- |
| White / canvas | 17.06:1 | Main text allowed |
| Light gray / canvas | 12.02:1 | Body text allowed |
| Orange / canvas | 6.37:1 | Accent text allowed |
| Pale orange / canvas | 7.89:1 | Accent text allowed |
| Navy / orange | 6.37:1 | Primary button label |
| White / orange | 2.68:1 | Do not use for button labels |
| Gray / canvas | 1.72:1 | Decorative divider only; insufficient as the only control boundary |
| Light gray / gray | 6.97:1 | Text in selected rows |
| Orange / gray | 3.69:1 | Insufficient for normal-size text |
| Pale orange / gray | 4.58:1 | Selected accent text allowed at full opacity |
| Muted / surface | 5.71:1 | Secondary metadata allowed |
| Muted / gray | 4.04:1 | Use light gray instead in selected rows |

Require 4.5:1 for all ordinary UI text and 3:1 for visual information needed to identify controls and states. Decorative dividers can be quieter. This avoids relying on the large-text exception. Verify the final rendered pairs, including hover, selection, overlays, and any opacity. [Contrast guidance](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)

Orange is for action and selection, not a general claim of safety or success. Keep ready, local, unknown, and failed states distinguishable through labels and icons. For this draft, use white/light-gray text with a check, local-file, question, or error symbol respectively. A critical message remains persistent with an explicit action. A red error token may be considered during visual review; it is not silently added to the palette.

Use one dominant orange-filled action in each active context. While a dialog is open, its action becomes the focus. Ordinary row actions remain neutral. No orange fill across the entire selected row.

## 5. Typography, spacing, and shape

All sizes below are logical design units at 100% scale, equivalent to Windows device-independent pixels. They must scale with display and text settings.

Body face: Segoe UI Variable, with Segoe UI and system sans-serif fallbacks. The reason is readable small text and familiar Windows controls. Display face: Bahnschrift for page and collection headings only, with the body face as fallback. Its compact, engineered letterforms provide game character without turning mod names into display text. Use installed system faces; do not bundle Windows font files.

| Text role | Size / line height | Weight |
| --- | --- | --- |
| Page or collection title | 28 / 34 | Semibold |
| Section heading | 18 / 24 | Semibold |
| Mod name and button | 14 / 20 | Semibold |
| Body and navigation | 14 / 20 | Regular |
| Metadata | 12 / 18 | Regular |
| Paths and diagnostic values | 12 / 18 | System monospace |

Use sentence case. Reserve monospace for paths, versions when alignment matters, and logs. Use tabular numerals for download progress. Do not add wide letter spacing to navigation or labels. Logo and wordmark typography remain open.

Spacing scale: 4, 8, 12, 16, 24, 32, 48. Use 8 between related icon/label pairs, 12 within control groups, 16 within sections, 24 for page padding, and 32 between distinct sections. Alignment has priority over filling empty space.

| Element | Radius | Reason |
| --- | --- | --- |
| App regions, tables, ordinary list rows | 0 | Continuous workspace and stable scan lines |
| Checkboxes and small status labels | 3 | Small controls retain a defined shape |
| Buttons, inputs, selected navigation fill | 6 | Clearly bounded interaction surfaces |
| Menus, collection tiles | 8 | Grouped content and temporary surfaces |
| Dialogs | 12 | A distinct foreground layer |
| Switch track and thumb | Fully rounded | Shape communicates sliding on/off state |

No nesting of rounded containers around every subsection. Let the operating system own the outer window corners and caption buttons. Use a 1-unit border by default; a 2-unit focus ring with 2 units of dark separation must remain visible on orange controls too.

Standard control height: 36. Primary launch action: 40. Icon-only hit area: at least 32 square, with a visible tooltip and accessible name. Content rows: minimum 56, growing for wrapped names or larger text. Avoid fixed text clipping.

## 6. Window composition

Design first at 1280 × 800. Also review 1024 × 720 and 1600 × 1000. Resizing, Windows snapping, and display scaling are normal use cases. Compact behavior is a desktop requirement; mobile support is not in scope.

```text
┌────────────────── system title bar / window controls ─────────────────┐
│                  │ Page title                   Search / view actions│
│ Product identity ├───────────────────────────────────────────────────┤
│                  │                                                   │
│ My mods          │ Main list, catalog, or collection workspace        │
│ Catalog          │                                  ┌───────────────┤
│ Collections      │                                  │ Mod details   │
│ Downloads        │                                  │ when opened   │
│                  │                                  │               │
│ Settings         ├──────────────────────────────────┴───────────────┤
│ Help & logs      │ Active collection · readiness         Launch      │
└──────────────────┴───────────────────────────────────────────────────┘
```

This diagram records the accepted navigation direction and starting proportions. It is not a finished UI; product-specific terminology remains open.

Sidebar width: 208. Keep labels visible in the standard window. The main header starts at 72 high; controls can wrap. The launch area starts at 64 high and spans the workspace. Both grow with content. The main list owns its scrolling region.

At workspace widths that support it, mod details use a 360-unit side panel. Below 1280 window width, open details as a full workspace view with Back, preserving the list's search, selection, and scroll position. Under 1100, reduce the sidebar to 176 and move optional columns into detail. Under 900 effective units, use a collapsible navigation drawer. At high text scale, stack filters and launch controls and allow vertical scrolling instead of squeezing them into a fixed bar.

Landing view: My mods. The visible active collection selector explains which setup the switches affect. Collections remain a separate destination for creating and sharing setups. Avoid a separate dashboard unless it answers a user need that these views cannot.

## 7. Screen and component rules

### My mods

Use a list by default. A row contains a selection checkbox, mod name and author, version, origin, compatibility text, and an enabled switch. Selection chooses rows for bulk actions; the switch changes whether a mod is included in the active setup. These are separate controls with separate accessible labels.

Show `Catalog release` or `Local import` as plain provenance text. Catalog compatibility uses labels such as `Compatible with [game version]`, `Not checked for this version`, or `Made for a previous game version`. Show the declared and installed game versions in details. An older-version warning must not disable Enable or launch or repeatedly demand confirmation. Missing required dependencies and a missing runtime are separate issues. Local imports skip catalog compatibility-version checks. Approval and compatibility are separate labels. Never display `Malware-free` or imply that curation guarantees safety.

Keep frequent actions visible when relevant. Put uninstall and infrequent actions in a named row menu. Clicking the mod name opens its detail view. Hover changes only the row fill; selection uses the selected surface, a checked selector, and readable text. Do not make information appear only on hover.

A bulk action bar occupies a reserved area below the filters when rows are selected. Its labels name the outcome, such as `Enable selected` or `Remove from collection`. Distinguish removal from a collection from uninstalling a mod.

### Catalog and mod details

Use compact list entries with optional author-supplied artwork. A large thumbnail grid is an alternative to evaluate, not the default for a small curated catalog. Lead with name, purpose, approved release, and compatibility. Install is a neutral row action; it becomes the primary action in the detail view.

Details include description, author/source link, approved versions, dependencies, compatibility notes, and installation state. Put technical paths and diagnostics behind expandable sections. Download progress belongs beside the action it replaces and in Downloads. Keep labels stable in width as progress changes.

Fetch catalog changes on startup and every five minutes while the app is open. A valid new catalog updates the list in place, preserving filters, focus and scroll position. Reuse cached data while offline and show when it was last checked. New approval metadata must not install a mod or update an installed release by itself.

### Collections

Tiles are appropriate here because each collection is an identifiable setup. Use an 8-unit radius, optional artwork, its name, and a concise mod summary. One active collection gets an explicit `Active` label and a check, not a glowing border. Use two or three columns where they fit; fall back to a list at narrow widths.

Opening a collection shows its contents and `Use collection`, `Edit`, and `Share` actions as applicable. A collection summary may use the proposed cropped artwork header. Do not embed game art behind its editable list.

A collection is a name and an ordered list of mods. Do not add collection-level mod settings. Enabling a library mod adds it to the active collection; disabling removes it from that collection while keeping its downloaded files. Confirmed edits save automatically. Changes to the active collection apply automatically when the game is closed; while it runs, show `Waiting for game to close` and apply the latest saved revision after exit. There is no separate Apply button.

If Starframe closes before the game, retain those pending changes until the desktop app next opens. Explain this in the pending status when closing; do not leave a process running to apply them.

Show numbered load order and provide both drag handles and keyboard-accessible move actions. Resolve required dependencies automatically. Manual movement changes priority only where the dependency rules permit it. Explain adjustments beside the affected row, such as `Core Library must load before Terrain Tools`. Show the effective order that will run. A cycle identifies the affected mods and the next action. If a package or integration cannot honor manual order, explain that limitation and disable its reorder control.

The share/import review shows which exact releases are already downloaded, which will be downloaded, which are local-only, and which are unavailable. Once the user accepts import, reuse verified matches and download missing approved releases automatically, preserving the shared order. Keep unavailable entries visible for repair; never silently substitute the latest release. An imported file cannot approve arbitrary download links. Local-only builds need matching imported content on the receiving computer. Sharing excludes binaries, local paths and mod settings.

### Local development

Use the same list, details, switches, load order, and collection controls as catalog mods. Show the local source path in details, with copy/open actions. Include an `Import local mod` entry point. Detect a rebuilt local mod and update its visible state without requiring a refresh. Apply a valid new copy automatically when it is active and the game is closed. While the game runs, show the pending build without claiming it has loaded. Incomplete builds retain the last usable copy. Do not show catalog update or compatibility-version checks for local imports. Uninstall removes the managed copy, never the source folder. DLL hot reload is not part of the first version.

### Launch and setup

The persistent launch area shows the active collection and a short readiness message. Its ready action reads **Launch Sanctuary Shattered Sun**. Use the full visible label and accessible name; do not shorten it to Play. During preparation, show `Preparing mods` beside the unavailable launch action; when the game runs, show `Game running`. Use `Finish setup` when setup is the next useful action.

Place Sanctuary artwork behind the launch action, fading roughly halfway into the navy surface with a restrained orange tint. This static fade is intentional. Keep the composition within the launch area, clear of mod rows. Start with a 320–400 px wide, at least 64 px high action where space permits. Let the label wrap to two lines and the launch area grow at narrow widths or large text sizes; never truncate the game name. The existing specimen demonstrates the revised label only; artwork selection and its final composition still need review.

Use real game artwork with recorded provenance and terms suitable for its intended use. Confirm those terms before bundling assets. Keep the action usable with a solid background if artwork is missing. Treat the image as decorative: no text baked into it, no duplicate accessible name, and no layout shift while it loads. Check text contrast over the actual crop in every state, with at least 4.5:1 for normal text. Use a sufficiently opaque backing behind the label where needed; palette measurements alone do not verify an image-backed control.

Use a short opacity transition for hover/focus emphasis and immediate pressed feedback. Keep the dissolved edge static, with no particle loop, moving mask or idle glow. Reduced motion retains the static artwork and all state feedback. Native CSS/Svelte behavior comes first; this treatment does not require an animation library.

Problems have a nearby `View issues` action and a count only when the count is real. Downloading and applying changes must not appear complete until they succeed. While the game runs, collection edits remain available and show that application is waiting for exit. Launch waits for the latest valid active collection to finish preparation. A game-version warning alone leaves launch available.

First-run setup uses one focused sequence: locate game, explain the loader requirement, review the planned setup, show progress, then show the resulting state. Reuse native file pickers. Do not make users navigate multiple settings pages for the initial setup.

### In-game mod settings

Add a **Mods** entry to Sanctuary's main menu using the game's existing menu structure and controls. Open an installed-mod list from that entry, show which mods are enabled and which actually loaded, and let players select a mod to change its settings. Match the game's typography, spacing, panels, selection states, scrolling, transitions, input navigation and back/close behavior. Reuse the game's UI components where its integration permits. Starframe owns the implementation and settings behavior; the desktop navy-and-orange skin does not carry into this menu.

Use the [monochrome Starframe mark](docs/design/starframe-mark-mono.svg) for the Mods entry. Keep the approved frame-and-sun geometry, but render both parts in the same tint as the other menu icons. Match their size, optical weight and hover/selected/disabled treatment. Retain the visible `Mods` label. Do not bake orange or white into the game icon.

The desktop prepares a metadata-only inventory for the game-side list, alongside the enabled activation entries. Disabled entries must not load DLLs to obtain display information. Distinguish disabled, loaded and failed states. A disabled mod without available settings should explain that its settings become available after enabling it and restarting. The list reflects the prepared setup for this game session; pending desktop changes do not pretend to be loaded.

The former browser in-game screen is superseded. Its replacement in the review artifact is a direction note, not a game preview. Inspect the actual game menus before implementation, then verify the Mods entry, focus/input capture, back behavior and UI scaling in-game. Do not claim game-native visual fidelity from a browser mockup.

Show each mod's named settings using suitable controls for its supported types, with descriptions and defaults where supplied. Save changes to that mod's configuration. Settings stay the same when a collection changes. A setting applies live only when the mod supports it; otherwise display `Takes effect after restart`. The desktop does not maintain a second copy to merge. Loading and collection membership changes still wait for a game restart rather than promising live DLL unloading.

### Live behavior and responsiveness

Open the app shell and available local data before waiting for network checks. Mark cached information where freshness matters. Startup checks must not cover the app with a blocking loading screen.

- Give pressed, selection, navigation, and pending-state feedback immediately. Target visible input feedback within 100 ms under representative background load. This is a validation target, not a claim about measured performance.
- Keep navigation, scrolling, search, detail views, and unrelated actions usable during background work. Network requests, archive extraction, hashing, and lengthy file operations must not occupy the UI thread.
- Update every affected view from actual state changes. Counts, lists, details, collection readiness, and Downloads must agree without a page reload or manual refresh. Preserve focus, selection, scroll position, and unfinished edits when data changes.
- Show requested changes immediately when safe, with a pending state until confirmed. Reversible edits may update optimistically; if they fail, restore the confirmed state and explain what happened. Never claim that installation, removal, or game launch succeeded before it did.
- Restrict only actions that conflict with current work or cannot be completed. Use a subdued control treatment and a readable nearby reason, such as `Available when the game closes`. Do not make a tooltip on an unfocusable disabled button the only explanation.
- Queue or serialize conflicting changes to the same files. Users can continue work elsewhere. Prevent duplicate jobs from repeated clicks, and do not let an older response overwrite a newer choice.
- Show progress for long operations, with cancellation where it is safe. If an operation has entered a stage that cannot be canceled safely, explain that briefly. Errors remain attached to the affected task and offer a useful next action.
- Reconcile external changes, such as a replaced local build or the game closing, without requiring the user to restart the app. Check again after resuming from sleep if observations may have been missed.

Target smooth motion at 60 frames per second on the agreed baseline hardware. Input and useful state updates take priority over animation. Validate this behavior under slow downloads and file operations; choosing a framework alone does not establish responsiveness.

### App updates from GitHub

Run the startup check after the shell is usable, then check every five minutes while the app is open. Five minutes is the selected default within the user's suggested range. Checks also continue while the window is minimized. After sleep or connectivity returns, run one check if due, with no burst of missed checks. Keep requests from overlapping and delay retries if the server requests it. Settings also provides `Check for updates`, the installed version, and the last successful check time.

All desktop checking runs within the app's lifetime. Closing the app exits it and stops update checks. Do not install a service, scheduled task, startup agent, or separate background updater. Do not keep the app running in the system tray after its window closes. If a file operation needs to finish safely before exit, explain that in the visible app instead of silently continuing after close. The in-game runtime remains part of an already-running game and ends with that game.

Use the official app repository's published releases. Compare release versions with the installed app version. The initial recommendation is stable releases only; a preview channel requires a separate product decision. Keep the repository address configurable by the maintainer, not an arbitrary imported collection.

When a newer eligible version exists, show a persistent, quiet `Update available` notice near Settings or Help. Opening it shows the installed version, available version, release notes, and `Update`, `View release` and `Later` actions. Keep the notice separate from the main launch action and do not interrupt a game launch with a modal. Dismissing the notice must not cause repeated prompts for the same release; the update remains accessible in Settings.

The user chooses when to install or restart. Background checking does not authorize automatic installation. An app update must wait for active file changes to reach a safe stopping point and, initially, for the game to close because the release can include a new in-game runtime. Explain that reason beside the action. Failure to reach GitHub leaves mod management usable and shows a nonblocking check status. Do not label a failed check as `Up to date`.

Use Tauri's NSIS installer, generated Windows uninstaller, and signed updater as specified in [ARCHITECTURE.md](ARCHITECTURE.md). Preserve user data by default. Provide `Remove Starframe from game` in Settings to remove owned integration files safely, with the game closed. App removal and game cleanup must state their different effects. No custom installer framework or persistent update service is required.

## 8. Motion specification

Motion communicates cause, location, or completion. Immediate pressed and focus feedback must not wait for a transition. Keep animation interruptible; a second action starts from the current visual position. Never delay work to let an animation finish.

Default easing: cubic-bezier(0.2, 0, 0, 1), or the platform equivalent. Exits use cubic-bezier(0.4, 0, 1, 1). These are proposed timings to tune once with a representative screen, not a separate animation system to build.

| Trigger | Transition | Duration | Purpose |
| --- | --- | --- | --- |
| Hover button or row | Background/color only | 100 ms | Confirm the target without moving it |
| Change tab | Underline moves to selected tab | 160 ms | Preserve position between related views |
| Open detail panel | Translate 16 units from right + fade | 200 ms | Explain the panel's relation to the list |
| Close detail panel | Reverse toward right | 140 ms | Return attention to the selected row |
| Open menu | Fade + 4-unit movement from its trigger | 120 ms | Show where the actions belong |
| Open dialog | Fade + scale 0.98 to 1 | 180 ms | Establish a temporary foreground task |
| Expand details | Height reveal + chevron rotation | 180 ms | Show information belonging to the parent |
| Confirm a change | One icon crossfade to check | 140 ms | Acknowledge completion without celebration |
| Start/finish download | Progress region appears; label changes on actual events | 120 ms | Make task state clear |
| Apply list filter | Results replace in place | 80 ms fade, no travel | Preserve scanning position |

Use a tab underline rather than a sliding pill. Do not animate each list row on initial load. Progress uses actual bytes when known; otherwise use a labeled indeterminate indicator. A busy indicator is the only repeated motion, and stops when work stops. No spring overshoot on switches, counters, or data rows. No blur on changing text, error shakes, confetti, parallax, or hover tilt.

Respect the system reduced-motion preference. With reduced motion, remove translation, scaling, height animation, and moving tab indicators; update instantly or with an opacity change of at most 80 ms. Progress text and static busy labels must still communicate activity. Preserve all focus and state information.

## 9. States, accessibility, and copy

| State | Required presentation |
| --- | --- |
| No installed mods | `No mods installed` with `Browse catalog` and `Import local mod` |
| Empty collection | Explain that it contains no mods; provide an add action |
| Filter returns nothing | Keep the filters visible; offer `Clear filters` |
| Catalog loading | Named loading status in the content region; keep navigation usable |
| Offline | Keep installed mods and local collections usable; mark cached catalog information |
| Download fails | Identify the mod and error; show retry and source link |
| Game not found | Show the expected action: `Locate game` |
| Compatibility unknown | State that it has not been checked; do not show a success check |
| Blocking issue | Persistent issue text and a route to details; do not rely on a toast |
| Long mod name/path | Allow wrapping or provide the full value in details and copy access |

Use platform keyboard and accessibility semantics. Tab order follows the visual layout. Lists support arrow-key movement where the platform pattern supports it; Enter opens details and Space operates the focused selector. Escape closes dismissible surfaces and returns focus to their trigger. Menus and dialogs must not leak focus to hidden controls.

Maintain visible focus, label every icon-only action, and announce download completion and errors without repeatedly interrupting the screen reader. Status must not depend on color. Test Windows high-contrast settings, 100%, 150%, and 200% display scale, and 200% text scaling. Preserve readable content through wrapping and layout changes.

Write direct labels: `Install`, `Update`, `Enable`, `Disable`, `Uninstall`, `Import collection`, `Share collection`. Explain the consequence of uninstalling and how saved settings are handled once that policy is decided. Avoid alarmist copy for local imports; give the user source information and control.

## 10. Chosen stack

The user selected Tauri + Svelte + TypeScript, with Rust handling local operations. This is an installed desktop app with its interface bundled locally. [Tauri architecture](https://v2.tauri.app/start/)

- Tauri provides the desktop shell and the connection between the interface and native operations.
- Svelte and TypeScript define the interface, interaction states, and live presentation of data.
- Rust handles local file operations and game launching. Long-running work must leave the interface responsive and report progress and results back to it.

The in-game component uses C# for the verified Unity/Mono environment, with BepInEx providing bootstrap and reusable configuration support. Starframe owns its activation logic and settings presentation. The 0.1.1 build uses bundled SQLite with retained-source conversion of legacy Turso data. This changes no accepted interface behavior. Details and validation requirements belong in ARCHITECTURE.md.

Validate accessible mod lists, live state updates, safe file operations, installer/update behavior, and purposeful motion against this design. Verify responsiveness under representative background load. Framework versions and additional dependencies will be chosen during implementation planning. Ponytail is required for all coding and dependency decisions: reuse established code, standard libraries, and platform features before custom solutions. Do not add a UI library solely to obtain one transition.

## 11. Review and handoff

The [design specimen](docs/design/review.html), amended for 0.0.2, shows My mods, details, collections, download states and setup/update dialogs. Its in-game tab now records the game-native direction; it does not preview a replacement game menu. It includes normal, selected, busy, error and empty states, plus reduced motion and compact reflow. It uses fictional data and does not connect to game files or download mods. See [review evidence](docs/design/REVIEW.md) for the checked interactions and verification limits.

The selected identity uses an open structural frame around an orange sun, with a Bahnschrift wordmark. The [dark-surface mark](docs/design/starframe-mark.svg) and [light-surface mark](docs/design/starframe-mark-light.svg) use original geometry. The specimen shows small sizes and both surfaces. The user approved the identity and screens on 6 September 2026: "Approve this design for the handoff".

Handoff decisions and implementation checks:

- Original frame-and-sun mark and Bahnschrift wordmark selected.
- Fonts, neutral shades, navigation, screen structure and interaction direction approved through the specimen; verify their actual native rendering during implementation.
- Representative visual screens, motion direction and reduced-motion alternative approved.
- Accessible load-order controls remain approved. The in-game presentation now follows the game-native Mods menu described above; verify its visuals and Unity input behavior during implementation.
- The initial Windows target is defined in the [implementation handoff](docs/HANDOFF.md): Windows 11 x64, starting with 25H2. Verify supported releases and actual Windows integration during implementation.
- Validation of the chosen Windows installation and update flow remains an implementation requirement, not a claim made by visual review.

Handoff acceptance criteria:

- Palette, typography, spacing, radii, and motion follow the reviewed document.
- Orange-filled controls use navy labels; selected-row text uses verified pairings.
- Dense screens preserve readable names, versions, provenance, and state.
- Local imports have working management controls and clear origin labels.
- Every interactive element has a defined action and all required states.
- Startup and periodic GitHub app-update checks are nonblocking; a newer release remains discoverable and installation stays under user control.
- Update checks run every five minutes only while the app is open. Closing it stops checks and exits the app; no service, scheduled task, tray process, or background updater remains active.
- Slow network and file operations leave navigation and unrelated actions responsive. Changes appear live without manual refresh and preserve the user's current context.
- Pending, failed, canceled, and completed operations remain distinct. Stale responses and repeated input cannot misrepresent state or create duplicate conflicting jobs.
- Keyboard, screen-reader semantics, high contrast, resizing, and reduced motion are verified in the actual UI.
- Collections preserve exact mod references and order; imports reuse downloaded matches before fetching missing approved releases. They contain no mod settings.
- Active collection edits apply automatically when the game is closed and remain visibly pending while it runs.
- Load-order controls respect dependencies and the actual integration capabilities; keyboard operation works without dragging.
- Catalog changes appear without an app update. Older game-version declarations warn while still allowing enable and launch.
- In-game settings have clear persistence and restart behavior, and remain usable after the desktop app closes.
- UI and runtime verification are recorded separately from design review.

Design verification: supplied colors recorded; contrast pairs measured; reference pages inspected; UI structure and motion purposes documented. Revision 0.6 recorded the original specimen approval. Revision 0.7 retains the logo and first-version desktop layout, replaces the in-game direction, and specifies the artwork-backed launch action. Earlier browser checks are recorded in docs/design/REVIEW.md. Game-native rendering and the final artwork crop remain unverified. Those design reviews did not build an installed app or runtime. Milestone 0.1.0 is now authorized; [issue #8 verification](docs/verification/desktop-state.md) records implementation checks separately from design approval.

Required working guidance: [Ponytail](https://github.com/dietrichgebert/ponytail), [Anti-slop](https://github.com/miqdadbadjuber/anti-slop), and [Avoid AI Writing](https://github.com/conorbronsdon/avoid-ai-writing). These are installed globally in Codex. Read the applicable skill instructions when beginning the corresponding work.

Use UI Skills' [fixing-accessibility](https://github.com/ibelick/ui-skills/tree/83b757b8bba91b7268b8e8d370f9a8052a7943c5/skills/fixing-accessibility) for desktop controls and [fixing-motion-performance](https://github.com/ibelick/ui-skills/tree/83b757b8bba91b7268b8e8d370f9a8052a7943c5/skills/fixing-motion-performance) for animation decisions. Apply them within Svelte and the accepted design. The installed `improve-ui` skill is for explicitly requested audits and produces a plan; it is not the workflow for ordinary UI implementation. These skills do not require React, Tailwind or a new animation dependency. Game-side controls follow equivalent input and readability goals through the game's UI system, rather than HTML/ARIA rules.
