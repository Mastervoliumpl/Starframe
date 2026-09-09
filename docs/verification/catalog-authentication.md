# Catalog authentication, issue 46

The first implementation adds a TUF verification boundary using `tough` 0.24.0. It compiles with the pinned Windows toolchain and verifies a signed `catalog.json` target before parsing Starframe's catalog. It is not yet connected to the desktop refresh or download commands. No production keys or signed catalog have been published.

## Tooling decision

[tough](https://github.com/awslabs/tough/releases/tag/tough-v0.24.0) supplies the Rust client and [tuftool](https://github.com/awslabs/tough/tree/tuftool-v0.17.0/tuftool) supplies a publisher for static TUF repositories. The maintained alternative [rust-tuf](https://github.com/theupdateframework/rust-tuf) was also inspected; its current package identifies itself as 0.3.0-beta15. Both provide established metadata verification. Starframe uses tough's repository loader and publisher interfaces to keep signature, role and rotation rules in the maintained library. A detached file signature alone would leave Starframe to implement those repository rules.

The integration disables tough's optional HTTP transport and reuses Starframe's existing reqwest client: HTTPS, no redirects, connection and request timeouts, and no hidden retries. It permits only the two configured metadata/target directory URLs, bounds each response to 2 MiB and each refresh to 40 requests and 60 seconds. TUF metadata limits further restrict root/timestamp files to 64 KiB, snapshot/targets metadata to 256 KiB where the library's signed-length rules permit, and root updates to 32 per attempt. The final catalog target must fit 2 MiB even when a trusted signer declares a larger target.

The Windows dependency graph adds 30 non-development packages, including the client and AWS-LC verification dependency, for 341 Rust packages plus two bundled JavaScript packages. Existing futures utilities supply stream adapters. The refreshed notice inventory retains the new source and license records. `typed-path` 0.9.3 omits standalone license files; its [pinned README](https://github.com/chipsenkbeil/typed-path/blob/08cae85913d37b862b1d0e1aa2fa9e3857e9312e/README.md) offers MIT or Apache-2.0, so the inventory includes the [Apache-2.0 text](https://www.apache.org/licenses/LICENSE-2.0.txt).

## Trust and freshness

The caller supplies an embedded trusted root and an owned persistent TUF datastore. The module rejects reparse points in that directory, retains metadata versions for rollback checks and resumes from an accepted cached root. Retaining the root matters even if a later refresh stage fails; an old embedded root must not restore a revoked signing authority. This datastore assumes the same local integrity boundary as Starframe's existing saved data; it is not protection against a hostile process with the user's filesystem access.

On 9 September 2026 the owner selected daily GitHub Actions renewal with thirty-day validity. Expired metadata pauses new catalog downloads. Installed offline use and cached confirmed security blocks remain effective. Catalog/advisory keys must be separate from app-release keys. The publishing workflow, credential provisioning, root backup procedure and desktop policy integration remain unfinished; the cadence here records the accepted behavior, not an already running renewal job.

## Verification

The focused Rust fixtures create fresh Ed25519 keys in memory and sign real TUF metadata through tough's publisher API. No private key material is committed or logged. The tests verify:

- Valid catalog bytes and their authenticated expiry.
- Rejection of changed target bytes and signed-metadata tampering.
- Rollback rejection after a newer signed version was retained.
- Expired metadata rejection and subsequent valid recovery.
- Root rotation signed by both old and new authorities, retained even when expired timestamp metadata interrupts the refresh.
- Rejection of an unrelated replacement root.
- HTTP error/redirect rejection, missing-file classification, oversized and truncated responses, endpoint restrictions and the request budget.

The focused Windows tests passed on 9 September 2026. This does not complete #46: signed refresh/cache migration, advisory identities and corrections, download/activation enforcement, reporting routes, publishing and full desktop acceptance still need implementation and verification.
