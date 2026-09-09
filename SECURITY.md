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

See [package evidence](docs/verification/packages.md), [mod lifecycle evidence](docs/verification/mods.md), [management screens](docs/verification/management.md), [bootstrap evidence](docs/verification/bootstrap.md) and [runtime evidence](docs/verification/runtime-activation.md). Tests establish the stated cases, not a complete security audit. The status distinctions below remain separate from the authenticated advisory delivery implemented in the active 0.6.0 branch.

## Internal lifecycle delivered in 0.3.0

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

The desktop uses the maintained TUF client selected in the [authentication evaluation](docs/verification/catalog-authentication.md) with static GitHub hosting and a separate catalog authority. On 9 September 2026 the owner chose daily renewal through GitHub Actions with thirty-day validity. Expired or unverified metadata pauses new catalog downloads until a valid signed refresh succeeds. Installed offline use remains available, stale/unknown status is visible, and cached confirmed blocks remain effective. A mod's age or lack of new releases is not a security finding. A signed but stale catalog cannot reveal warnings the client has never received. The publisher remains disabled pending hosted acceptance under #46 and #29.

Security advisories identify exact release IDs and artifact hashes, evidence, confirmation state, recommended action and correction history. The validator also matches known payload hashes in local copies, requires appended corrections and rejects silently removed findings. Schema 13 commits verified catalog and advisory data together and retains findings after expiry. Download, enable and activation checks enforce retained confirmed findings, including local copies. The desktop displays persistent warnings and evidence history while keeping disable/uninstall controls available. Suspected findings do not trigger those blocks; signed corrections can clear a finding. Files/settings remain intact and running games are not mutated. Ordinary withdrawal remains separate. Starframe cannot stop code already executing or prevent launches outside its own launch path.

Follow the [curator review and reporting process](docs/catalog-review.md): record the reviewed artifact, available source revision, scope and test evidence. Use the [mod problem form](https://github.com/Mastervoliumpl/Starframe/issues/new?template=mod-report.yml) for ordinary problems and [private security reporting](https://github.com/Mastervoliumpl/Starframe/security/advisories/new) for suspected malicious code or vulnerabilities. Private reporting is enabled; no response-time guarantee is offered. Reviewing source alone does not prove a published binary matches it. Approval is not a safety certification. Remove secrets, usernames, personal paths and private logs from public reports.

## Distribution: active 0.6.0

The local NSIS candidate and its [packaging checks](docs/verification/windows-installer.md) are in progress under #27. It has no publication approval. On 9 September 2026 the owner deferred Windows Authenticode to future work without a milestone. The initial alpha may therefore have no Windows publisher signature. [SignPath form notes](docs/planning/signpath-request.md) replace the earlier email-style enquiry; no application has been submitted.

Issues #28–#29 retain private/public-key signatures over final installer/update artifacts, verified update metadata and hosted release approval. Catalog/advisory authentication remains in #46 under a separate key and authority. Embed the update verification key in the app; keep private keys out of source, logs and untrusted PR jobs. Document backups, rotation, lost-key recovery and compromise response. Use maintained signing tools and Tauri's required updater verifier, not custom cryptography. Detached signatures for manual installer downloads require a trusted public-key fingerprint; an installer cannot establish its own trust merely by carrying a public key. These checks are planned, not implemented by the existing unsigned candidate.

Use hosted builds from checked trusted revisions and explicit maintainer release approval. No dedicated signing computer or server is assumed. Windows certificate enrollment is not a 0.6.0 prerequisite. Revisit a suitable certificate service when that backlog issue is scheduled; no paid subscription, identity submission or hardware-key changes are authorized.

Document possible SmartScreen warnings and policy blocks for the unsigned alpha. Do not tell users to disable Defender, bypass malware detections or install a project certificate as a trusted root. A self-signed Windows certificate does not supply public publisher trust. Even a publicly trusted signature does not guarantee the absence of SmartScreen warnings. Artifact/catalog verification, bounded fuzzing and alpha acceptance remain distribution requirements.

## References

- [TUF roles, signatures and freshness metadata](https://theupdateframework.io/docs/metadata/)
- [Tauri updater signing](https://v2.tauri.app/plugin/updater/)
- [Windows signing and SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
- [SignPath Foundation eligibility](https://signpath.org/terms.html)
- [RustSec dependency auditing](https://rustsec.org/)
