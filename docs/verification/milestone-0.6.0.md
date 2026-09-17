# Milestone 0.6.0 acceptance

Status: pre-merge work on 17 September 2026. Issues #27, #28, #29, #47 and #74 are complete. #30 awaits the owner's short hosted-installer smoke result; #46 awaits the main-only publisher's first live publication and renewal after an approved merge. No public app release, catalog publication or merge is authorized by this record.

## Accepted scope and reused evidence

The owner directed a benefit-versus-effort approach on 17 September. Reuse passed checks unless relevant code, dependencies, packaging or environment assumptions changed. A final milestone checklist is not a reason to repeat unchanged tests. Existing CI remains; no extra full manual campaign is required. This record supersedes broader historical acceptance lists in #30, the handoff and earlier verification documents.

| Area | Evidence retained | Remaining work |
| --- | --- | --- |
| Library, imports, collections, ordering, sharing, source watching | [0.4.1](milestone-0.4.1.md), [0.5.0](milestone-0.5.0.md), and current automated/native regressions | No duplicate walkthrough. |
| Game runtime, settings and real-mod use | [BepInEx acceptance](bepinex-plugins.md), [Ladder Reporter](ladder-reporter-catalog.md), original-game-file preservation and owner match report | Retain exact compatibility claims; do not infer gameplay support for untested mods. |
| Fresh Windows installation and offline use | [Owner VM results](windows-installer.md#owner-reported-offline-vm-acceptance), Windows 11 Education 25H2 x64 with WebView2 present | Short hosted-package smoke below; no repeat of retention/reinstall/offline campaign. |
| Installer lifecycle and branding | [Installer evidence](windows-installer.md): owner visual/keyboard acceptance, repair, migration, default deletion, explicit retention, actual interruption recovery | No duplicate lifecycle checks. Additional scaling/high-contrast/screen-reader tests waived, not passed. |
| WebView2 prerequisite | Source review of [NSIS configuration](../../src-tauri/tauri.nsis.conf.json), [prerequisite section](../../src-tauri/windows/installer.nsi) and [English errors](../../src-tauri/windows/English.nsh) | Existing download/installation failure paths abort with a required-runtime explanation. Absent-runtime VM testing waived; no claim that it was exercised. |
| App updates and data migration | [Update evidence](app-updates.md), including packaged replacement/restart/retention and signature rejection | No duplicate packaged updater test. Public-feed discovery cannot be established by a private draft. |
| Release integrity and source delivery | [Signed draft evidence](draft-releases.md), nine verified attachments, source/component archives, immutable tags and protected key | Owner smoke of the reviewed hosted installer. |
| Catalog signatures, expiry, advisories and recovery | [Authentication](catalog-authentication.md), publisher rotation/renewal fixtures and native warning/retention controls | Actual publication, normal client refresh and one renewal under #46. |
| Responsiveness and display | [Measured desktop workload](desktop-state.md), [owner display review](display-scaling.md), current pending-work/navigation regression checks | Reuse their stated conditions. No new exhaustive performance/display matrix or universal performance promise. |
| User instructions and diagnostics | [User guide](../user-guide.md), [local development](../local-development.md), [compatibility](../bepinex-mods.md), existing collection-export tests and inspection of Help/copy-source actions | Guide review complete; log export remains unavailable and source-path copying is explicitly unredacted. |

The signed candidate at `617dc7c7e53de35d362d6f08150dff4d3484cf95` passed its checked build and hosted rehearsal. Follow-up `90329b699e7778878ba5be179237b94e853eab26` passed [required checks](https://github.com/Mastervoliumpl/Starframe/actions/runs/35134053731) and [dependency audits](https://github.com/Mastervoliumpl/Starframe/actions/runs/35134053445). Its workflow dependency fix and verification prose do not change installer behavior. Subsequent #30 changes correct documentation and Help text, without changing installation, startup, game or updater logic. Current-head validation is recorded on #30 and PR #60.

## Owner smoke: one install and start

Use the already reviewed `Starframe_0.6.0-dev.2_x64-setup.exe` from the [maintainer draft](https://github.com/Mastervoliumpl/Starframe/releases/tag/untagged-2375b1f85a264b76f2bf). SHA-256: `ea3db2c4b9bb0098751c63c5694a824f22aed0e8d5dfa9e4ff209b6774bb95c6`. Local preparation checks this hash before copying the file into a private smoke kit. The executable is not rebuilt for these documentation changes.

1. Use the existing Windows VM with WebView2 present. Copy the installer into the guest; the guest may remain offline. No game, SDK, repository or signing key is needed.
2. Close an older Starframe instance if present, run setup and complete normal installation. An existing VM installation is acceptable; restoring a clean snapshot and repeating earlier tests is unnecessary.
3. Open Starframe. Confirm the main window renders and Settings and Collections open. A missing-game or offline catalog message is expected. No mod download, game launch, settings-retention campaign or display-settings changes are needed.
4. Close Starframe normally. Report whether install, first launch, navigation and close worked, plus the exact error if any. Uninstall the disposable VM copy if desired; this is housekeeping rather than another acceptance test.

Purpose: exercise the hosted package's installed resources and first launch. It does not re-prove the previously tested manager workflows or validate every byte of later documentation-only builds. A later change to runtime resources, startup or packaging would require a specific reassessment.

## Merge and publication sequence

The catalog workflow and its environment are restricted to main. It cannot establish production publication from the milestone branch. Avoid a second publisher or temporary production-key access merely to move this check before merge.

After the smoke result and maintainer merge approval, merge PR #60 and wait for successful main checks. With maintainer catalog-publication approval, enable the existing workflow, publish once, verify a normal client refresh and manually trigger one renewal. Verify the client accepts the newer signed metadata, the targets match reviewed content and expiry is thirty days. Do not create artificial production advisories or repeat tamper/rotation suites. See [catalog publication](../catalog-publishing.md#first-live-acceptance).

Keep #46 and the milestone open through this live check. Close #30 when its narrowed acceptance is complete; #46 owns the remaining catalog check. A merge, an issue closure or an unpublished draft does not authorize public installer publication. Final version preparation and public release remain a separate maintainer decision.

## Known limits

Windows Authenticode is deferred to #61. No absent-WebView2 execution result or extra assistive-technology/display acceptance is claimed. Existing real-mod evidence is limited to its recorded versions and game build. The six additional curated Remmy releases have not acquired gameplay acceptance simply by appearing in the catalog. Maps, AI packages, other loader formats, multiplayer synchronization and DLL hot reload are outside the current support claims. Manual packaged updater fixtures and cryptographically verified draft assets do not establish discovery of a future public release.
