# Catalog verification for issue #16

Verified on Windows on 7 September 2026, on `codex/0.3.0-curated-mods` at app version `0.3.0-dev.1`. Main remains on completed 0.2.0. This work adds metadata validation and refresh; it does not download mod packages or deploy them to the game.

## Automated results

- PASS: 50 Rust tests, including six catalog/storage checks. The four ignored worker entry points run through their parent interruption tests. Rustfmt, Clippy with warnings denied, generated TypeScript contracts and the local catalog CI command pass.
- PASS: schema fixtures reject unsupported versions, duplicate fields/IDs, invalid hashes, unsafe layouts, missing dependencies and cycles. Revision checks reject identity reuse, deleted releases, rollback and changed metadata without a new revision. Withdrawn releases cannot authorize new downloads.
- PASS: loopback HTTP fixtures exercise 200 and 304, ETag and Last-Modified requests, redirects, 404/429/503, numeric Retry-After, oversized responses with and without Content-Length, and truncated bodies. No external download is needed for these checks.
- PASS: scheduler checks cover waiting for the shell, non-overlap, five-minute success intervals, exponential backoff, resume after a due time and cancellation on shutdown. Its test runs on one async thread and cancels the production request before that task executes network work.
- PASS: SQLite checks cover schema-5 migration, restart, backup restoration and an aborted cache update. Failed responses and rejected writes preserve the previous catalog and validators. A newer catalog is accepted at an unchanged app version; installed release records remain unchanged.
- PASS: `npm run check` passes formatting, lint, Svelte diagnostics, six Vitest tests and the production frontend build. All ten browser tests pass. The two new catalog cases cover live replacement with retained keyboard focus/app version, cached offline status, check time, 200% text, forced colors and reduced motion.
- PASS: the full native suite passes desktop, storage/conversion, game-discovery and catalog checks. These use isolated temporary data and game fixtures. The catalog check uses a refused loopback proxy, restarts with a seeded cache, confirms the persistent error and retained successful-check time, then closes its own app process. It does not depend on GitHub being reachable.
- PASS: Windows Tauri debug and release executable builds pass with the locked dependencies. Version synchronization, 18 Python repository tests, documentation links and `git diff --check` pass. No installer or app release was published.

## Status display review

The catalog page uses the existing navy surfaces, typography, orange navigation and error treatment. Its heading and status explain the current work; the persistent launch control retains the main action. The design settings are ENERGY 1 / RHYTHM 1 / MOTION 1. No new animation, decoration, navigation destination or visual asset was added.

- PASS, R-25: measured body/canvas contrast is 12.02:1, error/canvas is 12.34:1 and muted/canvas is 6.96:1. Existing control and focus styling is retained.
- PASS, R-03/R-27/R-32: browser checks cover loading, empty and offline text, keyboard focus across refresh and navigation, enlarged text without horizontal overflow, forced colors and reduced motion. Errors persist and are announced when the page is visible.
- PASS, R-23/R-26/R-35/R-38: the native screenshot was inspected. The screen retains the existing GitHub action, whose approved destination is covered by command tests. The initial catalog is empty; test records are synthetic and confined to fixtures. No approval or download capability is fabricated.
- PASS, purpose and consistency checks: status text uses existing page structure and colors to report refresh results. No visual redesign or additional interaction control was introduced. The network error gives a connection/retry instruction instead of exposing a long request URL.

The native screenshot is retained locally under ignored `test-results/native/catalog-offline.png`. Browser checks establish reflow and interaction behavior; they do not constitute a new manual Windows display-scaling matrix or screen-reader certification. No in-game UI changed.

## Publication and remaining milestone work

The initial [metadata file](../../catalog/releases.json) contains zero approved releases. [The schema and publication procedure](../../catalog/README.md) explain how a maintainer adds reviewed releases without changing the app version. Catalog validation is added to the required Windows CI job and compares retained identities against the pull-request base or prior main revision.

The branch has not been pushed, so GitHub Actions has not run for this change. The raw main-branch endpoint remains unpublished until a maintainer merges the metadata. Issue #16 therefore remains open. No milestone was closed, and issues #17–#19 have not started. Public-endpoint verification remains a publication step; fixture and native tests establish local behavior only.
