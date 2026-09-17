"""Retain matching Starframe and required component source archives for a release."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import urllib.request
import zipfile


def source_records(root):
    pinned = json.loads((root / "scripts/release-sources.json").read_text(encoding="utf-8"))
    records = []
    for kind in ("desktop", "runtime"):
        inventory = json.loads((root / f"docs/notices/{kind}-dependencies.json").read_text(encoding="utf-8"))
        for package in inventory["packages"]:
            if not any(term in package["license"] for term in ("MPL", "LGPL", "GPL", "CPL")) and package["name"] != "NSIS":
                continue
            if package["ecosystem"] == "cargo":
                record = dict(download=package["source"], sha256=package["sourceSha256"],
                              filename=f"{package['name']}-{package['version']}.crate")
            else:
                record = pinned[package["name"]].copy()
                if record["version"] != package["version"] or record["source"] != package["source"]:
                    raise ValueError("Review corresponding-source pins after updating a dependency")
            if not re.fullmatch(r"[A-Za-z0-9_.-]+", record["filename"]) or not re.fullmatch(r"[0-9a-f]{64}", record["sha256"]):
                raise ValueError("Invalid source archive filename or hash")
            records.append(record | {key: package[key] for key in ("name", "version", "license", "source")})
    if len({record["filename"] for record in records}) != len(records):
        raise ValueError("Source archive filenames must be unique")
    return sorted(records, key=lambda record: record["filename"])


def fetch(record):
    if not record["download"].startswith("https://"):
        raise ValueError("Source downloads require HTTPS")
    with urllib.request.urlopen(record["download"], timeout=45) as response:
        if not response.url.startswith("https://"):
            raise ValueError("Source download redirected outside HTTPS")
        data = response.read(64 * 1024 * 1024 + 1)
    if len(data) > 64 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != record["sha256"]:
        raise ValueError(f"Source archive hash/size mismatch: {record['filename']}")
    return data


def prepare(root, release, output):
    output.mkdir(parents=True, exist_ok=False)
    prefix = f"Starframe_{release['version']}"
    source = output / f"{prefix}_source.zip"
    subprocess.run(["git", "-C", str(root), "archive", "--format=zip",
                    f"--prefix={prefix}/", f"--output={source.resolve()}", release["commit"]], check=True)
    records = source_records(root)
    with zipfile.ZipFile(output / f"{prefix}_component-sources.zip", "x", compression=zipfile.ZIP_DEFLATED) as archive:
        payload = {record["filename"]: fetch(record) for record in records}
        payload["sources.json"] = (json.dumps(records, indent=2) + "\n").encode()
        for name, data in sorted(payload.items()):
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, data)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    prepare(Path(__file__).resolve().parents[1],
            json.loads(args.release.read_text(encoding="utf-8")), args.output)
    print("Matching project source and hash-verified component sources retained.")
