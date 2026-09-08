# Collection sharing verification

Issue [#22](https://github.com/Mastervoliumpl/Starframe/issues/22), milestone 0.4.0. Verified on Windows on 8 September 2026 with development version `0.4.0-dev.1`.

## Delivered behavior

Collections export as version-1 `.starframe-collection.json` files. The format contains `format: "starframe-collection"`, `schemaVersion: 1`, a name and ordered `entries`. Each entry contains only `modId`, `hash`, `origin` and `releaseId`. Array position is the requested priority; dependency ordering still determines effective activation order. Files over 1 MiB, unsupported versions, duplicate mod IDs, invalid identities and unknown fields are rejected before storage changes. Binaries, settings, credentials, source paths, URLs and commands are excluded.

Import accepts either a file or pasted JSON. Review identifies cached content, approved downloads and unresolved references. One acceptance creates the collection and persistent progress in one SQLite transaction. The collection starts inactive. The existing bounded package queue verifies cached files before reuse and prepares missing exact approved releases without separate Install clicks. Imports never select a newer release or obtain approval from an imported link.

SQLite schema 9 adds import progress with collection-owned lifetime. Deleting a collection removes its progress without uninstalling library files. Closing Starframe cancels package work. Reopening retains interrupted references as unresolved; Retry import rechecks current approval and content. Invalid or incomplete active imports cannot reach deployment. A complete valid selected collection follows the existing game-closed application path.

## Verification

- Rust tests reproduce export/import/export through two independent fixture libraries, preserving exact requested references and validating effective activation order. Cached-file verification succeeds with unreachable fixture download URLs.
- Regressions cover changed bytes, withdrawn/deleted/hash-mismatched releases, reapproval identity conflicts, missing required references, matched and unmatched local content, rejected untrusted fields, unsupported formats and size limits.
- Queue tests confirm that one acceptance starts the exact missing approved release, cancellation retains its reference and prevents application, repeated acceptance does not duplicate a collection, transaction failure rolls back collection and progress together, and restart preserves interrupted work for retry.
- Existing package transfer tests cover successful HTTP fixture transfers, retries, interruption, archive verification and cancellation. The sharing tests reuse that queue; no live author package was downloaded for this issue.
- Browser checks cover file input, pasted JSON, review, one acceptance, immediate collection/progress display, failed downloads, unresolved references, export download, focus restoration and preservation of edits during polling. Enlarged text, forced colors and reduced motion retain usable dialog controls without horizontal overflow. Visual inspection confirms the existing management-dialog styling.
- Native Windows package fixtures verify sharing command permissions, review/import/export through the actual UI, offline verified reuse and saved collection retention. The mod lifecycle fixture verifies that a shared withdrawn release stays unresolved and cannot replace the existing activation, alongside collection switching, game-exit application, file retention and settings preservation.

Rustfmt, Clippy, frontend formatting, lint, type checks, build and tests pass. The full Rust suite passed before the final missing-dependency regression; the affected 19 mod/collection tests and Clippy passed after that addition. The full browser suite passed before the final enlarged-sharing-dialog test; all three sharing tests passed afterward. Native package and mod fixture results are recorded with the PR checks.

## Limits and recovery

Imported local references can reuse matching verified library content. Creating and activating general local imports still belongs to 0.5.0. This does not establish compatibility with arbitrary BepInEx plugins, Lua packages or maps. No real-game launch, gameplay, multiplayer, screen-reader speech review, installer or public catalog approval is claimed for #22. Milestone exit issue #23 retains the broader desktop/game acceptance work.

The owner assigned the separate review findings to planned milestone [0.4.1](https://github.com/Mastervoliumpl/Starframe/milestone/11): approval aliases [#50](https://github.com/Mastervoliumpl/Starframe/issues/50), disabled-library inventory limits [#51](https://github.com/Mastervoliumpl/Starframe/issues/51), package/runtime file-count limits [#52](https://github.com/Mastervoliumpl/Starframe/issues/52), and shared-archive deployment roots [#53](https://github.com/Mastervoliumpl/Starframe/issues/53). Sharing detects a mismatched saved approval and retains the incoming reference as unresolved; it does not implement the identity migration in #50. Runtime validation continues to block unsupported deployments.

For a stopped or failed transfer, use Retry import. For withdrawn or changed references and missing dependencies, ask the sender for a corrected file, or export the retained list and explicitly edit its references before importing it as a new collection. Existing collections and settings remain available.
