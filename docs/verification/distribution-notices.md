# Installer dependency notices

The 0.6.0 installer includes the [desktop notices](../notices/desktop-dependencies.txt), [runtime and installer notices](../notices/runtime-dependencies.txt), [Rust standard-library notices](../notices/Rust-1.98.1-COPYRIGHT.html), existing font/artwork notices and Starframe's AGPL license. The JSON inventories record versions, source locations, license text hashes and runtime DLL identities. No installer has been published.

The installer puts `THIRD_PARTY_NOTICES.md` beside the executable and its linked license files beneath `notices/`. Verification-document links point to the public repository. The ordinary-user installer fixture checks the installed index hash, each linked license target and every notice file hash. This catches the earlier packaging layout that placed the index one directory too deep. The full fixture passed with the corrected layout on 9 September 2026.

## Inventory scope

The Windows x64 Cargo graph contains 341 non-development dependencies, including build dependencies and the catalog TUF verifier. Keeping build dependencies accounts for generated code; it does not mean every listed package is linked into the executable. The production JavaScript source map identifies Svelte and `@tauri-apps/api`. Thirteen Rust packages omit license files from their archives; [upstream records](../notices/upstream-sources.json) supply those texts. The `selectors` archive declares MPL-2.0, with the full license copied from Mozilla's versioned license page. `typed-path` offers an Apache-2.0 alternative in its pinned README; the inventory retains that license from Apache's official versioned text.

The runtime inventory covers six bootstrap component groups, nine Microsoft NuGet packages and two installer components. Bundled Microsoft DLLs match assets inside the exact NuGet archives. The pinned .NET SDK's `nuget verify --all` verifies each package and reports its signature-independent content hash, which must match the lock file. The inventory records the signed archive SHA-512 separately. These hashes differ for signed packages by design; see [NuGet's content-hash description](https://github.com/NuGet/Home/wiki/Nupkg-Metadata-File).

The bootstrap archive is pinned by `runtime/bootstrap.json`. Assembly versions and BepInEx's [pinned build references](https://github.com/BepInEx/BepInEx/blob/v5.4.23.5/BepInEx.Preloader/BepInEx.Preloader.csproj) identify HarmonyX 2.9.0, Mono.Cecil 0.10.4 and MonoMod 22.1.29.1. The BepInEx.Harmony source revision comes from that release's submodule. Doorstop 4.5.0 is selected by BepInEx's build script and its bundled version file.

## Source and redistribution requirements

Preserve each component's terms and notices. The inventories link exact source revisions or versioned source archives; Cargo archive hashes come from Cargo.lock. The components are unchanged upstream binaries or compiled dependencies. Proprietary game/Unity assemblies remain compile-only references and are absent from the runtime payload.

Doorstop 4.5.0 uses LGPL-2.1 separately from BepInEx's MIT license. Retain its full notice and corresponding source, including build scripts, with release preparation. Users retain their rights to inspect, modify and replace the LGPL component. The [pinned Doorstop source](https://github.com/NeighTools/UnityDoorstop/tree/33dab9a6733862eb81869ff08431d9478b28784b) includes its license and build instructions. Starframe's ownership checks can refuse automatic replacement of changed files; they do not change the component's license.

The five MPL-2.0 Cargo packages are cssparser, cssparser-macros, dtoa-short, option-ext and selectors. Their corresponding source must remain available under MPL-2.0, and recipients must be told how to obtain it. Exact source archive links are in the installed notice file. See [MPL-2.0 sections 3.1–3.4](https://www.mozilla.org/en-US/MPL/2.0/).

NSIS 3.11's COPYING file is retained, including the separate CPL-1.0 terms for LZMA compression. The inventory links the matching NSIS source distribution. The Tauri template and helper have separate retained notices. The Rust standard-library notice file is copied unchanged from Rust 1.98.1 and includes component-specific exceptions.

Before publication, #29 must attach matching Starframe source and retain the required component sources with the immutable release artifacts. Verify source downloads at release review; external links alone do not prove that a future release supplied its corresponding source. The Sanctuary artwork permission record still limits use to the accepted free launcher context. This inventory grants no new artwork permissions.

## Refresh and verification

Use the pinned tools, a locked NuGet restore and verified runtime staging. Refresh after reviewing dependency/runtime changes:

```powershell
New-Item -ItemType Directory -Force test-results/notices | Out-Null
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc > test-results/notices/cargo-metadata.json
npm run build -- --outDir test-results/notices/frontend --sourcemap
$noticeMap = (Get-ChildItem test-results/notices/frontend/assets -Filter '*.js.map').FullName
python scripts/generate_dependency_notices.py --metadata test-results/notices/cargo-metadata.json --frontend-map $noticeMap
python scripts/generate_runtime_notices.py
python scripts/check_installer.py
python -m unittest discover -s scripts -p 'test_*.py'
```

Runtime refresh reads public GitHub sources through `gh` and verifies NuGet signatures. Ordinary packaging reads generated inventories without network access. It fails when tracked dependency inputs, notice texts, rendered notices or third-party runtime DLLs differ from the reviewed records. This catches drift; it does not authenticate a hostile build machine or replace release signatures. Analysis source maps stay under ignored test output, outside application resources.

On 9 September 2026, both generators completed, all 27 Python tests passed and packaging preflight accepted the staged runtime. Fixtures cover graph traversal, development-dependency exclusion, stale inputs, changed license text and replacement DLL rejection. Other installer acceptance remains in [#27's verification record](windows-installer.md).
