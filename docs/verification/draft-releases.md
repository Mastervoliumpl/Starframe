# Draft-release verification

The hosted build, protected signing and attachment verification for `0.6.0-dev.2` passed on 16 September 2026 at `617dc7c7e53de35d362d6f08150dff4d3484cf95`. The resulting maintainer-only draft remains unpublished. Nothing in this record authorizes publication.

## Hosted and local evidence

- [Required checks](https://github.com/Mastervoliumpl/Starframe/actions/runs/35129742486) and [dependency audits](https://github.com/Mastervoliumpl/Starframe/actions/runs/35129742465) passed on the exact candidate commit.
- [Rehearsal 35131449841](https://github.com/Mastervoliumpl/Starframe/actions/runs/35131449841) passed the checked-revision gate, runtime source/file verification, Windows NSIS build, protected key-availability check, signing and draft attachment verification. The owner approved uploading the application key and completing the draft-signing task.
- The downloaded unsigned artifact ZIP matched GitHub's recorded digest. Every archived Starframe source file matched `git archive` of the selected commit. All seven component sources matched the reviewed source pins.
- The hosted signing job downloaded and verified all nine uploaded attachments. A second download on the maintainer PC passed both signature checks, the closed file inventory and updater metadata validation. Installer, project source, component sources and release identity retained the exact reviewed unsigned bytes.
- The parallel Windows check in the rehearsal hit the folder-picker helper's 15-second PowerShell deadline. The same candidate had passed native checks in the prerequisite run. The unchanged Windows rerun is retained in the same linked workflow; the failed attempt remains visible in its history. The rehearsal caller now depends on `Required checks`, so a failure in its own test run prevents the release jobs from starting.

The draft includes the installer and its Tauri signature, project and component source ZIPs, `latest.json`, `release.json`, the update public key, `SHA256SUMS` and its signature. The signed checksum inventory covers all seven other files. Installer signatures use the independently trusted public key embedded in the checked source; they are not Windows Authenticode certificates.

| Artifact | SHA-256 |
| --- | --- |
| `Starframe_0.6.0-dev.2_x64-setup.exe` | `ea3db2c4b9bb0098751c63c5694a824f22aed0e8d5dfa9e4ff209b6774bb95c6` |
| `Starframe_0.6.0-dev.2_source.zip` | `8ee6d68867a775ff23737274e336d1cdc6b6b2bb7f15e239fee7c6504b38928d` |
| `Starframe_0.6.0-dev.2_component-sources.zip` | `9f01f2a88dd29e0433afbb1ecd124ee25c36bcd34756ffbd2d5c0fe160878978` |

## Signing and runtime boundaries

The `release` environment contains only the dedicated application key. It requires owner review, disables administrator bypass and now permits only main: the temporary milestone-branch policy was removed after successful signature verification. Catalog authority is separate. The source build receives no signing key; only the approved signing step receives its value, then removes the temporary key file. An active tag ruleset forbids version-tag updates/deletion without bypass actors. No private keys, game reference assemblies or personal home paths are included in source or release artifacts.

The owner-approved local runtime was built with .NET 10.0.400, checked against the installer/notices inventory and scanned for local profile/username strings. The ZIP contains two Starframe DLLs, nine redistributable Microsoft DLLs, the project license and runtime notices. Its SHA-256 is `d6cde02f97f3d611c594b474ecf02d33afc245856df11a23d22e1f48447ff032`; download verification and hosted source/file verification passed against `release/runtime.json`. This establishes the reviewed local input, not a hosted reproduction of proprietary-reference compilation.

Initial rehearsals exposed GitHub's draft-visibility restriction and the reusable workflow's missing secret declaration/mapping. A minimal transfer job handles the draft input; build commands retain read-only permissions. The caller explicitly maps only the application secret name and the callee resolves its protected environment value. The corrected job passed the availability check and cryptographic verification. The immutable `v0.6.0-dev.1` tag and empty failed draft are retained; its two signing attempts never used the key or uploaded release assets. The pre-merge draft is prepared through the maintainer CLI because GITHUB_TOKEN cannot create a release targeting workflow changes; no broader token is stored in Actions.

## Regression coverage and remaining release gates

Thirty-four Python tests cover version/changelog identity, current successful workflow revisions, audit freshness, immutable tags, draft refusal, the exact empty rehearsal draft, runtime source/hash binding, closed archives, component sources and signed artifact accounting. The disposable-key fixture accepts a valid release and rejects changed installer bytes/metadata, invalid signatures and a replacement key. The actual application-key signature verifies and a modified installer fails. Rust formatting/all-target Clippy, workflow lint/formatting and repository/privacy checks passed during implementation.

Ordinary pull-request runs skip the rehearsal and cannot access the signing environment. Source archives retain the five MPL Cargo components, LGPL Doorstop and NSIS sources. Local logs and downloaded artifacts remain under ignored `test-results/0.6.0-release`.

The [release procedure](../releasing.md) describes repeatable preparation. [Packaged client-update acceptance](app-updates.md) remains separate: a private draft cannot be discovered through the public updater feed. The owner accepted this hosted installer on 17 September, completing #30. The [consolidated milestone record](milestone-0.6.0.md) records the live catalog acceptance, reused game/Windows evidence, user guide and explicitly waived additional checks. Windows publisher certificates remain deferred to #61. Public app publication remains a separate maintainer decision.
