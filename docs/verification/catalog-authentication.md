# Catalog authentication, issue 46

The desktop refresh uses `tough` 0.24.0 to verify both `catalog.json` and `advisories.json` from the same signed repository before accepting either. SQLite schema 13 commits the verified pair together. Download and activation checks enforce retained confirmed findings, and management screens retain evidence and correction history. The embedded production root and local signed candidate are prepared; no signed catalog has been published.

## Tooling decision

[tough](https://github.com/awslabs/tough/releases/tag/tough-v0.24.0) supplies the Rust client and [tuftool](https://github.com/awslabs/tough/tree/tuftool-v0.17.0/tuftool) supplies a publisher for static TUF repositories. The maintained alternative [rust-tuf](https://github.com/theupdateframework/rust-tuf) was also inspected; its current package identifies itself as 0.3.0-beta15. Both provide established metadata verification. Starframe uses tough's repository loader and publisher interfaces to keep signature, role and rotation rules in the maintained library. A detached file signature alone would leave Starframe to implement those repository rules.

The integration disables tough's optional HTTP transport and reuses Starframe's existing reqwest client: HTTPS, no redirects, connection and request timeouts, and no hidden retries. It permits only the two configured metadata/target directory URLs, bounds each response to 2 MiB and each refresh to 40 requests and 60 seconds. TUF metadata limits further restrict root/timestamp files to 64 KiB, snapshot/targets metadata to 256 KiB where the library's signed-length rules permit, and root updates to 32 per attempt. The final catalog target must fit 2 MiB even when a trusted signer declares a larger target.

The Windows dependency graph adds 30 non-development packages, including the client and AWS-LC verification dependency, for 341 Rust packages plus two bundled JavaScript packages. Existing futures utilities supply stream adapters. The refreshed notice inventory retains the new source and license records. `typed-path` 0.9.3 omits standalone license files; its [pinned README](https://github.com/chipsenkbeil/typed-path/blob/08cae85913d37b862b1d0e1aa2fa9e3857e9312e/README.md) offers MIT or Apache-2.0, so the inventory includes the [Apache-2.0 text](https://www.apache.org/licenses/LICENSE-2.0.txt).

## Trust and freshness

The caller supplies an embedded trusted root and an owned persistent TUF datastore. The module rejects reparse points in that directory, retains metadata versions for rollback checks and resumes from an accepted cached root. Retaining the root matters even if a later refresh stage fails; an old embedded root must not restore a revoked signing authority. This datastore assumes the same local integrity boundary as Starframe's existing saved data; it is not protection against a hostile process with the user's filesystem access.

On 9 September 2026 the owner selected daily GitHub Actions renewal with thirty-day validity. Expired metadata pauses new catalog downloads. Installed offline use and cached confirmed security blocks remain effective. Catalog/advisory keys are separate from app-release keys. The [publisher and disabled workflow](../catalog-publishing.md) implement renewal and checked publication; disposable RSA-key fixtures passed locally. The owner confirmed the production recovery-key backup. No renewal job is active yet.

## Verification

The focused Rust fixtures create fresh Ed25519 keys in memory and sign real TUF metadata through tough's publisher API. No private key material is committed or logged. The tests verify:

- Valid catalog/advisory bytes and their shared authenticated expiry; tampering with either target rejects the refresh.
- Rejection of changed target bytes and signed-metadata tampering.
- Rollback rejection after a newer signed version was retained.
- Expired metadata rejection and subsequent valid recovery.
- Root rotation signed by both old and new authorities, retained even when expired timestamp metadata interrupts the refresh.
- Rejection of an unrelated replacement root.
- HTTP error/redirect rejection, missing-file classification, oversized and truncated responses, endpoint restrictions and the request budget.

## Advisory records and saved state

The [advisory validator](../../src-tauri/src/catalog/advisories.rs) accepts schema 1 documents up to 1 MiB, with a positive decimal revision and at most 256 advisories. Each advisory retains an ID, title, exact release/archive identities, optional known payload hashes, and chronological evidence, explanation, state and recommended-action entries. The states are `suspected`, `confirmed` and `cleared`. Matching uses archive or payload hashes, including local copies with different filenames or import identities. It does not classify a changed, unknown binary as safe.

Corrections append history and increase the advisory revision. Previously received findings cannot disappear or have their history rewritten. A later `cleared` entry removes that finding from active matches. Affected identities remain fixed; additional affected artifacts need a new advisory. These records stay separate from ordinary withdrawal, maintenance and compatibility metadata.

Schema 13 adds one bounded `catalog_security` record. Only a verifier-produced catalog can save this record through the storage API. Its advisory data, authenticated expiry and normalized catalog hash commit in the same transaction as the catalog cache. Existing unsigned caches migrate without acquiring authenticated status. Once authenticated, the ordinary cache writer may update refresh results but cannot replace catalog content or extend the signed expiry.

The storage tests verify migration from schema 12, restart and backup/restore, failure of the second write rolling back both targets, rejection of omitted or rewritten findings, an appended correction, revision rollback, clock rollback, expiry at the exact boundary and mismatched catalog content. Expiry does not delete cached confirmed findings. These tests use freshly signed fixture repositories through the production verifier.

## Enforcement

Catalog downloads check confirmed archive and known cached payload findings before starting. Package completion checks again, so a finding received during preparation can reject completion. Enabling a mod also checks its required dependencies. Every requested activation checks all selected catalog and local payloads; the launch path makes this check before repairing or preparing game files. Changed findings advance the saved-data revision so the existing stopped-game preparation loop rechecks the active setup.

Confirmed findings stop launch through Starframe until the affected selections are disabled or a signed correction clears the finding. They do not delete library files, settings or collection intent. A local import remains available for management, but matching confirmed payloads cannot be enabled or included in a Starframe launch. Suspected findings do not trigger these blocks. The desktop shows a persistent warning for installed affected copies and keeps Disable and Uninstall available. Details retain suspected, confirmed and cleared history; evidence buttons resolve only retained advisory URLs. The checks cannot stop code already running in the game or prevent launches outside Starframe.

Rust regression tests cover an already enabled dependency, a finding arriving during package preparation, restart/expiry retention, disabling blocked selections, signed correction and restored activation. A separate test imports the same payload under another mod identity and filename, verifies that suspected findings permit activation, then confirms that the same bytes remain blocked after confirmation. Source and managed files remain present. The full Rust suite, formatter and Clippy passed on 9 September 2026.

The rebuilt Windows debug app passed the affected native saved-data, mod-management and local-import checks: legacy conversion and restart, navigation during held storage startup, collection controls, settings retention, game-exit application, ordering/Lua deployment, local folder selection, offline import and retained sources. These existing desktop fixtures do not yet establish the advisory warning display or signed network refresh.

New downloads require a matching authenticated catalog and a valid clock/expiry window at start and completion. Regression tests reject unsigned-cache starts and expiry during preparation without creating a library entry. Existing reuse tests continue to verify cached files offline and reject changed files without a network fallback. Signed collection fixtures cover missing downloads through the same queue. Refresh tests retain the previous pair on failure, restore it after restart, bound retries, avoid overlap and abort on desktop close.

All 25 browser checks passed on 9 September 2026, including expired/unverified download controls, offline library activation, confirmed/suspected/cleared findings, retained disable/uninstall controls, evidence history, focus, 200% text and forced colors. The full Rust suite passed with 118 library tests and the desktop/contract/integration tests; six library entries are child-process workers or deliberately separate extended campaigns. Frontend format/lint/type/unit/build checks and Rust Clippy also passed. The [reporting process](../catalog-review.md) records enabled private security reporting and the ordinary mod problem form.

All nine native Windows scripts passed against the rebuilt app on 9 September 2026. Catalog checks cover unverified startup, failed signed refresh, retained cache/error and restart. Package checks cover verified reuse without network access, changed-file rejection and expired suspected/confirmed/cleared advisory controls after restart. Confirmed matches reject package commands, preserve uninstall controls and show a persistent warning; evidence and corrections remain visible. The native screen was also inspected visually. Seeded saved-state fixtures do not replace the real TUF signature and correction-history tests above. The remaining native scripts cover storage, game discovery, collections, ordering, local imports/watchers and guarded lifecycle cleanup, using temporary installations.

The previous hosted run passed compilation and its other jobs but timed out while attaching to the initial Windows document. The shared native helper now waits for the Tauri document before assertions; a browser regression verifies new/existing page attachment. Offline catalog assertions also allow the configured connection timeout to finish. All [required hosted checks](https://github.com/Mastervoliumpl/Starframe/actions/runs/34408717870) and [dependency audits](https://github.com/Mastervoliumpl/Starframe/actions/runs/34408717813) passed on `8ba04f575d4fbf68d4be899e623748e9c9e93382`, including the native Windows scripts and release executable build. Publication and an initial live signed refresh remain pending.

On 10 September 2026 the publisher's disposable-key checks were extended to cover its full rotation path. The previous publication uses an explicitly retained reviewed public root, while the candidate must verify under both the old and new roots. Numbered root history is preserved. Tests replace the online authority, reject the revoked online key, replace the offline authority and verify from a client two root versions behind. A missing old-authority cross-signature rejects publication. Production keys were not rotated; the live publisher remains disabled.
