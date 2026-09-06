"""Prepare pinned BepInEx bootstrap files in a new directory; never install them."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
import zipfile
import io


def prepare(destination: Path, archive: bytes, manifest: dict) -> None:
    if hashlib.sha256(archive).hexdigest() != manifest["sha256"]:
        raise ValueError("BepInEx archive hash mismatch")
    expected = {item["path"]: item for item in manifest["files"]}
    with zipfile.ZipFile(io.BytesIO(archive)) as source:
        files = [item for item in source.infolist() if not item.is_dir()]
        if len(files) != len(expected) or {item.filename for item in files} != set(expected):
            raise ValueError("BepInEx archive inventory mismatch")
        payload = []
        for item in files:
            path = Path(item.filename)
            if path.is_absolute() or ".." in path.parts or "\\" in item.filename or ":" in item.filename:
                raise ValueError("Invalid bootstrap archive path")
            metadata = expected[item.filename]
            if item.file_size != metadata["size"] or item.file_size > 8_388_608:
                raise ValueError("Bootstrap entry size mismatch")
            content = source.read(item)
            if hashlib.sha256(content).hexdigest() != metadata["sha256"]:
                raise ValueError("Bootstrap entry hash mismatch")
            payload.append((path, content))
    destination.mkdir(parents=True, exist_ok=False)
    for path, content in payload:
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as output:
            output.write(content)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--archive", type=Path, help="Use a previously downloaded archive")
    args = parser.parse_args()
    manifest = json.loads((Path(__file__).resolve().parents[1] / "runtime/bootstrap.json").read_text(encoding="utf-8"))
    if args.archive:
        archive = args.archive.read_bytes()
    else:
        with urllib.request.urlopen(manifest["url"], timeout=30) as response:
            archive = response.read(4_194_305)
    if len(archive) > 4_194_304:
        raise ValueError("Bootstrap archive exceeds its size limit")
    prepare(args.destination, archive, manifest)
    print("Verified bootstrap prepared at", args.destination)


if __name__ == "__main__":
    main()
