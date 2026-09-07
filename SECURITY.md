# Security and protection scope

Starframe is in internal development. There is no public installer release. Curation records approval of exact mod bytes after the maintainer's checks; it does not guarantee that a mod is malware-free. Mods run with the game's access to files and the network. Deployment recovery cannot undo changes made by code that has already executed.

## Implemented

- Restricted local Tauri commands and Rust argument validation; downloaded DLLs are never loaded in the desktop for inspection.
- Validated catalog cache with retained release identities, revision checks and offline use. HTTPS and hashes currently provide transport/content integrity; the catalog has no independent signature or expiry.
- Bounded downloads, SHA-256 verification, guarded ZIP extraction and verified library reuse. Failures retain operation history. Abrupt exits can leave private staging for recovery.
- Bootstrap/runtime deployment ownership, backups, Windows path guards, installation locks and interrupted-operation recovery. Game mutations require an observed stopped process.
- Saved mod enable/disable intent, streamed deployment, confirmed uninstall and retryable owned-artifact cleanup. Space estimates precede downloads, extraction and deployment; failed writes still require recovery.
- Runtime manifest and payload verification before managed activation. This is not a mod sandbox.
- Separate maintenance, compatibility and availability metadata in live management screens. Compatibility warnings permit use; ordinary withdrawal prevents new downloads but preserves installed copies. Persistent download errors remain retryable.

See [package evidence](docs/verification/packages.md), [mod lifecycle evidence](docs/verification/mods.md), [management screens](docs/verification/management.md), [bootstrap evidence](docs/verification/bootstrap.md) and [runtime evidence](docs/verification/runtime-activation.md). Tests establish the stated cases, not a complete security audit. The status distinctions below are implemented for the internal curated-mod workflow; security advisory delivery remains separate future work.

## Current milestone: 0.3.0

Prioritize a working internal mod lifecycle. Extend existing issues rather than start a separate security overhaul.

| Information | Required behavior |
| --- | --- |
| Unmaintained | Label the maintenance status; keep installed copies usable. |
| Not tested with this game version | Show that wording when evidence is absent. A newer game build alone does not establish incompatibility. |
| Known compatibility problem | Describe the affected versions/combination and evidence. Compatibility warnings allow the user to continue; structural failures such as missing required files remain separate. |
| Download unavailable | Explain the failed request and permit retry. One failed request does not withdraw a release. |
| Withdrawn by author/curator | Retain identity/history and a reason. Block new downloads, preserve verified installed copies and settings, and do not treat ordinary withdrawal as malware. |
| Artifact differs from approval | Reject the bytes. Never silently substitute another archive, version or hash. |

Catalog issue #16 owns metadata distinctions; #17 owns transfer outcomes and space checks; #18 owns lifecycle/recovery; #19 owns their user messages. Compatibility and maintenance facts can coexist with availability information. Do not use one catch-all status that makes these facts mutually exclusive.

Estimate space before downloads and deployment, including archives, extraction, backups and simultaneous work on each affected volume. Recheck before expansion/mutation and still handle failed writes: free-space observations do not reserve disk space against other programs. Keep tests focused on disk exhaustion, file locks, changed files, interrupted steps and safe recovery using temporary installations. Retained staging needs a safe cleanup path; do not delete recovery evidence indiscriminately.

Run dependency scans in separate CI jobs on dependency changes, on a schedule and before distribution. Keep them out of ordinary local build commands. Report scanner/network failures as failed checks, not clean results. Review advisories and record the reason and expiry of any exception. Broader fuzzing and a detailed compatibility-report service are follow-up work.

## Before public catalog access

Internal 0.3.0 work can use the existing empty/fixture catalog. Opening a live catalog to general users requires catalog authentication and delivery of security advisories. Public catalog use is not an implicit exit requirement for the internal 0.3.0 milestone. If that distribution decision changes, bring these requirements forward before opening access.

Evaluate maintained signing/TUF tooling and static GitHub hosting. Separate catalog approval authority from application release authority. Choose a practical freshness policy before enforcing expiry; no 90-day policy has been adopted. A mod's age or lack of new releases is not a security finding. Preserve offline use when freshness checks fail, show the age/unknown status, and define separately whether stale metadata can authorize new downloads. A signed but stale catalog cannot reveal warnings the client has never received.

Security advisories must identify exact release IDs and artifact hashes, evidence, confirmation state, recommended action and correction history. Suspected findings stay visibly unconfirmed. Confirmed malicious artifacts are blocked from new downloads and activation through Starframe by default, including matching installed copies. Preserve files/settings, display a prominent warning, and do not mutate a running game. Cached confirmed findings remain effective offline. Advisory delivery is not implemented yet; ordinary withdrawal must not stand in for it.

The initial review/reporting process can use GitHub: record the reviewed artifact, available source revision, scope of review and test evidence; provide ordinary compatibility reports and a private route for security reports before public access. Reviewing source alone does not prove a published binary matches it. Never label approval as a safety certification. No private reporting route or response-time guarantee is currently advertised; do not post secrets or personal logs in public issues.

## Distribution: planned 0.6.0

Issues #27–#29 own Windows Authenticode, Tauri updater signatures and hosted release approval. Sign executables and the installer, then sign the final update artifact. Verify signatures and identities before publishing. Keep catalog signing, updater signing and Windows publisher signing separately authorized; protect credentials from pull-request code and retain recovery/rotation instructions and encrypted backups where applicable. A Windows signature does not guarantee removal of SmartScreen warnings.

Use hosted builds from checked trusted revisions and an explicit maintainer release approval. The maintainer has ordinary-use desktop/laptop computers and a personal YubiKey; assume no dedicated signing computer or server. Evaluate SignPath eligibility and suitable services before recommending a purchase. Check the YubiKey model and supported key operations only if hardware signing is selected, with the owner present; preserve existing keys, PINs, slots and configuration. No device changes are authorized by this plan.

Move signing requirements earlier if public installers are distributed before 0.6.0. Broader archive/path fuzzing and compatibility reporting remain planned follow-ups, not a condition for every local build.

## References

- [TUF roles, signatures and freshness metadata](https://theupdateframework.io/docs/metadata/)
- [Tauri updater signing](https://v2.tauri.app/plugin/updater/)
- [Windows signing and SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
- [SignPath Foundation eligibility](https://signpath.org/terms.html)
- [RustSec dependency auditing](https://rustsec.org/)
