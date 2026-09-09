# Catalog publication

Issue #46 is in progress. The publisher, embedded trust root and desktop signed refresh are implemented. The workflow is disabled until `CATALOG_PUBLICATION_ENABLED` is set to `true`. Credential upload requires owner approval; no signed catalog has been published. The first hosted renewal and live client refresh remain acceptance checks before public distribution.

The initial public root is [catalog/trust/root.json](../catalog/trust/root.json), version 1, with separate RSA-4096 recovery and online publishing keys. Its file SHA-256 is `2ab2812d4d89d4d37d2722b64c6e1902ee1dacb80f44de2bbcfd5206fc887345`, and it expires on 9 September 2027. The owner confirmed a secure backup of the recovery key and root on 9 September 2026. Private keys remain outside tracked files. A candidate signed with these keys passed local TUF target verification without publication.

## Authority and hosting

Use a separate offline root key and online catalog key. The online key signs TUF targets, snapshot and timestamp metadata; it cannot change the root authority. Neither key is an app-release signing key. Keep the root private key outside GitHub Actions and retain an owner-controlled backup before enabling publication. A root must have more than thirty days remaining before the publisher will sign another thirty-day catalog.

The `catalog` GitHub environment holds only `CATALOG_SIGNING_KEY`, containing the online key's PKCS#8 PEM. Restrict that environment to `main`. Its renewal job must run without a manual approval on each daily renewal. Root changes and catalog/advisory edits still require maintainer review through the normal main-branch checks. Keep private keys out of the repository, issue comments, logs and uploaded build artifacts.

The [workflow](../.github/workflows/catalog.yml) runs daily at 03:17 UTC and after successful Checks runs on main. It also permits manual dispatch. Before and after preparation it confirms that the checked source commit is still current main. It serializes publication jobs and pushes normally to `codex/catalog-published`; it never force-pushes. A rejected push retains the previous publication. Versioned metadata and hashed target filenames remain on that branch so clients can finish a refresh across publication boundaries. Root history must remain available for older clients to rotate their trust.

Daily renewal advances TUF metadata versions and gives targets, snapshot and timestamp metadata thirty days of validity. It does not change catalog or advisory revisions when their content is unchanged. A failed renewal leaves the previous signed publication in place. The desktop pauses new catalog downloads after expiry while retaining installed offline use and known confirmed findings.

## Preparation and verification

Install `tuftool` 0.17.0 with its lockfile. Build Starframe's validator and development-only signing example:

```powershell
cargo install tuftool --version 0.17.0 --locked --root test-results/catalog-tools
cargo build --manifest-path src-tauri/Cargo.toml --locked --bin check_catalog --example sign_catalog
python scripts/verify_catalog_publishing.py --tuftool test-results/catalog-tools/bin/tuftool.exe --validator src-tauri/target/debug/check_catalog.exe --signer src-tauri/target/debug/examples/sign_catalog.exe
```

The test creates disposable RSA keys, verifies the initial publication and renewal, and rejects a wrong signing key, invalid advisory data and modified previously published targets. It also checks recovery from a correctly signed but expired repository. The fixture keys are removed with their temporary directory.

`scripts/publish_catalog.py` accepts the reviewed catalog directory, public root, online key file, a new output directory and the previous publication directory. It validates the source data and retained identities before signing. It verifies both resulting targets again through TUF and checks that their bytes match the reviewed inputs. It produces files for publication; it does not push them itself.

The small `sign_catalog` example uses tough's repository editor, signature implementation and verified target-copy API. The upstream CLI's target writer requires symlink privileges on Windows, so Starframe uses the library's ordinary-file copy operation for that step. No signing implementation is copied or replaced. `tuftool` supplies root administration and an independent download verification command.

Only the publisher uses `--allow-expired-repo` when examining its previous publication for renewal. This preserves signature/hash verification and retained-content checks while allowing the maintainer to issue fresh metadata after an outage. It does not authorize downloads in the desktop; that client retains normal expiry enforcement. The public root is always supplied explicitly, never downloaded as a new trust anchor.

## Rotation and recovery

To replace an online key, retain the current public root, increase its version, replace the affected role key IDs and sign the new root with the offline authority. Verify a client starting from the old embedded root before publishing. Update the catalog environment's online credential only as part of that checked change. Retain every numbered root file in the publication branch.

To replace the offline root key, sign the new root under both the old and new root authorities. `tuftool root sign` supports the old root through `--cross-sign`; keep both signatures. The client tests cover accepted cross-signing and rejection of an unrelated root. If the old root authority is lost and there is no usable backup, an app update carrying a new reviewed trust root is required. Do not reset retained metadata or silently accept an unsigned replacement.

On compromise, stop the publishing workflow, revoke the affected online credential, review published target history and rotate the authority with the retained offline key. Append signed corrections to incorrect advisories; do not erase earlier findings. Review the separate app-release authority if evidence shows it was also exposed. The owner backup is confirmed; rotation/recovery tests use disposable authorities. A production recovery drill has not been performed.
