# Windows installer development

Issue [#27](https://github.com/Mastervoliumpl/Starframe/issues/27) has passed its functional checks on the 0.6.0 branch. The local `0.6.0-dev.1` candidate has no Windows publisher signature and has not been published. These checks establish the stated behavior, not public-alpha acceptance.

The owner accepted the later hosted `0.6.0-dev.2` installer on 17 September 2026. The [consolidated milestone record](milestone-0.6.0.md) supersedes the historical pending #30 checks below, records the exact installer hash, and distinguishes reused evidence from waived checks. No public application release has been published.

## Automatic lifecycle

DESIGN.md revision 0.10 supersedes the first candidate's separate game cleanup and keep-data default. Choosing a validated game location starts runtime preparation automatically when the game is closed. Each desktop start checks and prepares its bundled runtime, including new runtime bytes at an unchanged collection revision. Saved collection changes still apply after game exit.

Settings offers **Repair or reinstall runtime**. Missing owned files can be recreated. Changed files are retained with a persistent failure message; repair does not adopt or overwrite unknown content. Runtime preparation uses the existing stopped-game guards, ownership journal and recovery code. App close stops work at its safe boundary.

The NSIS uninstaller calls the installed executable's finite maintenance command before deleting the app. It processes all nonempty deployment records, including previously selected game installations. A running game, unavailable installation or external-plugin conflict fails removal and preserves the app and recovery data for retry. There is no requirement to visit Settings first. The command creates no webview, starts no catalog/source watcher and exits when cleanup finishes.

Full uninstall deletes managed library copies, collections/app settings, caches and legacy app-data files by default. The unchecked **Keep my library, collections and settings** option preserves data; silent uninstall supports `/KEEPDATA`. Install, upgrade and reinstall preserve data. Original import sources, external plugins, game saves, unowned game configuration and unexpected top-level data files remain. A tracked original source inside the app-data directory prevents deletion; keep the data or relocate/reimport that source first. Directory junctions/reparse points are rejected before data removal. File errors stop removal rather than trigger an unchecked recursive delete.

The [template](../../src-tauri/windows/installer.nsi) is based on Tauri CLI 2.11.4. Starframe's changes label explicit retention, mark replacement of another app version as an update so the old uninstaller preserves data, and exit setup after successful same-version uninstall. [Hooks](../../src-tauri/windows/installer-hooks.nsh) invoke cleanup and replace Tauri's force-termination macro. Setup refuses a running Starframe process even in silent/passive mode. The packaging preflight requires template review when the CLI changes. The upstream [MIT notice](../notices/Tauri-MIT.txt) is included.

## Local build

Use the pinned tools in [DEVELOPMENT.md](../../DEVELOPMENT.md). Build the Unity entry plugin with compile-only references using the [runtime instructions](runtime-activation.md). Never substitute fixture DLLs in an app candidate. With the verified bootstrap ZIP available locally:

```powershell
python scripts/prepare_desktop_runtime.py src-tauri/target/installer/integration --runtime runtime/Starframe.Bootstrap/bin/Release/netstandard2.1 --archive "$env:STARFRAME_BOOTSTRAP_ARCHIVE"
python scripts/check_installer.py
npm run tauri -- build --config src-tauri/tauri.nsis.conf.json -- --locked
```

Preparation requires a new staging directory. Preserve older staging under ignored build output before rebuilding. The [preflight](../../scripts/check_installer.py) checks exact runtime membership, hashes, limits, empty activation and rejection of links/unexpected files. It also checks dependency notice inputs, text and third-party DLL identities against the [reviewed inventories](distribution-notices.md). It is not a signature or protection against a hostile build host. The package includes 35 integration files and the desktop, runtime, installer and Rust standard-library notices. Corresponding source delivery remains a release-review requirement.

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

The [fixture helper](../../scripts/installer_game_fixture.py) creates inert game-layout files and synthetic ownership records in the separate installer-test database. It accepts only an installer evidence directory. It neither launches a game nor reads the owner's game installation. These checks exercise silent NSIS execution; the separate interactive checks are recorded below.

### Actual process interruption

The current release executable and NSIS package built from the #46 implementation passed the extended ordinary-user installer fixture on 9 September 2026. The helper now creates 8,192 disposable files to make partial removal observable, starts only the isolated NSIS uninstaller, and terminates that process tree after deletion begins. It requires some files to have been removed and others to remain; missing the interruption window fails the test.

- Game cleanup stopped after 9 of the additional files were deleted. The committed pending ownership journal, remaining files, installed app, database and uninstall registration survived. Retrying the uninstaller recovered the journal and removed the owned deployment. Game settings, the save sentinel and unowned installation content survived.
- Data removal stopped after 460 of the additional backup files were deleted. The installed app, database and uninstall registration survived. Retrying completed default data deletion and removed the app/registration while retaining unowned content.

The helper no longer seeds an interrupted journal for this check; the installed executable writes it during the real operation. The same run also passed damaged same-version reinstall, unavailable-root retry, locked-file rollback, running-app refusal, retained-data reinstall, explicit retention and default deletion. All 27 Python checks passed. Evidence remains under ignored `test-results/0.6.0-packaging`; temporary files and logs are not committed.

## Cross-build data migration

The optional `-BaselineExecutable` argument to [the installer fixture](../../scripts/test_windows_installer.ps1) packages two different application builds. Its [populated-data helper](../../scripts/installer_upgrade_fixture.py) verifies the installed executable bytes, allowing only Tauri's known NSIS bundle-marker replacement, and compares retained records before and after migration.

The baseline used on 15 September 2026 was built from `dfad58bcd854cbe796e0fc5f988be5ca6d816cd4`, whose storage schema is 12. An ignored source archive was synchronized to fixture version `0.6.0-dev.0`, its frontend was rebuilt, and its release executable was compiled with the locked dependencies and `tauri/custom-protocol`. The current branch's executable is `0.6.0-dev.1` with schema 13 and patched rustls 0.23.45. The baseline uses the current fixture's installer template and integration resources: this test isolates executable replacement and app-data migration, rather than claiming an upgrade between published installers or different runtime resources.

```powershell
./scripts/test_windows_installer.ps1 -BaselineExecutable "$env:STARFRAME_BASELINE_EXE"
```

The older installed executable creates schema 12, then opens a populated fixture containing two exact catalog/local references over shared content, two collections, a deliberately ordered active collection and saved revision. The new installed executable migrates that database to schema 13, preserves all existing table records, adds an empty security table without granting trust, and retains a complete schema-12 backup. A second invocation verifies reopening the migrated data. Managed artifact bytes, settings/backup sentinels and the original source fixture are checked separately. These finite maintenance invocations have no game deployment to remove and use only the installer-test identity; they do not launch a desktop window or touch a real game.

The complete installer fixture passed with these two builds. It also passed damaged-file reinstall, retained-data reinstall, unavailable-game retry, locked-file rollback, running-app refusal, explicit retention and default deletion. Actual process termination interrupted game cleanup after 9 of 8,192 disposable files and data removal after 510 of 8,192 files; both retries completed with unowned content intact.

### Installed release desktop after migration

A separate release build using the `Starframe Installer Test` product and `.installer-test` application identifier passed desktop startup from populated schema-12 data and a second launch after migration to schema 13. This used the production frontend and Rust release code; the identity override isolated the Windows registration and data directory. The older baseline installer created the fixture installation, and the new installer replaced it. Installed executable bytes matched the fixture build, allowing only Tauri's NSIS marker. No game location or deployment was saved.

On both launches, Settings displayed two library entries and two collections; Collections displayed both saved names, and My mods displayed both exact fixture mods. Navigation worked and normal window closure exited successfully. After closure, the existing migration helper verified every original table record, active collection and entry order, revision, complete schema-12 backup, and unchanged artifact/source bytes. The authenticated catalog table remained empty. The first test attempt needed a more specific mod-button selector because a name also appeared in the priority panel; a fresh schema-12 fixture then passed both launches.

The WebView2 debugging port was enabled only in the fixture process environment to inspect the installed desktop. No production debugging option was added. Release-build configuration, acceptance script, executable/installer hashes and screenshots remain under ignored `test-results/0.6.0-packaging/install-desktop-20260915`. The normal executable was preserved and restored after the fixture build. The fixture app and managed data were subsequently uninstalled, with its original source and evidence retained. This closes the desktop-first-start gap left by finite maintenance tests; final candidate acceptance remains in #30.

## Interactive keyboard checks

On 15 September 2026, the isolated installer passed keyboard cancellation from the welcome page and installation through the license, destination and completion pages. The destination was changed to an ignored fixture directory. Both completion checkboxes were cleared with Space/Tab; finishing did not launch Starframe or create a desktop shortcut. Windows registration pointed to the fixture installation.

Starting setup again displayed **Already Installed**, with **Add/Reinstall components** selected and **Uninstall Starframe Installer Test** available. The Down key selected uninstall, and Enter started the installed uninstaller. Computer Use could not access that application. The owner subsequently confirmed that retention was unchecked by default and that keyboard navigation with Tab/arrows worked. Filesystem and registry checks confirmed removal of the test app, managed data and uninstall registration. The owner also confirmed a bug: after uninstall, setup returned to installation pages. The correction and regression checks are recorded below.

These observations cover the current display settings only. The accessibility provider reported focus on the outer dialog even when screenshots showed focus on a button, radio option or checkbox; they do not establish screen-reader acceptance. No real game or personal Starframe data was used.

### Maintenance uninstall correction

After successful same-version removal, the setup callback now exits instead of advancing to installation pages. Add/reinstall and version replacement continue normally; the existing cancellation and failure guards run before the new exit. MSI replacement retains its existing remove/reinstall path.

The [compiled NSIS regression](../../scripts/test_installer_maintenance.ps1) extracts and executes the production callback with fixture radio input and an inert child uninstaller. The uninstall case reproduced the unwanted continuation before the correction. All six cases pass with the correction: uninstall-only exit, same-version reinstall, upgrade, older-version replacement, updater mode and cancelled removal. It checks continuation, child execution, file retention and the replacement `/UPDATE` argument. It does not inspect rendered controls or screen-reader behavior.

The complete ordinary-user installer suite also passed with the schema-12 baseline and current schema-13 executable. This includes retained records and backups, damaged reinstall, unavailable-game retry, locked-file rollback, running-app refusal and both retention choices. Actual process interruption stopped game cleanup after 9 of 8,192 disposable files and data removal after 510 of 8,192 files; both retries completed. The regression runs as part of that suite.

## Installer branding review

The license page identifies Starframe's GNU AGPL v3 license. At the owner's request, its introduction omits the explanation about example names and years. Those entries belong to the standard license's final author-guide section; the repository's complete `LICENSE` text remains unchanged. Back and Next are separated by half the measured Next–Cancel gap, preserving button sizes and keyboard order.

Issue [#74](https://github.com/Mastervoliumpl/Starframe/issues/74) extends the existing NSIS installer with the accepted portrait artwork, Segoe UI body text, Bahnschrift headings with a system fallback, navy surfaces and orange headings/progress. A small Starframe-owned Win32 drawing helper gives the native buttons thin borders and orange keyboard focus; fields, group frames and divider lines also use thin borders. The title bar uses Windows dark styling. The license, destination field and details list use readable light text on navy. License URLs remain selectable plain text so RichEdit does not force blue links with insufficient contrast. No new installer framework, external plugin or redistributed Windows font was added.

The owner accepted the portrait and palette on 15 September 2026, then asked for the remaining controls to match. [Representative native screenshots](../design/installer/README.md) record the revised controls. Artwork provenance and hashes are in [ASSETS.md](../design/ASSETS.md#windows-installer-artwork); the installer carries the Sanctuary artwork notice. The native high-contrast path restores system colors/themes, but final high-contrast, screen-reader and scaling acceptance remains in #30.

The full ordinary-user installer regression passed with the first branded build and the schema-12 baseline: migration and retained records, damaged reinstall, unavailable-game retry, locked-file rollback, running-app refusal, explicit retention and default deletion. Actual interruptions stopped game cleanup after 8 of 8,192 disposable files and data removal after 439 files; both retries completed. The six compiled maintenance cases passed again after the subsequent control styling. Full NSIS compiler output showed no warnings after the layout correction; undefined NSIS symbols now fail compilation instead of producing a broken color/layout instruction.

The final control styling was checked through a separate `Starframe Installer Test` installation and same-version repair. Keyboard navigation, destination controls, dark details, orange progress, disabled navigation, completion-page focus and Space toggling were observed. Both optional completion checkboxes were cleared before exit; the app was not launched. The custom-page callback receives the newly created page handle, avoiding the previous page while NSIS transitions between pages. The uninstall introduction, directory and retention checkbox use separate dialog-unit rows so the larger font does not overlap them. The default remains unchecked.

The owner confirmed that the updated test uninstaller looked right overall, keyboard navigation worked, and the fixture was uninstalled. Follow-up checks confirmed that its executable, uninstall registration and managed data were absent. The subsequent thin-border refinement was checked on the native welcome, license and destination pages, including Tab focus on Browse and Enter navigation. The helper leaves control input, selection and accessibility identities with Windows and bypasses custom drawing in high-contrast mode. Common dialogs remain native.

On 16 September the owner accepted the installer except for radio and checkbox styling. The same drawing helper now paints those controls with thin borders and orange selection marks, including disabled and indeterminate states. A separate text focus indicator distinguishes focus from selection. An isolated NSIS control page verified the production helper with native state readback: arrow navigation changed the selected radio without selecting both, Tab reached the initially unchecked retention control, and Space changed its state from 0 to 1 and back to 0. A disabled checked option remained disabled. The final NSIS build, runtime/notices preflight and all six maintenance callback cases passed. The normal installer was rebuilt; its retention default and cleanup code were unchanged. Final scaling/high-contrast/screen-reader acceptance remains in #30.

The full installer regression passed again with the drawing helper: migration, repair, both data choices, running-app protection and recovery. Actual interruption occurred after 9 of 8,192 game files and 491 data files; retries completed. The final field/group refinement compiled with warnings treated as errors and was inspected in the packaged installer. The fixture uses only ignored test evidence and a separate product/data identity. No real game or personal Starframe data was changed. Logs and any screenshots containing local paths remain ignored.

The next review found uneven focused outlines and an intermittently missing license heading. The old build reproduced a blank title while accessibility still reported `License Agreement`. Focus no longer changes pen width or control geometry; push buttons also receive a text focus marker. The header backdrop now clips sibling controls and stays behind the title, and the title aligns with the subtitle/body margin. The corrected native installer kept its title on four license-page visits, including two returns through Welcome and a return from the destination page. Welcome layout and destination content were also checked. The packaged build and all six maintenance cases passed again. To repeat this regression check: visit License, return to Welcome and revisit License twice, then continue to the destination page and return to License; verify both the visible title and the unchanged control outlines as focus moves.

## Owner-reported offline VM acceptance

The owner supplied the test-kit baseline and reported the following manual results. The baseline identifies Windows 11 Education 25H2, build 26200.6584, x64, running without elevation. Node, Rust and the .NET CLI were absent. Starframe was not registered, and its executable, app data and database were absent. WebView2 140.0.3485.66 was registered in the machine's 32-bit registry view. The report's guest-clock timestamp is `2026-09-16T04:59:51.9746773Z`; clock synchronization was not independently checked.

The installer SHA-256 matches the supplied test kit: `668557ee6677a65c1c653e76e7a94f62d9036f857d186471b5bf03a46d572615`. This is the earlier unbranded candidate, before the maintenance correction above. The owner kept the guest offline; the baseline script performs no network probe.

- Installation succeeded with no unexpected prerequisite or security failure.
- The desktop opened, navigation worked and offline errors were understandable.
- Collections/preferences survived app restart and reinstall, and were retained when the uninstall retention option was selected.
- Running-app protection and both uninstall data choices behaved as described in the test instructions.
- The owner reported no unexpected elevation, missing files, stuck windows or other failures.

These are owner-observed results; only the pre-install baseline JSON was supplied. They verify this candidate on one fresh guest with WebView2 already present. They do not establish absent-WebView2 behavior, prerequisite acquisition, cross-version desktop migration, real-game integration, the supported Windows matrix or final branded-installer accessibility. The known maintenance-flow issue was described in the test instructions and is not waived by the absence of unexpected problems. The original report remains in ignored local evidence; personal paths are excluded from this record.

## Remaining acceptance

Owner amendment, 17 September 2026: the [consolidated 0.6.0 record](milestone-0.6.0.md) supersedes the earlier remaining-check list below. Additional scaling/high-contrast/screen-reader tests and absent-WebView2 execution are waived, not passed. Existing installer, keyboard and offline VM results carry forward. Inspection confirmed that download failure and nonzero prerequisite-install exit abort with the required-WebView2 explanation; no new prerequisite code was needed. The only new owner installer check is installation and startup of the already verified hosted dev.2 package. Reinstall, retention, cleanup and migration do not need another manual campaign.

On 15 September 2026, follow-up validation found that hosted checks on `3077204` had stopped after the legacy storage fixture's default five-second assertion expired. The captured UI still showed opening saved data, with the desktop connected. Legacy conversion now uses the same bounded thirty-second wait as the existing delayed-startup fixture. The native storage script passed fresh/restarted data, corrupt/newer data retention, populated legacy schemas 1–3 and navigation during delayed conversion. No application behavior changed. All nine native scripts subsequently passed in hosted run `34998438792` on `a308d92`; its release-build result is tracked separately in the PR.

The transitive notice/source inventory is recorded in [distribution notices](distribution-notices.md); verify required source delivery with #29 before publication. On 15 September 2026 the owner moved final clean-machine/prerequisite and display/accessibility acceptance to [#30](https://github.com/Mastervoliumpl/Starframe/issues/30), retaining them as mandatory public-release gates. This includes absent WebView2, prerequisite download failure, the supported Windows matrix, scaling, high contrast and screen-reader checks on the final branded installer from [#74](https://github.com/Mastervoliumpl/Starframe/issues/74). Display/accessibility tests may run on an existing PC. A disposable Windows environment is needed to verify installation without development dependencies or earlier app state; its actual WebView2 state must be checked separately.

The maintenance-uninstall correction and installed desktop first start after migration have now passed, completing #27's functional acceptance. The owner-reported VM results add fresh-install and offline-use evidence with WebView2 present. The transferred #30 checks remain open for the final release candidate.

Windows Authenticode is deferred to [#61](https://github.com/Mastervoliumpl/Starframe/issues/61), with no milestone. Installer/update artifact signatures and catalog authentication remain required under #28, #29 and #46. [SignPath form notes](../planning/signpath-request.md) are retained for a future application; no application or consent has been submitted. No app release is authorized by this verification.
