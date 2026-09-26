# Security and protection scope

Starframe is in internal development. There is no public installer release. Curation records approval of exact mod bytes after the maintainer's checks; it does not guarantee that a mod is malware-free. Mods run with the game's access to files and the network. Deployment recovery cannot undo changes made by code that has already executed.

## Implemented

- Restricted local Tauri commands and Rust argument validation; downloaded DLLs are never loaded in the desktop for inspection.
- Signed website registry keysets/releases/security decisions, exact identity, expiry/rollback enforcement and retained offline hash blocks. Production access remains unavailable until an independently approved Ed25519 root is provisioned. The old TUF catalog is retired; isolated fixtures establish local acceptance.
- Bounded downloads, SHA-256 verification, guarded ZIP extraction and verified library reuse. Failures retain operation history. Abrupt exits can leave private staging for recovery.
- Bootstrap/runtime deployment ownership, backups, Windows path guards, installation locks and interrupted-operation recovery. Game mutations require an observed stopped process.
- Saved mod enable/disable intent, streamed deployment, confirmed uninstall and retryable owned-artifact cleanup. Space estimates precede downloads, extraction and deployment; failed writes still require recovery.
- Runtime manifest and payload verification before managed activation. This is not a mod sandbox.
- Separate maintenance, compatibility and availability metadata in live management screens. Compatibility warnings permit use; ordinary withdrawal prevents new downloads but preserves installed copies. Persistent download errors remain retryable.

See [package evidence](docs/verification/packages.md), [mod lifecycle evidence](docs/verification/mods.md), [management screens](docs/verification/management.md), [bootstrap evidence](docs/verification/bootstrap.md) and [runtime evidence](docs/verification/runtime-activation.md). Tests establish the stated cases, not a complete security audit. The historical status distinctions below remain separate from signed registry security decisions.

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

## Registry authority and security decisions

The desktop verifies the website's root-signed delegated keyset, closed full release and security snapshots with exact identities and revision floors. A fresh matching snapshot is required for a new download. Retained blocked archive hashes prevent new download, enable and activation without requiring a session. A newer signed explicit clear removes that block; omitted decisions, sign-out and offline use do not. Ordinary installed use remains available when unblocked. See [trust verification](docs/verification/registry-trust.md).

Registry decisions concern exact archive hashes. The retired catalog's payload-hash advisories and evidence-history UI are not read or converted. Starframe retains sources, settings, collection intent and game journals. Security gates cannot stop already executing code or prevent launches outside Starframe.

The old catalog publisher, source root and dedicated hosted resources are retired under #89. Application signing remains separate. An independently approved production registry root is still a production prerequisite; fixture roots never authorize production downloads. See [retirement verification](docs/verification/catalog-retirement.md).

Follow the [curator review and reporting process](docs/catalog-review.md): record the reviewed artifact, available source revision, scope and test evidence. Use the [mod problem form](https://github.com/Mastervoliumpl/Starframe/issues/new?template=mod-report.yml) for ordinary problems and [private security reporting](https://github.com/Mastervoliumpl/Starframe/security/advisories/new) for suspected malicious code or vulnerabilities. Private reporting is enabled; no response-time guarantee is offered. Reviewing source alone does not prove a published binary matches it. Approval is not a safety certification. Remove secrets, usernames, personal paths and private logs from public reports.

## Distribution: 0.6.0

The NSIS candidate passed its [functional packaging checks](docs/verification/windows-installer.md) under #27. The hosted dev.2 draft passed signing and attachment verification under #29; it has no publication approval. On 9 September 2026 the owner deferred Windows Authenticode to future work without a milestone. The initial alpha may therefore have no Windows publisher signature. [SignPath form notes](docs/planning/signpath-request.md) replace the earlier email-style enquiry; no application has been submitted.

Issues #28–#29 implement private/public-key signatures over final installer/update artifacts, validated update metadata and hosted release approval. Historical catalog authentication was completed under #46 and retired under #89. Registry verification uses its separate root and authority. The update verification key is embedded in the app; private keys stay out of source, logs and untrusted PR jobs. [Update verification](docs/verification/app-updates.md) records backups, rotation, lost-key recovery and compromise response. Signing uses maintained tools and Tauri's required verifier. Detached signatures for manual installer downloads require an independently trusted public key; an installer cannot establish its own trust merely by carrying one. The dev.2 installer and signed checksum inventory passed hosted and local verification. The updater signature authenticates installer bytes; release discovery/metadata also relies on GitHub HTTPS and the client's identity checks.

Use hosted builds from checked trusted revisions and explicit maintainer release approval. No dedicated signing computer or server is assumed. Windows certificate enrollment is not a 0.6.0 prerequisite. Revisit a suitable certificate service when that backlog issue is scheduled; no paid subscription, identity submission or hardware-key changes are authorized.

Document possible SmartScreen warnings and policy blocks for the unsigned alpha. Do not tell users to disable Defender, bypass malware detections or install a project certificate as a trusted root. A self-signed Windows certificate does not supply public publisher trust. Even a publicly trusted signature does not guarantee the absence of SmartScreen warnings. Artifact/registry verification, bounded fuzzing and alpha acceptance remain distribution requirements.

## References

- [TUF roles, signatures and freshness metadata](https://theupdateframework.io/docs/metadata/)
- [Tauri updater signing](https://v2.tauri.app/plugin/updater/)
- [Windows signing and SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
- [SignPath Foundation eligibility](https://signpath.org/terms.html)
- [RustSec dependency auditing](https://rustsec.org/)
