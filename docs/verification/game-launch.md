# Desktop setup and launch

Issue [#15](https://github.com/Mastervoliumpl/Starframe/issues/15), milestone 0.2.0. The desktop now prepares its runtime, launches the selected game and observes the process and runtime report independently.

## Behavior and limits

Settings offers game selection, runtime installation/retry and ownership-aware removal. The launch area routes incomplete setup to Settings. A ready action reads **Launch Sanctuary Shattered Sun**. Preparation runs on the existing background worker; navigation, diagnostics and local drafts remain usable. Only affected game actions become unavailable.

Before dispatch, the worker loads the requested SQLite revision, prepares a validated activation through the existing deployment journal, and reads the request again. A changed request is prepared again; continuous changes stop launch with a retry message. This milestone prepares an empty active collection. Non-empty collections fail explicitly without replacing their deployment; library/package materialization remains later milestone work. The existing developer fixture command still exercises managed mods and settings. A game-version declaration alone does not block launch; an installation changing during a file operation does.

Windows starts the exact revalidated `Sanctuary.exe` with its engine directory as the working directory. No shell, arbitrary arguments or frontend-provided executable path is accepted. The verified playtest supports this executable route. Steam URI launch has not been added or claimed as verified.

The UI separates preparation, accepted Windows launch request, observed process, matching runtime report and reported mod failures. It waits up to 30 seconds for a process before displaying a retry reason. After 60 seconds without a matching runtime report, it explains that verification is incomplete and continues observing. Reports are bounded and strictly parsed. A report must match the deployment revision, process ID, Windows process creation time and complete ordered mod list. A failure report exposes its per-mod messages in Settings. An empty report confirms the runtime with zero active mods; it does not claim the game has reached its main menu.

External game starts receive the same observation. Writes require a freshly verified stopped state and recheck the installation/process before each existing deployment boundary. An unknown process state postpones writes. Another program can still start the game between checks; the existing journal preserves recovery evidence rather than claiming an atomic launch/write exclusion.

Closing the desktop stops its workers. During a file operation, the visible window explains that it is waiting for a safe stopping point; a queued launch is not dispatched after close. An interrupted journal can recover on the next start when the game is stopped. Closing Starframe does not terminate the game or its in-game settings runtime.

## Internal builds

The Unity entry plugin still requires lawful compile-only game references; ordinary CI does not distribute that plugin or claim to build it. Build it as described in [runtime activation](runtime-activation.md), then stage the complete runtime and pinned BepInEx archive:

```sh
python scripts/prepare_desktop_runtime.py src-tauri/target/debug/integration --runtime runtime/Starframe.Bootstrap/bin/Release/netstandard2.1 --archive path/to/BepInEx_win_x64_5.4.23.5.zip
```

Use a new staging directory. The script copies only the explicit runtime dependency list and an empty activation template; game references and test probes are excluded. The archive and all bootstrap files are checked against the pinned inventory. Runtime files have a SHA-256 inventory checked again before deployment. The prepared resources are trusted build input, not an author-download approval mechanism.

The desktop reads `integration/bootstrap` and `integration/runtime` from its Tauri resource directory. A debug build can override that directory with `STARFRAME_INTEGRATION_DIR` for isolated tests. Release builds ignore that environment variable. A CI executable without runtime resources displays a setup failure and leaves the game unchanged. Installer/resource delivery and the full dependency-notice inventory remain distribution work; no installer or release has been published.

## Verification on 7 September 2026

- Rust: revision changes during preparation, repeated edits, failed writes/dispatch, non-empty collection retention, stale PID/start time/revision reports, incomplete mod reports and explicit runtime failures. Existing journal/interruption tests retain their process-start/write-race coverage. All 44 ordinary Rust tests pass; four helper tests marked ignored are invoked by their parent interruption tests.
- Frontend: formatting, lint, Svelte checks, six Vitest checks, production build and eight browser checks. Setup, keyboard launch, pending navigation, full-label reflow and reduced motion pass. Large-text fixtures now target `:root`, so the stylesheet's root rule does not override the requested test size.
- Runtime staging: missing output fails before staging; existing destinations are retained; game/probe DLLs are excluded. Five bootstrap/runtime preparation tests pass.
- Authorized game check: playtest build 25135612, Unity 6000.3.22, BepInEx 5.4.23.5. First-run runtime setup and desktop executable launch succeeded. The desktop confirmed a fresh process-bound report with zero active mods; navigation remained usable and runtime writes were disabled while running. The desktop closed while the game continued running. A separate executable start was also observed and matched to a fresh game session.
- Cleanup: runtime removal through the desktop passed after the test game stopped. Six pre-test engine-root files retained their SHA-256 values. Mod settings, logs, reports and unowned files were retained. No test-owned game process remains.

The game's external-launch shutdown stalled after Windows close requests, leaving a process with no main window. The verified test-owned process was stopped before removal. The combined smoke script therefore did not finish cleanly; a separate desktop cleanup check passed. This is not evidence of a verified graceful Windows-close path for the game. BepInEx also printed legacy Unity logging binding warnings; the runtime wrote matching reports despite those warnings. The in-game controls and persistence were checked in [#14 verification](runtime-settings.md), not re-certified by an empty runtime report.

Local evidence is under `test-results/native` (`launch-ready.png`, `launch-runtime-ready.png`, `launch-external.png`, `launch-removed.png`, `launch-cleanup-result.json`). A browser run initially cleared the shared `test-results` directory, including older local verification captures and staged packages. Browser output now has its own `test-results/browser` directory; current staging and game evidence were rebuilt. Earlier recorded results remain historical observations, but the deleted local captures are no longer available.

## Artwork review

The action uses the unchanged official Dyson-sphere image with a static CSS navy fade and orange tint. [Provenance and the owner's supplied permission record](../notices/Sanctuary-artwork.md) define the use conditions. Help & logs exposes the credit. No image loading is awaited by setup or launch. An opaque navy fallback works without the image, and forced colors removes the decoration.

Normal, hover, focus, pressed, disabled, narrow/doubled-text and missing-image states were captured. The full name remains visible and wraps at 640 px with doubled text. Text/background contrast was measured over the rendered label area with its text temporarily hidden: the lowest measured ratio was 5.69:1 in the pressed state, above 4.5:1. The opacity transition respects reduced motion; the crop and fade never move. Local captures and measurements are in `test-results/launch-artwork`.

The full Windows 100%/150%/200% display-scaling matrix and milestone exit review remain outstanding. Browser text-size and viewport checks do not substitute for that matrix. Main remains on completed 0.1.1 until 0.2.0 passes its exit checks.
