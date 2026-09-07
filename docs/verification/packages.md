# Package preparation for issue #17

Implemented on `codex/0.3.0-curated-mods` at `0.3.0-dev.1`. The package module prepares approved artifacts in app data. Game deployment is issue #18; the Catalog, My mods and Downloads controls are issue #19.

## Command and storage ownership

The local main window can call `package_action` with `prepare`, `cancel` or `list`. Prepare accepts a request UUID and catalog release ID. It cannot accept a URL, archive path, destination or executable command. Rust resolves the release from the validated catalog cache and checks its complete dependency graph for missing, conflicting or withdrawn releases. Each request prepares one artifact; callers must submit any required releases separately. Catalog approval is checked again before completion commits.

The existing game/storage worker owns the queue and SQLite connection. Up to three package workers can transfer or extract concurrently. A fourth request receives a busy result. Duplicate request IDs return the existing operation, and simultaneous requests for the same artifact cannot start another worker. Network work uses async I/O; directory checks, extraction and file verification use bounded blocking workers. `list` returns the latest 100 saved operations plus any older active operations, with current byte progress. It does not change the existing diagnostic operation list.

Schema 7 adds package operation history and prepared file manifests. The existing backup-before-migration procedure retains schema-6 records. A completed transaction saves the file manifest, library entry and terminal operation together. It preserves collection membership and order. Failed commits cannot create a partial library record. Operation failures remain in SQLite across restarts.

## Transfer and archive limits

| Check | Implemented policy |
| --- | --- |
| Source | Approved HTTPS URL; no request credentials or archive-supplied download instructions |
| Redirects | At most five per attempt; HTTPS only, without credentials or fragments; author asset hosts may differ |
| Timeouts | Five-second connection, 20-second read and five-minute total transfer budget, including redirects and retries |
| Retries | At most three attempts for transport failures, interrupted bodies, HTTP 429 or 5xx; restart from byte zero; one/two-second backoff |
| Retry-After | Numeric values up to 30 seconds are observed within the total budget; longer values return a retry-later failure |
| Download | Exact catalog size, at most 2 GiB; reject transformed HTTP content and hash mismatches |
| Integrity | SHA-256 during transfer and again from the staged file before extraction; ZIP CRC and expanded-size checks while reading |
| ZIP structure | Ordinary single-disk ZIP with Stored or Deflate entries; at most 4,096 entries and 4 MiB of central-directory metadata |
| Expansion | At most 512 MiB per file and 2 GiB total; enforce both declared and observed sizes |
| Names | Existing runtime path rules: ASCII relative paths, at most 240 bytes; no traversal, backslashes, device names, alternate streams or trailing dots/spaces |
| Collisions | Reject duplicate files, case aliases, inconsistent directory casing, file/directory conflicts and overlapping ZIP entry data |
| Links | Reject link/special-file modes, reparse attributes and Unix link metadata; reject filesystem junctions and reparse points |
| Supported layout | Catalog schema 1 `starframe_managed_zip`, with the exact nonempty entry DLL at the approved root |

ZIP64, split archives, encrypted entries and other compression formats receive an unsupported result. The desktop never loads DLLs or runs archive scripts. The curator supplies the managed entry type and compatibility decision; checking that the entry file exists is not a test of arbitrary DLL compatibility. Content-only packages and conventional BepInEx plugins still require their own verified adapters.

The ZIP parser is pinned to `zip` 8.6.0 with its optional formats disabled. Deflate uses the existing `flate2` 1.1.10 dependency with its Rust backend enabled. The [ZIP API](https://docs.rs/zip/latest/zip/) documents the reader and feature selection. The selected version is above the affected range in [RUSTSEC-2025-0168](https://rustsec.org/advisories/RUSTSEC-2025-0168.html). Starframe reads entries individually and applies its own path checks rather than using the archive's general extraction method.

## File ownership and recovery

Each operation uses `package-staging/<operation UUID>` under app data. Windows directory handles prevent parent paths from being renamed during preparation. New output files use exclusive creation. Verification opens ordinary files without following reparse points and denies concurrent writes/deletes while reading.

After verification, the payload directory moves to `artifacts/<approved archive SHA-256>`. The manifest records each extracted file's path, size and SHA-256. Existing content is verified before reuse and is never overwritten. A modified, missing or unexpected file produces a repair error. Package preparation takes no game destination and never calls game deployment.

Normal completion, failure and cancellation remove their private staging directory after checking its paths. Cleanup refuses links or unexpected path types and reports retained staging if it cannot finish. Cancellation interrupts network waits and is checked between hashing/extraction blocks and before the library commit. It does not remove an already completed library entry.

An abrupt process exit can leave partial staging or fully promoted files without a database commit. Startup marks unfinished operation records as failed and retains that evidence. A new request restarts the transfer; if an unrecorded final directory exists, its complete file set must match the newly verified artifact before it can be adopted. There is no automatic retry on startup. These checks cover process interruption; they do not establish durability through hardware failure or sudden power loss.

## Verification

The Rust package fixtures use inert bytes and loopback HTTP. They cover safe and malicious ZIPs, changed hashes, interrupted transfers, cancellation, the three-worker bound, Windows junctions, database rollback and restart after promotion. The native test seeds a verified-content fixture and exercises real Tauri command permissions, cached reuse, duplicate requests and persistent failure. It does not claim a live author download or game activation.

Run the focused tests with `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib packages`. Run all Rust checks as specified in [DEVELOPMENT.md](../../DEVELOPMENT.md). The native check is `node tests/native/packages.mjs` after building the Windows debug executable; it also runs through `npm run test:native`.

Verified on Windows on 7 September 2026:

- PASS: 62 Rust tests, including 12 package tests. Four ignored process-worker entry points run through their parent recovery tests. Cancellation retains actionable cleanup failures.
- PASS: Rustfmt, Clippy across all targets with warnings denied, and Windows Tauri debug/release builds with locked dependencies.
- PASS: the complete native suite covers desktop state, SQLite conversion/recovery, game observation, catalog caching and package commands. All files and databases used by these checks are temporary fixtures.
- PASS: frontend formatting, ESLint, Svelte/TypeScript checks, six Vitest tests and the production build.
- PASS: 18 Python tests, repository/version/link checks and whitespace validation.

Issue #17 is complete locally. The branch has not been pushed, so remote CI has not run for these changes. No catalog release, app release or installer has been published.
