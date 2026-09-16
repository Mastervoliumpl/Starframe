# App updates

Issue [#28](https://github.com/Mastervoliumpl/Starframe/issues/28), milestone 0.6.0. Local acceptance passed on 16 September 2026; hosted checks are recorded on the issue and PR. This record does not authorize a release.

The Rust storage worker starts release checks after the frontend subscribes. Successful checks repeat after five minutes, including while minimized. Manual requests join an active check; resume causes one due check. Failed checks back off from five minutes to eighty minutes and respect GitHub's Retry-After (seconds or HTTP date) and rate-limit reset. Failed checks retain the earlier release notice and last successful check, without reporting that the installation is current.

Fresh prerelease installations default to Preview; fresh stable installations default to Stable. The selected channel and Later choice are saved in SQLite schema 14. Preview includes eligible stable releases. Selection compares semantic versions rather than release dates and never downgrades. Draft releases, catalog-only releases and releases without updater metadata are excluded. A successful refresh removes a withdrawn or ineligible notice. Release notes render as plain text. Later hides the navigation notice while retaining the release in Settings.

Update is an explicit command. It refreshes the release listing and verifies that the selected version remains eligible before fetching its per-release latest.json. The Windows target must name that release's official installer URL. Tauri verifies the downloaded bytes using the embedded application update key. Metadata and release notes use GitHub HTTPS; the detached signature authenticates the installer bytes, not every metadata field. A failed signature or failed transfer is an update error, not a malware finding.

The verified installer waits in memory while the game, package queue, catalog refresh or game operation is active. The local-source copier receives cancellation and must finish before installation. The single storage owner checkpoints SQLite, rechecks Sanctuary processes and invokes Tauri's installer. Cancel or desktop close discards the pending download. Tauri starts the finite NSIS update and exits; the installer restarts Starframe, whose normal automatic deployment prepares the matching runtime after the game closes. No service or persistent checker is installed.

## Verification to date

- Release-policy tests cover semantic ordering, stable/preview eligibility, drafts, official asset URLs, bounded cache parsing, saved preferences, concurrent checks, resume, retry delays and stop behavior.
- Browser acceptance covers Later, plain-text release notes, keyboard channel selection, retained notices on failure and navigation during waiting/cancellation.
- Native acceptance exercises the actual Tauri signature verifier with a locally generated disposable key. Valid bytes reach Waiting while an inert Sanctuary-named process runs; tampering and malformed signatures never reach installation. It also covers withdrawn releases, saved Later across restart, channel changes, HTTP 503/Retry-After and clean exit.
- Storage migration fixtures now remove the new table when reconstructing older schemas. Migration/restart, interrupted transactions and retained backups pass with schema 14.
- The key-transition fixture rejects a replacement key while the old public key is configured, then accepts the same signed bytes after explicitly configuring the replacement public key. It does not claim automatic lost-key recovery.
- Final local checks passed: frontend formatting/lint/types/build and ten unit tests; 26 browser tests; Rust formatting/Clippy and tests; all ten native scripts; 27 Python tests; repository/privacy/version/notices checks; and release-profile NSIS packaging with an application-key signature. The new dependency notices include the updater verifier and HTTP-date parser.

The native HTTP fixture override exists only in debug builds, requires STARFRAME_TEST_DATA_DIR and a bounded update-fixture.json, and accepts only an explicit 127.0.0.1 HTTP port. Release builds cannot redirect update authority through this fixture. The native signature test serves inert non-executable bytes and does not run an installer.

The packaged upgrade passed from 0.6.0-dev.0 to 0.6.0-dev.1: Tauri verified the installer signature, NSIS replaced the app, the new process reopened automatically, and library/collection records, game configuration and original import sources remained intact. Automatic runtime preparation returned to Ready. The separately identified test installation was uninstalled. Both packaged builds use debug-only loopback test wiring and a disposable signing key; this does not establish a live hosted release or publisher trust. All local #28 checks passed; hosted results are recorded on the issue. Final display/accessibility and public-release acceptance remain in #30.

## Signing and recovery

The application update public key is in src-tauri/tauri.conf.json. Its private counterpart is separate from catalog/advisory keys. The owner confirmed secure backup on 16 September 2026. No app update secret has been uploaded, and no release has been published. #29 owns the restricted release environment and draft workflow.

Use the maintained [Tauri signer and updater](https://v2.tauri.app/plugin/updater/). Combine tauri.nsis.conf.json with tauri.updater.conf.json for a signing build, supplying TAURI_SIGNING_PRIVATE_KEY through a protected local/release environment. Sign the final installer bytes. If Authenticode is introduced under #61, it must run before detached signing; any later byte change invalidates the detached signature.

For a planned key transition, issue an installer signed by the current key that embeds the replacement public key. Clients must install that transition release before publication switches to the new signing key. This implementation uses one embedded authority: clients that miss the transition need a verified manual reinstall. There is no claim of automatic recovery across a missed transition or a lost key.

If the key is lost, recover the backed-up private key; otherwise publish a new trusted installer for manual reinstall. Preserve app data through the normal reinstall path. Never disable signature checks as a recovery method.

If compromise is suspected, stop release publication and remove affected release metadata/assets while investigating. A stolen signing key cannot safely authorize its own replacement for all already-installed clients. Announce the incident through the project's independent communication channels and provide a verified manual reinstall with a new authority. Offline clients cannot learn of a revocation immediately. Catalog advisory keys remain independent and do not authorize application updates. An Authenticode certificate would not remove these artifact-key recovery requirements.

## Repeating the packaged check

Use `scripts/prepare_update_fixture.py` to copy the current source into the ignored `test-results/0.6.0-updates/source` directory and set the first test version to `0.6.0-dev.0`. The copied app has the separate Starframe Updater Test product/registry identity. Build its debug NSIS installer and retain it as `old-setup.exe` under the evidence directory. Repeat for `0.6.0-dev.1`, enable `tauri.updater.conf.json`, and sign with a disposable test key. Retain the new installer/signature as `new-setup.exe` and `new-setup.exe.sig`, with the test public key at `fixture.key.pub`.

Run `node tests/native/packaged-update.mjs` from the main checkout as an ordinary user. The test refuses an existing registered fixture, serves metadata/artifacts on loopback, seeds a local package and collection, invokes Update, verifies the automatically reopened process and retained state, and uninstalls the fixture. It leaves evidence and temporary source/data directories for inspection. It does not install into the normal Starframe identity or touch the owner's game. Do not publish these debug fixtures.