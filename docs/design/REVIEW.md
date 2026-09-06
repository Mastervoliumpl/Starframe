# Starframe design review

Prepared and approved on 6 September 2026 for issues #3 and #4.

Open [review.html](review.html) directly in a browser. It is self-contained apart from the two adjacent SVG marks; no build, network request, persistent storage or game access is required. A local HTTP preview is optional, not a shipped service.

## Identity proposal

The mark is an open structural frame around an orange sun. Its offset frame pieces suggest a structure that can be assembled or changed, consistent with the Starframe name. The wordmark uses Bahnschrift with the accepted system-font fallback. The mark is original SVG geometry created for this project. No game logo, borrowed artwork or bundled font file is included. The project's existing license applies to the committed geometry; no separate asset license is introduced.

- [Dark-surface mark](starframe-mark.svg): white frame, orange center, transparent background.
- [Light-surface mark](starframe-mark-light.svg): navy frame, orange center, transparent background.
- The Identity tab shows each at 64, 48, 32, 24 and 16 logical pixels, plus the wordmark and the six supplied colors.

Use at least half the mark's displayed width as clear space from unrelated content. In the app sidebar, the mark sits beside the name, not as a repeated decorative background. Use the frame-and-sun mark only for product identity; approval and readiness use text/icons with their own meaning. Final Windows ICO generation belongs with packaging so it can be checked at the actual taskbar/install sizes.

## Review route

1. Start on Desktop → My mods. Search, select a row, open details and close them with Escape.
2. Set the scenario to Game running. Change a switch, observe the saved pending state, then use Simulate game exit.
3. Open Collections → Skirmish. Move Terrain Tools above Core Library. The order stays valid and explains the dependency. Move Tactical Overlay with the buttons or by dragging.
4. Open Share and inspect the sample ordered references. Inspect Import collection for reuse/download information.
5. Set Download error, retry, and navigate elsewhere during the simulated transfer. Use Empty library to review the empty state.
6. Open Settings → Game setup, then the update notice. Dialogs close with Escape and return focus to their trigger.
7. Switch to In-game settings. Change a live option and the restart option. Leave and return to confirm in-memory settings remain in the specimen.
8. Compare reduced motion and compact window widths. Review the Identity tab on dark and light surfaces.

Fictional names, builds and hashes in this artifact are examples. The simplified sample collection export illustrates identity/order only; it is not the final validated sharing schema. The installed app must follow ARCHITECTURE.md and the contract fixtures, not copy this specimen's in-memory implementation.

## Verification performed

Automated browser checks ran in local Microsoft Edge through the available Playwright runtime. They exercised:

- Saved pending changes, game exit and available Play behavior.
- Search/selection, detail opening, Escape dismissal and focus return.
- Keyboard dependency adjustment and drag ordering.
- Shared reference order in the sample dialog.
- In-game setting persistence within the specimen and restart-required text.
- Download failure/retry/cancel and navigation during progress.
- Empty-state navigation and setup-dialog dismissal.
- Reduced-motion behavior and a forced-colors rendering.

No page errors occurred. Desktop, in-game and identity views had no document-width overflow at 1280, 853 and 640 CSS pixels. Those widths simulate the available space of a 1280-pixel display at 100%, 150% and 200% scale. Rendered views were inspected at standard and compact widths. This is a reflow check, not a claim that native Windows display/text scaling or assistive technology has been tested.

The specimen uses the previously measured DESIGN.md color pairs. Selected-row metadata uses light gray rather than muted gray. Orange-filled actions use navy labels. Forced-colors rendering retains text and native control boundaries. The identity color on a light surface is branding; it is not a functional control boundary or small body text.

Not yet verified: native Tauri keyboard/window behavior, actual Windows screen-reader/high-contrast operation, 200% OS text scaling, measured input latency under real Rust work, Unity input capture and game UI scaling, game loading, database recovery, installer/updater behavior. These remain explicit implementation checks in the existing milestones.

## Review decision

The user approved the frame-and-sun identity and desktop/in-game screens on 6 September 2026: "Approve this design for the handoff". This selects the shown identity, screen structure and interaction direction for implementation. It does not turn the browser specimen into production code or waive the native verification listed above.
