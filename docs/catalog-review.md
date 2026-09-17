# Catalog review and reports

The owner approves each release. An approval identifies exact archive bytes; it does not certify that a mod is free of malware. Starframe downloads from the recorded author source and does not host mod binaries.

## Review record

For each new approval, keep a review in `docs/verification` and link it from `catalog/README.md`. Record:

- The mod and release IDs, author release/download URLs, archive SHA-256, size and selected package layout.
- The available source repository and exact commit or tag reviewed. Say when source is unavailable or when the published binary was not reproduced from it.
- What was inspected, which payloads are included or excluded, dependencies and any bundled runtime conflicts.
- The Starframe/game builds tested, observed results, limitations and the owner's approval evidence.

The existing [Ladder Reporter](verification/ladder-reporter-catalog.md) and [Remmy release reviews](verification/remmy-catalog.md) retain their original scope and limits. Approval of one artifact does not approve another archive with the same version label. Changed bytes require a new release identity and review. CI checks format and retained identity; it cannot replace curator review.

## Reports

Use the [mod problem form](https://github.com/Mastervoliumpl/Starframe/issues/new?template=mod-report.yml) for compatibility, download, installation or catalog mistakes. Include the exact release ID and SHA-256 from Package details, app/game versions, reproduction steps and relevant evidence. Public reports must omit usernames, personal paths, credentials and private logs. Do not upload mod binaries.

Use [GitHub private security reporting](https://github.com/Mastervoliumpl/Starframe/security/advisories/new) for suspected malicious mods or vulnerabilities. Private reporting was enabled for this repository on 9 September 2026. Explain the affected artifact, evidence, impact and reproduction steps without including live credentials. Reports go to repository maintainers; no response-time guarantee is offered. Do not publish exploit details in an ordinary issue while reporting privately.

## Findings and corrections

Triage maintenance, compatibility and ordinary withdrawal separately from security findings. Record unconfirmed evidence as `suspected`; this warns users and permits use. A `confirmed` finding blocks matching archives and known payload hashes from new downloads or activation through Starframe. Record the evidence and recommended action, and notify users through the signed advisory target. Keep the affected release IDs and hashes exact; do not substitute a newer version automatically.

Append a `cleared` history entry when review overturns a finding. Retain the previous evidence, explain the correction and increase the advisory revision. Never delete or rewrite an earlier finding to hide a correction. A fresh signed correction removes that finding's block; other confirmed findings still apply.

Starframe retains files, settings and collection intent. These controls do not undo code already executed, stop a running game or prevent a user from launching outside Starframe. An offline client retains findings it received, but cannot display new evidence until it reconnects. Once catalog metadata expires, new downloads pause until a valid signed refresh succeeds; installed copies remain usable unless a retained confirmed finding blocks them.
