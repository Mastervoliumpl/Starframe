# Dependency checks

The separate `Dependency security` workflow audits npm, Cargo and NuGet dependencies in parallel. It runs on dependency/workflow changes, weekly and on manual dispatch. It does not compile Starframe or change ordinary local build commands. Cargo's pinned audit tool is cached; a cold tool build has a 15-minute job limit. No scan automatically updates packages.

npm audits the lockfile without installation/scripts and fails at any reported vulnerability severity. Cargo uses the current RustSec database and fails on vulnerability advisories; informational/unmaintained/unsound warnings remain visible for review. NuGet audits direct and transitive packages during a locked restore with warnings treated as errors, including failed advisory lookups. All jobs must be reviewed before distribution; a successful application build alone is insufficient.

Local checks on 7 September 2026:

- npm audit: no reported vulnerabilities.
- NuGet locked restore with auditing enabled for all dependencies: passed without warnings; no application build.
- cargo-audit 0.22.2: no vulnerability advisories; 17 warnings, including GTK3 maintenance/GLib unsoundness and unmaintained `unic-*` packages. These warnings are not a clean security certification. Track the Tauri dependency chain and recheck before public distribution; no advisory IDs are suppressed by the workflow.

Targeted `cargo tree --target x86_64-pc-windows-msvc` inspection found no GLib dependency in the Windows graph. `unic-ucd-ident` is present through `urlpattern` and `tauri-utils`; its maintenance warning remains visible, with upstream replacement to be reviewed before public distribution or by 7 December 2026, whichever comes first. No speculative replacement of Tauri internals is part of 0.3.0.

The first local Cargo run could not refresh its optional crates.io index because Cargo was absent from that shell's PATH; the RustSec advisory database was fetched and scanned. The CI toolchain places Cargo on PATH. Native Windows and Linux-hosted CI are separate environments; the new remote workflow has not run until the branch is pushed.

Broader fuzzing is tracked in [#47](https://github.com/Mastervoliumpl/Starframe/issues/47), with [bounded campaign commands and limits](fuzzing.md). Catalog signing/advisory delivery is [#46](https://github.com/Mastervoliumpl/Starframe/issues/46). See [security scope](../../SECURITY.md) for release gates and review policy.
