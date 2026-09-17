"""Refresh runtime notice/source records using installed packages and pinned GitHub sources."""

import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET
import zipfile

from generate_dependency_notices import digest, notice_files, render
from prepare_desktop_runtime import RUNTIME_FILES


BOOTSTRAP = [
    ("BepInEx", "5.4.23.5", "BepInEx/BepInEx", "v5.4.23.5", "MIT", ["LICENSE"],
     ["BepInEx.dll", "BepInEx.Preloader.dll"]),
    ("BepInEx.Harmony", "2.0.0", "BepInEx/BepInEx.Harmony", "d4cdcb4cdeac14a0b77012165f5f5a9f5032a9fa", "MIT", ["LICENSE"],
     ["0Harmony20.dll", "BepInEx.Harmony.dll", "HarmonyXInterop.dll"]),
    ("HarmonyX", "2.9.0", "BepInEx/HarmonyX", "31d794f3affce55fa87c99efac7dae23a126cf52", "MIT", ["LICENSE", "LICENSE.Harmony"],
     ["0Harmony.dll"]),
    ("Mono.Cecil", "0.10.4", "jbevain/cecil", "98ec890d44643ad88d573e97be0e120435eda732", "MIT", ["LICENSE.txt"],
     ["Mono.Cecil.dll", "Mono.Cecil.Mdb.dll", "Mono.Cecil.Pdb.dll", "Mono.Cecil.Rocks.dll"]),
    ("MonoMod", "22.1.29.1", "MonoMod/MonoMod", "68cf23127bd2394004e8a812b160a0862c95a309", "MIT", ["LICENSE"],
     ["MonoMod.RuntimeDetour.dll", "MonoMod.Utils.dll"]),
    ("UnityDoorstop", "4.5.0", "NeighTools/UnityDoorstop", "33dab9a6733862eb81869ff08431d9478b28784b", "LGPL-2.1", ["LICENSE"],
     ["winhttp.dll"]),
]


def github(repository: str, revision: str, filename: str) -> str:
    value = json.loads(subprocess.check_output([
        "gh", "api", f"repos/{repository}/contents/{filename}?ref={revision}",
    ]))
    return base64.b64decode(value["content"]).decode("utf-8-sig")


def collect(root: Path) -> dict:
    texts = {}
    packages = []
    integration = root / "src-tauri/target/installer/integration"

    def notice(filename: str, source: str, content: str) -> dict:
        key = digest(content.encode("utf-8"))
        texts[key] = content
        return dict(file=filename, source=source, text=key)

    for name, version, repository, revision, license_id, filenames, binaries in BOOTSTRAP:
        notices = [notice(filename, f"https://github.com/{repository}/blob/{revision}/{filename}",
                          github(repository, revision, filename)) for filename in filenames]
        files = {}
        for filename in binaries:
            relative = "bootstrap/" + ("" if filename == "winhttp.dll" else "BepInEx/core/") + filename
            files[relative] = digest((integration / relative).read_bytes())
        packages.append(dict(ecosystem="bootstrap", name=name, version=version, license=license_id,
                             source=f"https://github.com/{repository}/archive/{revision}.tar.gz",
                             files=files, notices=notices))

    lock = json.loads((root / "runtime/Starframe.Runtime/packages.lock.json").read_text("utf-8"))
    dependencies = lock["dependencies"][".NETStandard,Version=v2.0"]
    cache = Path(os.environ.get("NUGET_PACKAGES", Path.home() / ".nuget/packages"))
    fetched = {}
    for filename in RUNTIME_FILES:
        if filename.startswith("Starframe."):
            continue
        name = filename.removesuffix(".dll")
        version = dependencies[name]["resolved"]
        directory = cache / name.lower() / version
        archive = directory / f"{name.lower()}.{version}.nupkg"
        verified = subprocess.check_output(["dotnet", "nuget", "verify", str(archive), "--all"], text=True, cwd=root / "runtime")
        if dependencies[name]["contentHash"] not in verified:
            raise ValueError(f"NuGet verified content hash differs from lock: {name}")
        archive_hash = base64.b64encode(hashlib.sha512(archive.read_bytes()).digest()).decode()
        relative = f"runtime/BepInEx/plugins/Starframe/{filename}"
        binary_hash = digest((integration / relative).read_bytes())
        with zipfile.ZipFile(archive) as package_zip:
            matches = [p for p in package_zip.namelist() if p.endswith("/" + filename)
                       and digest(package_zip.read(p)) == binary_hash]
        if not matches:
            raise ValueError(f"Bundled DLL does not match the locked NuGet package: {name}")
        metadata = ET.parse(directory / f"{name.lower()}.nuspec").getroot()
        repository = metadata.find(".//{*}repository").attrib
        license_id = metadata.find(".//{*}license").text
        repo = repository["url"].removeprefix("https://github.com/").removesuffix(".git")
        revision = repository["commit"]
        key = (repo, revision)
        license_file = "LICENSE" if repo == "dotnet/maintenance-packages" else "LICENSE.TXT"
        if key not in fetched:
            fetched[key] = github(repo, revision, license_file)
        notices = [notice(license_file, f'https://github.com/{repo}/blob/{revision}/{license_file}', fetched[key])]
        source = f"https://api.nuget.org/v3-flatcontainer/{name.lower()}/{version}/{name.lower()}.{version}.nupkg"
        notices.extend(notice(p.relative_to(directory).as_posix(), source, p.read_text("utf-8-sig"))
                       for p in notice_files(directory))
        packages.append(dict(ecosystem="nuget", name=name, version=version, license=license_id,
                             source=f"https://github.com/{repo}/archive/{revision}.tar.gz",
                             package=source, packageSha512=archive_hash, contentHash=dependencies[name]["contentHash"],
                             matchedAssets=matches, files={relative: binary_hash}, notices=notices))

    repo, rev = "tauri-apps/nsis-tauri-utils", "13d9edd27b69310e108d6fbd49f90992f8a05390"
    packages.append(dict(ecosystem="installer", name="nsis_tauri_utils", version="0.5.3",
                         license="Apache-2.0 OR MIT", source=f"https://github.com/{repo}/archive/{rev}.tar.gz",
                         notices=[notice(f, f"https://github.com/{repo}/blob/{rev}/{f}", github(repo, rev, f))
                                  for f in ("LICENSE_APACHE-2.0", "LICENSE_MIT")]))
    nsis = Path(os.environ["LOCALAPPDATA"]) / "tauri/NSIS"
    version = subprocess.check_output([str(nsis / "makensis.exe"), "/VERSION"]).decode().strip()
    if version != "v3.11":
        raise ValueError("Review NSIS version before refreshing notices")
    source = "https://sourceforge.net/projects/nsis/files/NSIS%203/3.11/nsis-3.11-src.tar.bz2/download"
    packages.append(dict(ecosystem="installer", name="NSIS", version="3.11",
                         license="Zlib; compression components have separate terms in COPYING",
                         source=source, notices=[notice("COPYING", source, (nsis / "COPYING").read_text("utf-8-sig"))]))
    return dict(schemaVersion=1, title="Starframe runtime and installer notices",
                scope="Pinned BepInEx bootstrap components, exact Microsoft runtime DLLs and NSIS. Starframe code, Rust standard-library notices and artwork/font notices are separate.",
                inputs={name: digest((root / name).read_text("utf-8").encode()) for name in ("runtime/bootstrap.json", "runtime/Starframe.Runtime/packages.lock.json")},
                packages=packages, texts=dict(sorted(texts.items())))


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    inventory = collect(root)
    (root / "docs/notices/runtime-dependencies.json").write_text(
        json.dumps(inventory, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    (root / "docs/notices/runtime-dependencies.txt").write_text(render(inventory), encoding="utf-8")
    print(f'Collected {len(inventory["packages"])} runtime/installer components.')
