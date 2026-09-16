"""Bind a locally built runtime to reviewed source and a closed DLL inventory."""

import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import zipfile

from prepare_desktop_runtime import RUNTIME_FILES
from versions import read_version

ARCHIVE_FILES = (*RUNTIME_FILES, "LICENSE.txt", "runtime-notices.txt")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def inputs(root):
    names = subprocess.check_output(
        ["git", "-C", str(root), "ls-files", "-z", "runtime", "scripts/release_runtime.py",
         "scripts/build_release_runtime.ps1", "scripts/prepare_desktop_runtime.py",
         "LICENSE", "docs/notices/runtime-dependencies.txt"],
    ).decode().split("\0")
    result = {}
    for name in sorted(filter(None, names)):
        data = (root / name).read_bytes()
        try:
            data = data.decode("utf-8").replace("\r\n", "\n").encode("utf-8")
        except UnicodeDecodeError:
            pass
        result[name] = digest(data)
    return result


def pack(root, runtime, output, manifest):
    if output.exists() or manifest.exists():
        raise ValueError("Use new output paths; existing runtime evidence was retained")
    payload = {name: (runtime / name).read_bytes() for name in RUNTIME_FILES}
    payload["LICENSE.txt"] = (root / "LICENSE").read_bytes()
    payload["runtime-notices.txt"] = (root / "docs/notices/runtime-dependencies.txt").read_bytes()
    if any(not data or len(data) > 8_388_608 for data in payload.values()):
        raise ValueError("Runtime DLL missing or outside its supported size")
    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in sorted(payload.items()):
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, data)
    record = dict(schemaVersion=1, version=read_version(root), inputs=inputs(root),
                  archiveSha256=digest(output.read_bytes()),
                  files={name: digest(data) for name, data in payload.items()})
    manifest.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


def verify(root, archive, manifest, output):
    record = json.loads(manifest.read_text(encoding="utf-8"))
    if record["schemaVersion"] != 1 or record["version"] != read_version(root) or record["inputs"] != inputs(root):
        raise ValueError("Runtime package does not match this version and source inventory")
    if archive.stat().st_size > 64 * 1024 * 1024:
        raise ValueError("Runtime archive exceeds its limit")
    data = archive.read_bytes()
    if digest(data) != record["archiveSha256"] or set(record["files"]) != set(ARCHIVE_FILES):
        raise ValueError("Runtime archive or DLL inventory differs from its reviewed manifest")
    payload = {}
    with zipfile.ZipFile(io.BytesIO(data)) as package:
        if len(package.infolist()) != len(ARCHIVE_FILES) or set(package.namelist()) != set(ARCHIVE_FILES):
            raise ValueError("Runtime archive contains unexpected or duplicate entries")
        for entry in package.infolist():
            if not 0 < entry.file_size <= 8_388_608:
                raise ValueError("Runtime entry exceeds its limit")
            content = package.read(entry)
            if digest(content) != record["files"][entry.filename]:
                raise ValueError("Runtime DLL differs from its reviewed hash")
            payload[entry.filename] = content
    output.mkdir(parents=True, exist_ok=False)
    for name, content in payload.items():
        (output / name).write_bytes(content)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("pack", "verify"))
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if args.mode == "pack":
        pack(root, args.runtime, args.archive, args.manifest)
    else:
        verify(root, args.archive, args.manifest, args.runtime)
    print("Runtime source and DLL inventory verified; proprietary reference assemblies are excluded.")
