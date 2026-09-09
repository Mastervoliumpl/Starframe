# Windows installer development

Issue [#27](https://github.com/Mastervoliumpl/Starframe/issues/27) is in progress on the 0.6.0 branch. The local `0.6.0-dev.1` NSIS candidate is unsigned and has not been published. These checks establish packaging behavior, not public-alpha acceptance or redistribution approval.

## Implemented behavior

[The NSIS configuration](../../src-tauri/tauri.nsis.conf.json) extends the ordinary Tauri configuration only when selected. It uses current-user installation, Windows shortcuts/registration, the existing icon/version and Tauri's download-on-demand WebView2 prerequisite. Downgrades are disabled. Executable-only development and CI retain their existing build path. See [Tauri's installer documentation](https://v2.tauri.app/distribute/windows-installer/).

The package includes the prepared bootstrap/runtime under `integration`, Starframe's license and existing notices. [The packaging check](../../scripts/check_installer.py) verifies bootstrap hashes against the pinned manifest, runtime membership/hashes, size limits and empty activation. It rejects unexpected files, including game references, and links/junctions in staged resources. This is not a signature or redistribution check, or protection against a hostile build host replacing files concurrently.

[The hook](../../src-tauri/windows/installer-hooks.nsh) replaces Tauri's default process-termination macro. Setup refuses a running Starframe process, including silent/passive operation. It requires normal shutdown so file operations can reach their stopping point. Recheck this override when the Tauri CLI or NSIS helper changes.

Interactive uninstall explains that it removes the desktop app and leaves game integration unchanged. The user can cancel and use **Settings > Remove Starframe from game**, which calls existing ownership/recovery code. Game cleanup requires the game to be closed, preserves settings/source folders and reports conflicts for retry. The installer does not call game cleanup. App data stays by default; Tauri's explicit delete-app-data checkbox remains available in its interactive uninstaller.

## Local build

Use the pinned tools in [DEVELOPMENT.md](../../DEVELOPMENT.md). Build the Unity entry plugin with compile-only references using the [runtime instructions](runtime-activation.md). Never substitute fixture DLLs in an app candidate. From the repository root, with the verified bootstrap ZIP available locally:

```powershell
python scripts/prepare_desktop_runtime.py src-tauri/target/installer/integration --runtime runtime/Starframe.Bootstrap/bin/Release/netstandard2.1 --archive "$env:STARFRAME_BOOTSTRAP_ARCHIVE"
python scripts/check_installer.py
npm run tauri -- build --config src-tauri/tauri.nsis.conf.json -- --locked
```

Preparation requires a new staging directory. Preserve previous staging elsewhere under ignored build output before rebuilding it. Do not copy a whole game directory or build output into resources. The unsigned candidate is written beneath `src-tauri/target/release/bundle/nsis`; do not upload it as a release.

## Verification on 9 September 2026

- Built the release executable and NSIS installer with the current runtime. The entry-plugin build retained two previously recorded Unity assembly-resolution warnings, with no errors. Runtime staging contains 35 files. No installed game was launched or changed.
- Passed 25 Python tests, including altered bootstrap bytes, extra/missing dependencies, nonempty activation and substituted/duplicate inventory entries. Passed frontend formatting, lint, Svelte/TypeScript checks, 10 Vitest tests and all 20 browser tests.
- [The ordinary-user installer fixture](../../scripts/test_windows_installer.ps1) builds separate `Starframe Installer Test` packages with versions `0.6.0-dev.0` and `0.6.0-dev.1`. It verifies registration, installed runtime hashes, retained synthetic database/legacy/import/settings files, retained unowned installation content and app-only removal. It refuses an existing fixture identity or running Starframe instance and never launches the bundled app or a game. Fixture data/registration are separate from Starframe's real data.
- Installer and uninstaller refusal checks returned a failure while leaving a running process named `starframe.exe` alive and the installed executable/data intact. That process is a copied Windows ping fixture, not the desktop app. The uninstaller refusal test runs in place to observe its own exit code; normal removal exercises NSIS's temporary-copy launcher.
- [The native cleanup fixture](../../tests/native/cleanup.mjs) verifies the disabled cleanup action during an observed fixture game, refusal to remove a loader used by an external plugin, preserved conflicting content, successful retry, retained settings/source files and desktop restart. It is included in `npm run test:native`.
- The first hosted Windows check exposed an older native-test race: file inspection could observe an unfinished deployment. The test now waits for the serial worker's reply and matching saved/deployed revisions. The focused native mod test passed locally. This changes test synchronization, not deployment behavior.

The installer fixture changes NSIS metadata around the same executable; it does not prove application/database migration between released builds. Its cleanup removes only its synthetic data and registry entries. It retains build evidence under ignored `test-results/0.6.0-packaging` and leaves unexpected content for inspection.

## Remaining #27 acceptance

Public packaging still needs the complete transitive desktop/runtime/bootstrap notice and source inventory, reviewed redistribution terms, Authenticode signing and signature/timestamp verification. Existing notices are incomplete for distribution. The [SignPath eligibility request](../planning/signpath-request.md) is prepared for maintainer review and has not been submitted.

Still required: a clean machine without WebView2, prerequisite download failure, actual packaged-version upgrade with populated app data, and interactive installer/uninstaller keyboard, scaling and high-contrast review. The target remains Windows 11 x64; one PC does not establish the supported matrix. #27 remains open, and #28's prerequisite has not been declared complete.
