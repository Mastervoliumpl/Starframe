# Curated release catalog

`releases.json` contains metadata only. Revision 2 approves Remmy's Ladder Reporter 0.3.0 as the first catalog mod. It requires the BepInEx plugin support added in Starframe 0.5.0. See the [release review](../docs/verification/ladder-reporter-catalog.md). Test releases use `example.invalid` and exist only in automated tests.

After a maintainer merges catalog changes to `main`, clients read [the catalog endpoint](https://raw.githubusercontent.com/Mastervoliumpl/Starframe/main/catalog/releases.json). GitHub/CDN caching can delay visibility. Catalog edits increase `catalogRevision` without changing VERSION or publishing an app release. Mod archives stay at author-controlled download locations.

## Schemas 1 and 2

The Rust [catalog validator](../src-tauri/src/catalog.rs) defines the accepted schema. It rejects unknown or duplicate JSON fields, missing fields, unsupported layouts and documents over 2 MiB.

| Field | Contract |
| --- | --- |
| `schemaVersion` | Integer `1` or `2`. Schema 2 adds ordering constraints and Lua overlays. Older clients retain their last supported catalog. |
| `catalogRevision` | Positive unsigned 64-bit integer encoded as a decimal string without leading zeros. Increase it whenever metadata changes. |
| `mods` | At most 1,024 mod records and 4,096 releases in total. |
| Mod `id` | Stable ID, 1–128 lowercase ASCII letters, digits, dots, underscores or hyphens. Start with a letter or digit. |
| Mod `name`, `author` | Nonblank display text, each at most 200 UTF-8 bytes, without control characters. |
| Mod `sourceUrl` | HTTPS author/source page. No credentials or fragment. At most 2,048 bytes. |
| Mod `description`, `unmaintained` | Optional description (up to 4,000 bytes) and maintenance flag. Defaults are empty/false for earlier caches. Unmaintained mods remain usable. |
| Mod `releases` | 1–256 release records. |
| Release `id` | Globally unique stable ID, with the same spelling rules as mod IDs. |
| Release `version` | Author's display label, at most 128 bytes. It need not be SemVer. |
| Release `withdrawn` | Boolean. Withdrawn identities remain in the catalog. New downloads of them or releases that require them are blocked. |
| Release `withdrawalReason` | Optional nonblank reason, up to 2,000 bytes; allowed only when withdrawn. Supply it for new withdrawals. Earlier records without a reason display that omission explicitly. Ordinary withdrawal preserves installed copies/settings. |
| Release `compatibilityProblems` | Optional list of up to 64 findings with exact `gameBuild`, `note` (up to 2,000 bytes) and HTTPS `sourceUrl` evidence. A build cannot also appear in `testedGameBuilds`. Warnings allow enable/launch. |
| Release `artifact` | Exact `url`, lowercase SHA-256 `sha256`, integer `sizeBytes` (1 byte–2 GiB), and `layout`. The hash identifies the reviewed archive bytes. |
| Release `requires` | Up to 64 exact release IDs. References must exist. Reject duplicates, self dependencies, cycles and multiple direct requirements for the same mod. |
| Release `loadBefore`, `loadAfter` | Schema 2 optional arrays of up to 64 distinct other mod IDs each. Mandatory ordering applies when the target mod is enabled; these fields do not install or enable it. Combined cycles block deployment with the involved IDs. |
| Release `preferBefore`, `preferAfter` | Schema 2 optional arrays with the same bounds. These are warnings when the effective order differs. User priority and mandatory constraints take precedence; absent targets and optional cycles never block deployment. |
| Release `testedGameBuilds` | Up to 64 distinct observed build labels, each at most 128 bytes. An empty list means no recorded test evidence. These labels do not impose enable/launch restrictions. |
| Layout `kind` | `starframe_managed_zip`, or schema 2 `starframe_lua_zip`. Starframe 0.5.0 adds supported BepInEx 5 plugins; maps remain unsupported. |
| Layout `root` | Relative archive folder, or an empty string for the archive root. |
| Layout `entryAssembly`, `entryType` | Relative `.dll` path within that root and a dotted managed type name implementing the internal runtime lifecycle or a supported BepInEx 5 `BaseUnityPlugin`. |

`starframe_lua_zip` has no extra layout fields. Its archive contains only `.lua` files under `LJ/lua/`; every file retains that path in the managed payload. The runtime serves verified bytes through the game's cache after cache construction. Target directories must already exist in the game. Paths containing an `ai` component, map content, binaries and other file types are rejected. Managed DLL companion files are not automatically overlaid. Later effective order wins a case-insensitive path collision; Collections lists the participants and expected winner. See [ordering verification](../docs/verification/ordering.md) for the tested game build and remaining limits.

Ordering arrays default to empty for older caches. Changing ordering metadata on an existing release requires a new release ID, like changing its required dependencies. Revision 2 uses schema 1 because Ladder Reporter does not require schema 2 fields.

Managed archives retain all files. `root` locates the entry assembly; it does not filter the archive. Approve the author's mod-only archive when a standalone bundle includes BepInEx or another loader. Those bundled assemblies can conflict with Starframe's runtime even inside private package storage. Catalog metadata supplies the import identity, so authors do not need to include `starframe.local.json`. See [BepInEx plugin support and limits](../docs/bepinex-mods.md).

Paths reuse the runtime's Windows alias/device/path checks. Artifact URLs have the same HTTPS and length rules as source URLs. This issue validates declarations only; archive inspection, hash verification and extraction belong to #17. Catalog validation never loads DLLs or runs installation scripts.

## Review and publication

1. Review the exact author release, its archive layout, dependencies and redistribution/source links. Approval does not establish that code is free of malware.
2. Add a new stable release ID for changed bytes, version labels, package layouts or dependency requirements. Existing identities cannot be reassigned to another mod. A mirror URL may change only for identical reviewed bytes.
3. Retain every previous release. Set `withdrawn` to `true` and supply `withdrawalReason` to stop new downloads. One failed request does not withdraw a release. Withdrawal is permanent for that release ID; a new approval needs a new ID.
4. Increase `catalogRevision`. Names, author/source links, tested-build evidence and identical-byte mirror URLs can change with a new revision.
5. Run `cargo run --manifest-path src-tauri/Cargo.toml --locked --bin check_catalog -- catalog/releases.json --git-base main`. The required Windows job runs the same validator against the PR base or previous main revision.
6. Merge only after the review and required checks. The raw GitHub file is the publication; no binary hosting or app update is involved.

The desktop accepts a fixed maintainer-controlled HTTPS endpoint. It rejects redirects. It sends ETag validators, or Last-Modified when ETag is absent. A 304 response requires a validated cache. Successful checks repeat after five minutes while the desktop remains open; failures back off from 30 seconds to 30 minutes. Numeric Retry-After values can extend the wait up to one hour. Each transfer has a 5-second connection timeout, 20-second total timeout and 2 MiB body limit, including responses without Content-Length.

Network work starts after the shell subscribes to desktop state. Requests do not overlap and do not hold the database owner. Cached metadata appears before a network reply. The existing database worker validates and commits a replacement in one SQLite transaction, then publishes its revision/status. Reopening the app checks immediately; resuming after a due time starts one check, without replaying missed intervals. Closing the app aborts the pending task. It creates no service or background updater.

SQLite schema 6 adds a single cache record containing the catalog, HTTP validators, last attempt/success times and error. Existing backup/migration handling applies. Malformed responses, older revisions, changed identities and storage failures retain the previous validated catalog. Catalog writes do not modify library or collection records. A corrupt or unsupported local cache is reported for recovery instead of silently resetting its identity history.
