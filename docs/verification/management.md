# Live management screens for issue #19

My mods and Catalog use the same bounded list/details component. Selection checkboxes are separate from enable switches. Search, selection, page and scroll state stay mounted across navigation and catalog refreshes. Lists render at most 100 rows per page. Below 1,280 pixels, details replace the workspace list; Back restores focus and scroll.

The management store reads the library/catalog and package progress once per second, without overlapping reads. Mutations display pending state and wait for desktop acknowledgement. Generation checks discard reads started before a mutation. A silent reply times out with an explicit unconfirmed result; it never claims success. Membership actions serialize revisions, and a failed bulk action retains earlier committed entries. Uninstall displays affected collection names and submits the revision shown in the confirmation dialog.

Catalog metadata supports descriptions, unmaintained labels, withdrawal reasons and build-specific compatibility findings with evidence sources. Missing test evidence displays “Not tested with this version.” Game observation changes recalculate the label. Compatibility and maintenance do not disable Enable or Play. Withdrawal prevents new downloads while preserving verified installed copies. Failed HTTP requests stay retryable operation errors, separate from catalog withdrawal.

Downloads displays real package progress, cancellation, retained task failures and retry of the exact approved release. Completion means verified library content; enabling and game readiness are separate. Pending uninstall cleanup has a retry control. Source buttons resolve a mod ID against the validated cache in Rust before opening its HTTPS author/source page. No webview receives a general file-opening or shell command.

## Checks

- Browser fixtures cover selection versus activation, bulk actions, installed withdrawal, failed/slow transfers, cancellation, source/detail focus, dialog Escape, retained search and bounded rendering with 1,200 fixture releases.
- Browser layout checks cover the existing desktop sizes, 200% text, forced colors and reduced motion. Existing navigation/search responsiveness checks remain in the full suite.
- State tests cover stale replies, repeated edits, revision changes within bulk operations, partial failure and unconfirmed replies.
- Rust fixtures cover independent catalog status facts, evidence URL validation and archive-root activation paths alongside the existing package/deployment recovery cases.
- The native lifecycle fixture operates the new Enable and Uninstall controls against temporary game/library data, verifies deployment and preserves settings. Its DLL bytes are inert; it does not establish compatibility with a real author mod.

No public catalog opening, installer, advisory service or signing implementation is part of this milestone. The initial catalog remains empty. Collection editing/sharing and local imports retain their later milestone scope. Earlier caches without new optional metadata remain readable and display missing withdrawal reasons explicitly.

Verified locally on Windows on 7 September 2026: 72 Rust checks, 10 frontend tests, 13 browser checks and the complete native suite passed. Rustfmt, Clippy, frontend formatting/lint/types and the native debug build passed. The native responsiveness fixture recorded a 39.7 ms p95 across 100 searches at 150% display scale. Native list/details screenshots were inspected for wrapping, control placement and readable status text. Required hosted checks and the final version's release build remain merge gates.

The first hosted run exposed a native test race: the activation manifest can change before the same journal finishes copying/removing mod files. The fixture now waits for both the manifest count and the expected payload hash/absence, instead of treating the manifest alone as deployment completion.
