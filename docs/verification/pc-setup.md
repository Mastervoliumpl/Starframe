# Windows development setup verification

Verified on 9 September 2026 against the internal 0.5.0 application and catalog revision 3. This checks a transferred checkout on another Windows PC. It does not start milestone 0.6.0 or publish an installer.

## Tools and builds

Node.js 24.19.0, npm 11.17.0, Rust/Cargo 1.98.1 with Rustfmt and Clippy, Python 3.13.15 and .NET SDK 10.0.400 are installed. Visual Studio C++ tools, Windows SDK 10.0.26100.0 and WebView2 are available. Dependencies were restored from the existing lockfiles. The exact .NET SDK was restored alongside a newer installed patch because runtime/global.json disables roll-forward.

| Check | Result |
| --- | --- |
| Repository | Version agreement, links, SVGs, local-instruction exclusion and home-path privacy passed; 21 Python tests passed. |
| Frontend | Formatting, ESLint, Svelte/TypeScript checks, 10 unit tests and the production build passed. |
| Browser | All 20 Playwright tests passed. |
| Rust | Rustfmt, Clippy with warnings denied, Cargo tests and catalog validation passed. |
| Windows executable | Debug and release Tauri builds passed without installer packaging. |
| Native Windows | All eight scripts passed: desktop, storage, game discovery, catalog, packages, mods, local import and local watching. The mods script passed after the synchronization correction below. |
| Core C# runtime | Locked restore, formatting, release build and all 91 tests passed. |
| Unity entry plugin | Compiled against read-only references from the owner's installed game. The two previously recorded MSB3277 conflicts for System.IO.Compression and System.Net.Http remain. |
| BepInEx fixture plugin | Compiled with no warnings or errors. |

The newly compiled runtime and verified pinned bootstrap were staged beside `src-tauri/target/release/starframe.exe` in its `integration` directory. This prepares local build resources; it does not deploy them to the game. No game launch or gameplay acceptance was performed during setup.

## Native test synchronization

The mod-management test initially failed at uninstall. Its direct IPC collection selection advanced the saved revision before the frontend's next refresh. Opening the confirmation dialog captured the earlier revision, and Rust rejected the request with the existing stale-revision error. Diagnostic output confirmed that rejection; the deployment and library were retained.

The test now waits for the enabled switch to reflect the selected collection before opening the dialog. It also checks that confirmation closes the dialog before waiting for deployment. This changes test synchronization only; application behavior and stale-write validation are unchanged.

## Privacy

The local handoff file was never tracked in the reachable Git history. Tracked text and 1,023 historical file blobs were scanned for the owner's local usernames, home paths, private IP addresses, machine-name patterns and common token markers without matches. The two tracked application screenshots contain no visible personal information; those images and the launch artwork have no EXIF metadata. Public issue/PR bodies and comments had no matching personal details.

Personal hardware details were removed from current verification records. The repository check now rejects absolute home-directory paths in tracked UTF-8 text, including fenced examples, escaped Windows paths and URL-encoded paths. It reports the file and line without echoing the private value. It is not a general secret scanner or image audit.

A personal email remains in 73 existing commits, and earlier file versions retain the removed hardware details. Shared history has not been rewritten. New cleanup commits use the owner's public GitHub handle and noreply email.

Machine paths, installation logs, skill source revisions and raw test diagnostics remain in ignored local evidence under `test-results/pc-setup-2026-09-09`. The transferred native-test evidence was preserved there before running the new tests.
