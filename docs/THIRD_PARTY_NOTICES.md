# Third-party notices

The 0.1.1 application links rusqlite 0.40.2 and libsqlite3-sys 0.38.2 under the MIT license. The bundled SQLite 3.53.2 source is public domain. The notice below is copied from the pinned rusqlite source package. Turso is no longer linked; retained Turso-generated database fixtures contain synthetic Starframe records.

This storage notice supplements the repository license. A complete distribution notice inventory remains part of installer packaging; no installer has been published.

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

The internal runtime uses System.Text.Json 10.0.11 and its locked Microsoft .NET dependencies under MIT terms. The official NuGet packages include their licenses and third-party notices; retain those files with any future binary distribution. The .NET Standard reference pack is used only for compilation. The test host uses Microsoft.NET.Test.Sdk 18.9.0 and MSTest 4.4.0, also under MIT terms; they are not runtime payloads. No proprietary game references, BepInEx payload or SDK package is distributed by issue #11. See [runtime build and SDK limits](verification/runtime-contracts.md).

Rust's contract reader directly uses sha2 0.10.9, already present in the dependency graph, under its MIT OR Apache-2.0 license. The installer notice inventory must include it and the existing desktop dependencies.
