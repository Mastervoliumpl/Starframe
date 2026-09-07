# Owner display-scaling review

On 7 September 2026, the owner supplied nine screenshots after the requested Windows 100%/150%/200% display-scaling procedure and reported: "all text and things were readable, and it looks good to me."

Result: owner visual/readability approval for the submitted desktop and in-game settings views. No clipping or overlap requiring a fix was identified in those views. The screenshots are not individually labelled with a Windows scale percentage, so no exact percentage-to-image mapping is asserted.

## Evidence

The original PNGs are retained locally, unchanged, under ignored `test-results/manual-scaling-review-2026-09-07`. Its `manifest.json` records attachment order, original filenames and SHA-256 hashes. All nine copies were verified against their source files. These are test evidence, not reusable brand exports.

- Images 1–6 show the Windows desktop Settings page at different window/text sizes. The uppercase STARFRAME wordmark, full launch label and artwork remain visible. The launch label has no separate background rectangle. The content area has vertical scrolling where needed.
- Images 7–9 show the game-native Mods settings page with `fixture.first` enabled and loaded. All five setting controls, descriptions, Back and Reset this mod are visible. Marker count displays 27.
- The desktop screenshots show the manually selected playtest installation and an installed runtime, with the game stopped. The user reported that Find in Steam returned no results before manual selection; the exit review traced this to debug test isolation: `STARFRAME_TEST_DATA_DIR` intentionally bypasses registry discovery unless `STARFRAME_TEST_STEAM_ROOT` is also set. The manual instructions omitted that second variable. This is not a failure of normal registry discovery.

This submission records visual approval. It does not independently prove keyboard sequences, unsaved-edit preservation, reset behavior, fresh-process persistence or cleanup; prior checks of those behaviors retain their own evidence. The screenshot values alone do not establish a new persistence test. Long-content and enlarged-text checks are recorded in the existing browser and runtime verification documents.

## Completion review

The owner paused work after submitting the images, then authorized completion and merge. The accepted visual changes are included in 0.2.0. See [milestone exit evidence](milestone-0.2.0.md) for the remaining checks and product limits.
