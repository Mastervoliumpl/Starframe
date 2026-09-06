# Runtime files, version 1

Issue [#11](https://github.com/Mastervoliumpl/Starframe/issues/11) implements the file boundary in [HANDOFF.md](../docs/HANDOFF.md). The runtime reads prepared files independently of the desktop database. These are internal formats; a public SDK has not been released.

[Activation](fixtures/activation.json), [capabilities](fixtures/capabilities.json) and [report](fixtures/report.json) are executable examples. Both readers consume every case in [cases.json](fixtures/cases.json). The top-level `schemaVersion` and `runtimeContractVersion` are JSON integers equal to 1; `integrationId` is `starframe.bepinex`. Unsupported versions, duplicate JSON properties, unknown/missing fields, null values, comments and trailing data are rejected. Product version is separate.

## Bounds and activation

Each UTF-8 document is at most 1,048,576 bytes with JSON depth at most 32. Inventory, activation, required dependencies and report outcomes each allow 256 entries. One mod allows 1,024 files; an activation allows 8,192 files total. String limits count UTF-8 bytes and reject control characters. IDs use lowercase ASCII letters, digits, dots, underscores and hyphens, start with a letter/digit and allow 128 bytes. Display names allow 256 bytes and versions 128; display text never authorizes loading and must be rendered as text.

Every active mod must have exactly one installed inventory entry. Inventory IDs and active IDs are unique. Disabled entries carry only `modId`, `name` and `version`; executable fields are rejected. Requirements are unique active IDs that appear earlier. Distinct active roots must not overlap, including Windows case aliases. Entry type names allow dot-separated ASCII C# identifiers; nested/generic type syntax is outside v1. Entry assemblies must end in `.dll` and occur in the immutable file inventory.

Paths are relative, forward-slash-separated ASCII paths of at most 240 bytes. Segments allow letters, digits, spaces, underscores, dots and hyphens. Empty/dot/parent segments, trailing dots/spaces, Windows device names, drive/UNC paths, alternate streams and short-name tildes are rejected. File paths must not collide or overlap under ASCII case folding. SHA-256 values are 64 lowercase hex characters.

These checks validate the document, not the filesystem. Deployment and activation must still resolve paths under their owned roots, reject reparse-point escapes, verify hashes and prevent assembly resolution outside the validated inventory. Callers must read files with the same byte limit before passing their contents to either reader. No validator call loads a DLL or executes a mod. These filesystem and lifecycle checks belong to #12 and #13.

`source.kind=catalog` has exactly `kind` and `releaseId`. `source.kind=local` has exactly `kind` and `contentId`. The local ID is `sha256:` followed by the hash of the canonical inventory bytes below. The reader verifies that ID against the declared inventory. It does not prove that the files on disk have those hashes.

## Canonical local inventory

Start with the UTF-8 bytes of `starframe-inventory-v1` followed by LF. For each file, append its ASCII-lowercased relative path, one NUL byte, its lowercase SHA-256 and LF. Sort these rows in byte order before appending. There is no BOM or CR. Hash the whole byte sequence with SHA-256. [Canonical bytes and expected ID](fixtures/canonical-inventory.json) and two differently ordered local activation fixtures are checked in both languages. Case aliases produce the same ID; duplicate aliases in one inventory are rejected. Content identity excludes mutable settings, display metadata and machine-specific paths.

## Capabilities and reports

Capabilities contain exactly `managedLifecycle`, `manualPriority`, `contentOverlays`, `settingsUi` and `activationReport`. Each has a boolean `supported` and a reason up to 512 bytes; unsupported capabilities require a nonempty reason. Fixture values do not advertise implemented game support.

Activation and report revisions are canonical unsigned decimal strings of at most 20 digits. Reports also carry a nonzero lowercase dashed GUID `gameSessionId`, a positive unsigned 32-bit `processId` and decimal `processStartFileTime` (Windows process creation FILETIME, 100 ns intervals since 1601 UTC). Report mod IDs are unique and outcomes are `loaded`, `failed` or `skipped_dependency`. Loaded entries have empty `errorCode`/`message`; other entries require both. Codes use lowercase ASCII letters, digits and underscores (letter first, 64 bytes maximum); messages allow 2,048 bytes. Skipped dependencies use `dependency_failed`.

A syntactically valid report does not establish readiness. The consumer must compare revision, the observed process ID/start time and the expected session, and match outcomes against the ordered activation list. Missing, stale, mismatched or unreadable reports mean unknown status. Atomic report replacement and process/session correlation are implemented with activation and launch in #13/#15.
