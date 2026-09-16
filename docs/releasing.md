# Preparing a Windows draft release

Issue #29 adds a manually dispatched release workflow. It builds the desktop and installer on a hosted Windows runner, then requests approval before a separate runner receives the application signing key. It creates a draft, downloads every attachment and verifies the signatures and file inventory again. Publication remains a maintainer action after #30 acceptance. Windows Authenticode remains deferred to #61.

## Trusted inputs

Finalize a dated changelog entry matching VERSION. All npm, Cargo, Tauri and runtime version values must agree. Select the full commit SHA of current main after both Checks and Dependency security pass on that exact revision. A newer failed or cancelled attempt does not inherit an earlier success. The workflow rejects a changed branch head, a version tag pointing elsewhere, or an existing draft/release with that version. It never replaces tags or uploaded assets.

The `release` GitHub environment requires the owner's review, disables administrative bypass and restricts deployment branches. It holds only `APP_UPDATE_SIGNING_KEY`, the separately backed-up application key. The catalog environment and its online key are independent. The build job has read-only repository permissions and no signing credentials. The signing job has a fresh checkout, no restored build cache, and sees the private key only in the signing step. Temporary key files are removed afterward. Pull requests cannot enter this workflow or access the environment.

For the pre-merge 0.6.0 rehearsal, the existing Checks workflow accepts `draft_release_commit` on `codex/0.6.0-windows-alpha`. This calls the same release workflow; it does not bypass its checked-commit, identity or approval gates. Temporarily permitting that exact branch in the release environment requires owner approval. Remove its environment branch policy after the rehearsal. Normal release dispatch is restricted to main. The rehearsal creates a development draft, never a public release.

## Local in-game plugin input

The owner approved a local plugin build because its proprietary game references are unavailable on hosted runners. This is an explicit build input, not a claim that GitHub reproduced the plugin. Source/file hashes establish which input was reviewed; they cannot prove that a compromised local compiler produced faithful binaries.

Commit runtime source and build scripts first. Build against the lawful local game references and the verified BepInEx package:

```powershell
./scripts/build_release_runtime.ps1 -GameManagedPath $env:STARFRAME_GAME_MANAGED -BootstrapPath $env:STARFRAME_BOOTSTRAP -Output $env:STARFRAME_RUNTIME_OUTPUT
```

The script uses the pinned .NET SDK, locked packages, a new output directory and mapped source paths. `runtime.zip` includes only the closed list of Starframe/runtime DLLs; game and Unity reference assemblies, PDBs and local paths are excluded. Review the package and its `runtime.json`, then upload only `runtime.zip` to a separately labelled draft input release. Record that draft's tag as `inputTag` in the manifest and commit it as `release/runtime.json`. Never publish the input draft. Retain it while a release workflow needs it.

The manifest binds the product version, runtime source/build inputs, archive hash and every DLL hash. GitHub verifies those values before extraction, then runs the ordinary installer inventory/notices preflight. A runtime or build-script change requires a fresh input. Final release source archives contain the source and build instructions; they do not contain proprietary reference assemblies. Users rebuilding the plugin must supply their own lawful game installation.

## Draft contents and verification

The draft contains the versioned NSIS installer and its Tauri signature, matching Starframe source, required component sources, `latest.json`, `release.json`, the public key, and signed `SHA256SUMS`. Source archives retain the five MPL Cargo components, LGPL Doorstop and the NSIS source distribution. Hashes come from the reviewed dependency inventories and source pins. Starframe source comes from `git archive` of the selected commit, excluding untracked files and private signing material.

Tauri signs the final installer bytes. The independently signed checksum inventory covers the installer, its signature, updater metadata, source archives, public key and release identity. The application's updater verifies the installer signature; it does not consume the checksum signature. No artifact may be altered after signing. Future Authenticode signing must precede this step.

The public key is embedded in `src-tauri/tauri.conf.json`. SHA-256 of its decoded Minisign public-key text is `aa153b836c11a9c901a4bc6e689fcac89c6a63d93f91195d9ce8d2974d90f189`. Confirm this through an independently trusted project source before relying on a first-download signature; the downloaded key cannot authenticate itself. Detached signatures do not remove Windows publisher warnings.

To verify downloaded attachments from a trusted source checkout, build the maintained verifier and run the release check, supplying the independently reviewed release identity:

```powershell
cargo build --manifest-path src-tauri/Cargo.toml --locked --example verify_release
python scripts/release_artifacts.py verify --release $env:STARFRAME_RELEASE_IDENTITY --output $env:STARFRAME_DOWNLOADED_RELEASE --verifier src-tauri/target/debug/examples/verify_release.exe
```

The verifier uses the same maintained Minisign implementation as Tauri, with streaming verification. It rejects altered files, unknown signatures, changed metadata and extra or missing attachments. A failed upload or verification leaves an unpublished draft for inspection. It is not automatically deleted or retried with replacement files. Review an incomplete draft before removing it and retrying; retain its version tag and exact source commit.

See [application-key recovery](verification/app-updates.md#signing-and-recovery), [dependency notices](verification/distribution-notices.md) and the [Tauri updater documentation](https://v2.tauri.app/plugin/updater/). GitHub documents [environment protection](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments); a workflow condition alone is not the secret boundary.
