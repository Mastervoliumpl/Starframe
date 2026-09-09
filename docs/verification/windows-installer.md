# Windows installer development

Issue [#27](https://github.com/Mastervoliumpl/Starframe/issues/27) is in progress on the 0.6.0 branch. The local `0.6.0-dev.1` candidate has no Windows publisher signature and has not been published. These checks establish the stated behavior, not public-alpha acceptance.

## Automatic lifecycle

DESIGN.md revision 0.10 supersedes the first candidate's separate game cleanup and keep-data default. Choosing a validated game location starts runtime preparation automatically when the game is closed. Each desktop start checks and prepares its bundled runtime, including new runtime bytes at an unchanged collection revision. Saved collection changes still apply after game exit.

Settings offers **Repair or reinstall runtime**. Missing owned files can be recreated. Changed files are retained with a persistent failure message; repair does not adopt or overwrite unknown content. Runtime preparation uses the existing stopped-game guards, ownership journal and recovery code. App close stops work at its safe boundary.

The NSIS uninstaller calls the installed executable's finite maintenance command before deleting the app. It processes all nonempty deployment records, including previously selected game installations. A running game, unavailable installation or external-plugin conflict fails removal and preserves the app and recovery data for retry. There is no requirement to visit Settings first. The command creates no webview, starts no catalog/source watcher and exits when cleanup finishes.

Full uninstall deletes managed library copies, collections/app settings, caches and legacy app-data files by default. The unchecked **Keep my library, collections and settings** option preserves data; silent uninstall supports `/KEEPDATA`. Install, upgrade and reinstall preserve data. Original import sources, external plugins, game saves, unowned game configuration and unexpected top-level data files remain. A tracked original source inside the app-data directory prevents deletion; keep the data or relocate/reimport that source first. Directory junctions/reparse points are rejected before data removal. File errors stop removal rather than trigger an unchecked recursive delete.

The [template](../../src-tauri/windows/installer.nsi) is based on Tauri CLI 2.11.4. Its two changes label explicit retention and mark replacement of another app version as an update so the old uninstaller preserves data. [Hooks](../../src-tauri/windows/installer-hooks.nsh) invoke cleanup and replace Tauri's force-termination macro. Setup refuses a running Starframe process even in silent/passive mode. The packaging preflight requires template review when the CLI changes. The upstream [MIT notice](../notices/Tauri-MIT.txt) is included.

## Local build

Use the pinned tools in [DEVELOPMENT.md](../../DEVELOPMENT.md). Build the Unity entry plugin with compile-only references using the [runtime instructions](runtime-activation.md). Never substitute fixture DLLs in an app candidate. With the verified bootstrap ZIP available locally:

```powershell
python scripts/prepare_desktop_runtime.py src-tauri/target/installer/integration --runtime runtime/Starframe.Bootstrap/bin/Release/netstandard2.1 --archive "$env:STARFRAME_BOOTSTRAP_ARCHIVE"
python scripts/check_installer.py
npm run tauri -- build --config src-tauri/tauri.nsis.conf.json -- --locked
```

Preparation requires a new staging directory. Preserve older staging under ignored build output before rebuilding. The [preflight](../../scripts/check_installer.py) checks exact runtime membership, hashes, limits, empty activation and rejection of links/unexpected files. It is not a signature, license review or protection against a hostile build host. The package includes 35 prepared integration files and existing notices. Redistribution review remains incomplete.

## Verification on 9 September 2026

- Rust formatting, Clippy and regression tests passed, including explicit retention/default deletion, active storage refusal, unavailable recorded game and junction protection.
- Frontend formatting, lint, Svelte/TypeScript, 10 Vitest tests and the production build passed. All 20 browser tests passed.
- The [native lifecycle fixture](../../tests/native/cleanup.mjs) passed automatic first preparation, packaged runtime replacement at the same saved revision, missing-file repair, changed-file refusal, running-game protection, cleanup of two recorded installations, external-plugin refusal/retry, explicit retention and default data deletion. Game settings and original sources survived. All game paths were synthetic; no real game was launched or changed.
- The release executable and NSIS candidate built. The [ordinary-user installer fixture](../../scripts/test_windows_installer.ps1) passed install, metadata upgrade, installed runtime hashes, running-app refusal, explicit data retention, reinstall over retained data, default data deletion and removal of app files/registration. Unowned installation content survived. Its separate product/data identity is `Starframe Installer Test`; it never launches the desktop UI or a game.

The installer fixture changes NSIS metadata around the same executable. It does not prove migration between released app/database versions. It invokes the same executable's maintenance command against the separate fixture data identity. Evidence stays under ignored `test-results/0.6.0-lifecycle` and `test-results/0.6.0-packaging`. Native screenshots/logs may contain private paths; do not publish them unchanged.

### Reinstall and cleanup recovery

The extended ordinary-user NSIS fixture passed these additional checks:

- Same-version reinstall restored a deliberately damaged desktop executable and a deleted packaged runtime DLL to their original hashes. Managed data, unowned installation files and the recorded game deployment survived.
- Uninstall with an unavailable recorded game folder returned failure and retained the executable, Windows uninstall registration and SQLite database. Restoring the fixture folder allowed cleanup to proceed.
- A read-only shared handle allowed hashing an owned loader but prevented its deletion. Cleanup restored the earlier removed file, retained ownership and left the app/data installed. Releasing the handle allowed retry.
- A seeded committed uninstall journal with its first owned file already removed recovered through the installed uninstaller. Both owned files were removed; unowned game settings and a save sentinel survived. This reproduces an interruption state, not an actual process termination.
- A locked managed backup stopped data removal after the artifact directory had been deleted. The app, registration and database remained. Releasing the handle and repeating uninstall completed deletion without losing unowned files.

The [fixture helper](../../scripts/installer_game_fixture.py) creates inert game-layout files and a synthetic ownership record for two files in the separate installer-test database. It accepts only an installer evidence directory. It neither launches a game nor reads the owner's game installation. These checks exercise silent NSIS execution; the interactive maintenance page remains unverified.

## Remaining acceptance

Complete the transitive desktop/runtime/bootstrap notice and source inventory and review redistribution terms. Verify absent WebView2, prerequisite download failure, an actual packaged-version upgrade with populated data, and interactive installer/uninstaller keyboard, scaling and high-contrast behavior. Test actual process termination during installer cleanup/data removal in addition to the seeded journal and file-error retry checks above. A single PC does not establish the supported Windows matrix.

Windows Authenticode is deferred to [#61](https://github.com/Mastervoliumpl/Starframe/issues/61), with no milestone. Installer/update artifact signatures and catalog authentication remain required under #28, #29 and #46. [SignPath form notes](../planning/signpath-request.md) are retained for a future application; no application or consent has been submitted. No app release is authorized by this verification, and #27 remains open.
