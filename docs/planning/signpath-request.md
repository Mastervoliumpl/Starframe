# SignPath eligibility request

Prepared on 9 September 2026 for maintainer review. This request has not been submitted. It contains the public project identity and no personal contact details or local machine paths.

Submit through the [SignPath Foundation application page](https://signpath.org/apply) after reviewing the text. The [eligibility terms](https://signpath.org/terms) require an existing release, open-source components, verifiable builds and manual signing approval. Admission is discretionary. Starframe has unresolved questions about those requirements, so this asks for an eligibility assessment rather than asserting compliance.

## Subject

Eligibility enquiry: Starframe, an AGPL Windows game mod manager

## Message

Hello SignPath Foundation team,

I maintain [Starframe](https://github.com/Mastervoliumpl/Starframe) under the GitHub account [Mastervoliumpl](https://github.com/Mastervoliumpl). I would like to check its eligibility for Foundation code signing before applying for its first Windows alpha release.

Starframe is a free, Windows-first mod manager for Sanctuary: Shattered Sun. Its code is licensed under AGPL-3.0-only, without commercial dual licensing. The desktop uses Tauri, Svelte, Rust and bundled SQLite. It manages exact curated releases and local imports, ordered collections and game launch. Mods download from their authors' sources; Starframe does not host their binaries.

The app also installs a reversible BepInEx 5 bootstrap and Starframe-owned C# runtime into a game installation selected by the user. BepInEx uses a proxy DLL to bootstrap code inside the game process. The runtime loads the user's enabled mods and adds an in-game settings menu. These modifications are explained in the app and apply only while the game is closed. Starframe provides ownership-based removal and recovery. It does not scan other systems for vulnerabilities or exploit security vulnerabilities. Would this game-modding functionality be eligible under your restrictions on hacking tools?

Internal milestone 0.5.0 is complete. We have built a local NSIS installer for 0.6.0 development, but have not published an executable or installer release. The project currently requires signing before public installer distribution. Can you assess the project using a review build, or is an existing public unsigned release required before consideration?

There are two additional points I would like to resolve:

1. The app includes a Sanctuary promotional illustration with a recorded permission exchange for free use and attribution. This non-code artwork has separate copyright conditions and is not under the project's AGPL license. Its [permission record](https://github.com/Mastervoliumpl/Starframe/blob/main/docs/notices/Sanctuary-artwork.md) is public. Can a package containing this asset qualify?
2. The Unity entry plugin compiles against proprietary game assemblies from a lawful local game installation. Those reference assemblies are never bundled or published. The desktop and reference-free runtime already build in GitHub Actions. A lawful acquisition method for the game-specific references on hosted runners is still unresolved. What provenance and reference-access arrangement would you require to sign the resulting Starframe binaries?

The intended package contains Starframe binaries, the unchanged official BepInEx bootstrap, Microsoft runtime dependencies, an OFL font and their notices. We are completing the exact redistribution inventory. We would sign our own executable/runtime and installer, without re-signing upstream libraries as our own work.

Our release plan uses checked repository revisions, hosted builds, explicit maintainer approval and draft release review. Windows publisher signing, Tauri updater signing and catalog/advisory authentication have separate authority. We have not enabled a release-signing workflow or claimed SignPath sponsorship. If accepted, we would configure your required roles, multifactor authentication, artifact restrictions, signing policy and network/privacy disclosures before requesting a production signature.

The app checks GitHub catalog metadata automatically while open. Requested mod downloads contact the author's host. The forthcoming app updater will also check official GitHub releases while the app is open. Our privacy wording will describe these requests rather than claim the application never connects without a separate click.

Please let me know whether the project can qualify and which of the points above must be resolved before a formal application.

Thank you,

Mastervoliumpl

## Maintainer notes

Use your chosen contact address in the application form, not in a public repository file. Verify your account's multifactor-authentication status yourself; this draft does not claim it is configured. A SignPath decision does not replace the project's redistribution review or authorize a public release.

Microsoft [Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/overview) is a hosted alternative if Foundation eligibility does not fit. Its service requires an Azure account and identity validation; no paid service or subscription has been selected. Preserve the current artwork and local signing-device configuration while these questions are open.
