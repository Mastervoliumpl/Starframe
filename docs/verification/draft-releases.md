# Draft-release verification

Issue #29 is in progress on 16 September 2026. The [release procedure](../releasing.md) describes the implemented gates and owner-approved local plugin input. Hosted rehearsal and application-key upload remain pending. Nothing in this record authorizes publication.

Local checks passed:

- Thirty-three Python tests cover release/changelog identity, exact successful workflow revisions, newer failed attempts, audit freshness, immutable tags, existing draft refusal, runtime source/hash binding, closed archive inventories, required component source coverage and signed artifact accounting.
- The disposable-key release fixture signs inert installer bytes and the checksum inventory, verifies both, and rejects changed installer bytes, altered metadata, an invalid signature and an untrusted replacement key. No fixture installer is executed.
- The maintained Minisign verifier accepts the existing application-key-signed installer and rejects an appended-byte copy. Its example target builds, formats and passes Clippy.
- The reviewed Doorstop and NSIS source archives download successfully, match the recorded hashes and contain their license/build records. MPL source hashes come from the Cargo notice inventory.
- Actionlint 1.7.12 validates the release workflow and its explicit rehearsal caller; Prettier and repository/privacy checks pass.

The build job cannot access a signing secret. A fresh signing job follows environment approval, verifies the selected revision again, signs final bytes and creates only a draft. Downloading and independently verifying uploaded attachments is mandatory before a run reports success. There is no publication command or asset replacement option.

The release environment now requires the owner's review, disables administrative bypass and permits only main plus the explicit milestone rehearsal branch. The application signing key has not been uploaded. An active tag ruleset rejects update/deletion of version tags without bypass actors. The local runtime was rebuilt with .NET 10.0.400, checked against the installer/notices inventory and scanned for local profile/username strings. Its closed ZIP was uploaded to an unpublished input draft, downloaded again and verified against `release/runtime.json`. No game references or private keys were uploaded.

Remaining acceptance: approve any application-key transfer separately, run the exact checked revision through hosted draft preparation, and verify the resulting draft attachments. Remove the temporary environment branch policy after rehearsal. Confirm ordinary pull-request runs skip the rehearsal caller. The packaged client-update acceptance from #28 remains recorded in [app updates](app-updates.md); a private draft cannot be discovered through the public updater feed.
