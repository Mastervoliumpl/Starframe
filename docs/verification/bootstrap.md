# Bootstrap deployment and recovery

Issue [#12](https://github.com/Mastervoliumpl/Starframe/issues/12), milestone 0.2.0. The deployment module prepares the pinned BepInEx loader and removes only its recorded files. The Starframe entry plugin, mod activation and game readiness remain #13/#15. This issue does not install mods or add desktop setup buttons.

## Package and operation boundary

[BepInEx 5.4.23.5 Windows x64](https://github.com/BepInEx/BepInEx/releases/tag/v5.4.23.5) is pinned by its official ZIP SHA-256 (`82f9878551030f54657792c0740d9d51a09500eeae1fba21106b0c441e6732c4`) and each file's hash/size in [bootstrap.json](../../runtime/bootstrap.json). The preparation script downloads or reads that exact archive, validates its full inventory, and extracts into a new directory. It does not execute or install it. Installation rechecks the required files against the compiled inventory; editing the prepared package cannot authorize a different bootstrap.

The deployed set contains 15 loader files plus the [upstream MIT notice](../notices/BepInEx-5.4.23.5.txt). XML documentation and the root changelog are not installed. The loader uses its shipped configuration. Mod payloads are not placed in its plugin scan directories by this operation. The Starframe entry plugin and supported payload locations will be added with activation in #13; no successful activation is reported here.

[deployment.rs](../../src-tauri/src/deployment.rs) plans file changes independently of mod format. DLL, Lua-only, map-only and mixed package support remain explicitly in the #13 investigation; AI packages are deferred. Bootstrap inputs are bounded to 512 files, 8 MiB per file and 32 MiB per payload. Larger content packages need an artifact-file backup strategy before using this machinery for their assets.

## Ownership and recovery

The existing single database owner now uses SQLite schema 5. Migration backs up schema 4 before adding deployment records and verified backup blobs. Legacy Turso conversion still produces the validated schema-4 intermediate, then follows the same migration. Previous library/collection records remain unchanged. A schema-5 database cannot be opened by 0.1.1; use retained backups when recovering older data rather than changing version markers.

A game-directory lock prevents concurrent Starframe operations, including operations using different app-data directories. Windows directory handles keep parent names stable during file work. Paths reject traversal, Windows aliases and reparse points. Only regular files are accepted. Unowned files with matching hashes are borrowed and never adopted; a different unowned file blocks the plan. Unexpected changes to owned files block update/removal and retain the files and backups.

Before changing active files, one SQLite transaction records the intended ownership change and verified original/new bytes. SQL remains in [storage.rs](../../src-tauri/src/storage.rs). Files are written to new sibling `.tmp` files, flushed, then renamed into place. Removal deletes only matching recorded files. Each target is checked again before mutation; the final ownership record commits only after verification. A failed operation rolls back where safe. Unknown content or a running/unknown game leaves recovery pending and reports the reason.

Recovery compares the current files against the recorded before/after hashes. It can repeat after another interruption. The desktop's existing background game worker retries pending recovery when the selected installation is first observed stopped, or transitions back to stopped. A discovery/selection request permits a new attempt. Failed recovery is shown through the existing game error state; it does not loop over writes every two seconds.

Removing the loader is refused while external DLLs remain under its plugin/patcher directories. Unknown plugin paths or excessive scan depth also block removal. Configurations, logs, unrelated files, empty directories, the small `.starframe-bootstrap.lock` metadata file and backup blobs remain. Interrupted temporary files are retained; recovery neither executes nor guesses ownership of them. Bootstrap plugin scans use DLL files, while these temporary files end in `.tmp`.

These checks reduce races with Steam or other tools; they do not make multiple filesystem changes atomic or prevent another program from starting the game between observations. Verification covers process interruption, not a physical power-loss experiment. Keep pending recovery and backups when a path cannot be verified. Deployment blobs are included in SQLite backups; general library artifact files still require separate preservation.

## Development commands

Close the desktop before using the command so its database owner can release the data-directory lock. Use an explicitly selected game installation and the same app-data directory used by the desktop. There is no unguarded command option to override a running or unknown game state.

```powershell
python scripts/prepare_bootstrap.py "<new-prepared-directory>"
cargo run --manifest-path src-tauri/Cargo.toml --locked --example bootstrap -- install "<app-data-directory>" "<game-installation>" "<prepared-directory>"
cargo run --manifest-path src-tauri/Cargo.toml --locked --example bootstrap -- recover "<app-data-directory>" "<game-installation>"
cargo run --manifest-path src-tauri/Cargo.toml --locked --example bootstrap -- remove "<app-data-directory>" "<game-installation>"
```

The preparation script accepts `--archive` for a cached copy; the same hash check applies. Keep prepared files outside the game until the guarded install operation applies them. These commands change game integration when pointed at a real installation. The verification below used isolated fixtures only.

## Verification on 6 September 2026

- Thirteen Rust deployment tests exercise install/update/removal, borrowed files, conflicts, backup restoration, game-state deferral, unknown recovery content, file locks, Windows junctions and the public recovery entry. Child processes exit without Rust cleanup at each persisted install/update/remove boundary and during rollback; parent tests reopen the database and recover.
- The full Rust suite passes 39 tests. Four ignored worker entry points are invoked by the interruption tests. Clippy runs with warnings denied. Two Python preparation tests bring the repository suite to 15 tests.
- The public bootstrap command installed all 16 files from the verified official package in a temporary game fixture, repeated installation, and removed them. Original fixture hashes matched afterward; only the lock metadata remained as an additional file. A hidden Windows ping process with the fixture executable name blocked installation before any loader file was written. The process was stopped before installation; no loader or Sanctuary code ran.
- Existing storage fixtures still exercise legacy conversion and restart recovery. Older-schema fixtures now remove schema-5 tables when constructing historical databases. The interrupted-backup test captures the interrupted folder before restart can create a second, complete migration backup.

The debug desktop build and native checks passed, including saved-data restart/conversion, responsive navigation, discovery and process start/exit. Required CI is recorded on the issue before closure. Desktop setup/removal controls and the full game smoke check remain milestone work. No personal game files were changed during this issue's verification.
