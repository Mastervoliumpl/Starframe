# Curated release catalog

`releases.json` contains metadata only. The initial revision is empty because no external release has been approved for this catalog. Test releases use `example.invalid` and exist only in automated tests.

After a maintainer merges catalog changes to `main`, clients read [the catalog endpoint](https://raw.githubusercontent.com/Mastervoliumpl/Starframe/main/catalog/releases.json). GitHub/CDN caching can delay visibility. Catalog edits increase `catalogRevision` without changing VERSION or publishing an app release. Mod archives stay at author-controlled download locations.

## Schema 1

The Rust [catalog validator](../src-tauri/src/catalog.rs) defines the accepted schema. It rejects unknown or duplicate JSON fields, missing fields, unsupported layouts and documents over 2 MiB.

| Field | Contract |
| --- | --- |
| `schemaVersion` | Integer `1`. A different format needs explicit app support. |
| `catalogRevision` | Positive unsigned 64-bit integer encoded as a decimal string without leading zeros. Increase it whenever metadata changes. |
| `mods` | At most 1,024 mod records and 4,096 releases in total. |
| Mod `id` | Stable ID, 1–128 lowercase ASCII letters, digits, dots, underscores or hyphens. Start with a letter or digit. |
| Mod `name`, `author` | Nonblank display text, each at most 200 UTF-8 bytes, without control characters. |
| Mod `sourceUrl` | HTTPS author/source page. No credentials or fragment. At most 2,048 bytes. |
| Mod `releases` | 1–256 release records. |
| Release `id` | Globally unique stable ID, with the same spelling rules as mod IDs. |
| Release `version` | Author's display label, at most 128 bytes. It need not be SemVer. |
| Release `withdrawn` | Boolean. Withdrawn identities remain in the catalog. New downloads of them or releases that require them are blocked. |
| Release `artifact` | Exact `url`, lowercase SHA-256 `sha256`, integer `sizeBytes` (1 byte–2 GiB), and `layout`. The hash identifies the reviewed archive bytes. |
| Release `requires` | Up to 64 exact release IDs. References must exist. Reject duplicates, self dependencies, cycles and multiple direct requirements for the same mod. |
| Release `testedGameBuilds` | Up to 64 distinct observed build labels, each at most 128 bytes. An empty list means no recorded test evidence. These labels do not impose enable/launch restrictions. |
| Layout `kind` | Only `starframe_managed_zip` is accepted initially. Content overlays and conventional BepInEx plugins need separately verified package support. |
| Layout `root` | Relative archive folder, or an empty string for the archive root. |
| Layout `entryAssembly`, `entryType` | Relative `.dll` path within that root and a dotted managed type name implementing the supported runtime lifecycle. |

Paths reuse the runtime's Windows alias/device/path checks. Artifact URLs have the same HTTPS and length rules as source URLs. This issue validates declarations only; archive inspection, hash verification and extraction belong to #17. Catalog validation never loads DLLs or runs installation scripts.

## Review and publication

1. Review the exact author release, its archive layout, dependencies and redistribution/source links. Approval does not establish that code is free of malware.
2. Add a new stable release ID for changed bytes, version labels, package layouts or dependency requirements. Existing identities cannot be reassigned to another mod. A mirror URL may change only for identical reviewed bytes.
3. Retain every previous release. Set `withdrawn` to `true` to stop new downloads. Withdrawal is permanent for that release ID; a new approval needs a new ID.
4. Increase `catalogRevision`. Names, author/source links, tested-build evidence and identical-byte mirror URLs can change with a new revision.
5. Run `cargo run --manifest-path src-tauri/Cargo.toml --locked --bin check_catalog -- catalog/releases.json --git-base main`. The required Windows job runs the same validator against the PR base or previous main revision.
6. Merge only after the review and required checks. The raw GitHub file is the publication; no binary hosting or app update is involved.

The desktop accepts a fixed maintainer-controlled HTTPS endpoint. It rejects redirects. It sends ETag validators, or Last-Modified when ETag is absent. A 304 response requires a validated cache. Successful checks repeat after five minutes while the desktop remains open; failures back off from 30 seconds to 30 minutes. Numeric Retry-After values can extend the wait up to one hour. Each transfer has a 5-second connection timeout, 20-second total timeout and 2 MiB body limit, including responses without Content-Length.

Network work starts after the shell subscribes to desktop state. Requests do not overlap and do not hold the database owner. Cached metadata appears before a network reply. The existing database worker validates and commits a replacement in one SQLite transaction, then publishes its revision/status. Reopening the app checks immediately; resuming after a due time starts one check, without replaying missed intervals. Closing the app aborts the pending task. It creates no service or background updater.

SQLite schema 6 adds a single cache record containing the catalog, HTTP validators, last attempt/success times and error. Existing backup/migration handling applies. Malformed responses, older revisions, changed identities and storage failures retain the previous validated catalog. Catalog writes do not modify library or collection records. A corrupt or unsupported local cache is reported for recovery instead of silently resetting its identity history.
