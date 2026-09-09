# Third-party notices

The 0.1.1 application links rusqlite 0.40.2 and libsqlite3-sys 0.38.2 under the MIT license. The bundled SQLite 3.53.2 source is public domain. The notice below is copied from the pinned rusqlite source package. Turso is no longer linked; retained Turso-generated database fixtures contain synthetic Starframe records.

This notice supplements the repository license. Installer inventories include the [desktop dependencies](notices/desktop-dependencies.txt), [runtime and installer components](notices/runtime-dependencies.txt) and [Rust standard library](notices/Rust-1.98.1-COPYRIGHT.html). See [inventory scope, source obligations and refresh checks](verification/distribution-notices.md). No installer has been published.

## Tauri installer template

The NSIS template derives from Tauri CLI 2.11.4, copyright (c) 2017 - Present Tauri Apps Contributors, under the [MIT license](notices/Tauri-MIT.txt). Starframe changes the uninstall checkbox to offer explicit retention and marks an app replacement as an update so it preserves data. Source: https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi.

## Oxanium font

Copyright 2019 The Oxanium Project Authors. The bundled, unchanged Oxanium variable font is used at weight 800 for the Starframe wordmark and launch control. It is licensed under the [SIL Open Font License 1.1](notices/OFL-Oxanium.txt). Source: https://github.com/google/fonts/tree/main/ofl/oxanium.

## Sanctuary artwork

The launch image is copyright Enhearten Media and/or its respective artist, with separate use conditions. It is not licensed under Starframe’s AGPL code license. See the [source and permission record](notices/Sanctuary-artwork.md).

## rusqlite and libsqlite3-sys

Copyright (c) 2014 The rusqlite developers

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.

## SQLite

SQLite is in the public domain. Source: https://sqlite.org/copyright.html. Bundled source: libsqlite3-sys 0.38.2, sqlite3/sqlite3.c, SQLite 3.53.2.

## Runtime build dependencies

The runtime uses System.Text.Json 10.0.11 and its locked Microsoft .NET dependencies under MIT terms. The [runtime inventory](notices/runtime-dependencies.txt) retains their source licenses and NuGet third-party notices. The .NET Standard reference pack is used only for compilation. The test host uses Microsoft.NET.Test.Sdk 18.9.0 and MSTest 4.4.0 under MIT terms; they are not runtime payloads. See [runtime build and SDK limits](verification/runtime-contracts.md).

Rust's contract reader uses sha2 0.10.9 under MIT OR Apache-2.0. It is included in the desktop inventory with the other Windows build dependencies.

## Prepared BepInEx bootstrap

Preparation downloads the unchanged official BepInEx 5.4.23.5 Windows x64 archive and verifies its SHA-256. The installer includes the [BepInEx MIT notice](notices/BepInEx-5.4.23.5.txt) and the component notices for HarmonyX, BepInEx.Harmony, Mono.Cecil, MonoMod and Doorstop. Doorstop 4.5.0 has separate LGPL-2.1 terms. Exact source references and redistribution requirements are recorded in the runtime inventory and verification record above. This repository contains the inventories and notices, not those binaries. Public SDK publication remains outside this issue.

Issue #13 uses compile-only Unity references from the local installed game and BepInEx references from the verified package. They are not copied into build outputs or committed. The core runtime targets .NET Standard 2.0; NETStandard.Library is a build reference under Microsoft .NET terms, and the existing locked MIT dependency notices still apply. Runtime preparation includes the complete Microsoft dependency output needed by Mono.
