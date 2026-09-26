# Registry review and reports

The owner approves each release. An approval identifies exact archive bytes; it does not certify that a mod is free of malware. The website owns release intake, review and signed approval. The manager consumes its exact archive grant and declaration.

## Review record

For each approval, retain its website review and exact signed release identity. These manager records retain historical test evidence. Record:

- The mod and release IDs, author release/download URLs, archive SHA-256, size and selected package layout.
- The available source repository and exact commit or tag reviewed. Say when source is unavailable or when the published binary was not reproduced from it.
- What was inspected, which payloads are included or excluded, dependencies and any bundled runtime conflicts.
- The Starframe/game builds tested, observed results, limitations and the owner's approval evidence.

The existing [Ladder Reporter](verification/ladder-reporter-catalog.md) and [Remmy release reviews](verification/remmy-catalog.md) retain their original scope and limits. Approval of one artifact does not approve another archive with the same version label. Changed bytes require a new release identity and review. CI checks format and retained identity; it cannot replace curator review.

## Reports

Use the [mod problem form](https://github.com/Mastervoliumpl/Starframe/issues/new?template=mod-report.yml) for compatibility, download, installation or registry mistakes. Include the exact release ID and SHA-256 from Package details, app/game versions, reproduction steps and relevant evidence. Public reports must omit usernames, personal paths, credentials and private logs. Do not upload mod binaries.

Use [GitHub private security reporting](https://github.com/Mastervoliumpl/Starframe/security/advisories/new) for suspected malicious mods or vulnerabilities. Private reporting was enabled for this repository on 9 September 2026. Explain the affected artifact, evidence, impact and reproduction steps without including live credentials. Reports go to repository maintainers; no response-time guarantee is offered. Do not publish exploit details in an ordinary issue while reporting privately.

## Findings and corrections

Triage maintenance, compatibility and ordinary withdrawal separately from security decisions. The website signs exact archive-hash decisions as `blocked` or `cleared`, with revisions and reasons. The manager retains the highest accepted decision and rejects rollback or same-revision changes. An omitted decision cannot clear a block. Historical payload-hash advisories are retired.

Starframe retains files, settings and collection intent. These controls do not undo code already executed, stop a running game or prevent a launch outside Starframe. An offline client enforces retained blocks but cannot learn a new decision until reconnecting. New downloads require fresh signed registry metadata; unblocked installed copies remain usable offline.
