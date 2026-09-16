"""Assemble and verify the exact files attached to a Starframe draft release."""

import argparse
import base64
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

REPOSITORY_URL = "https://github.com/Mastervoliumpl/Starframe"


def names(version):
    prefix = f"Starframe_{version}"
    return {"installer": f"{prefix}_x64-setup.exe", "source": f"{prefix}_source.zip",
            "components": f"{prefix}_component-sources.zip"}


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def prepare(release, installer, sources, output):
    artifacts = names(release["version"])
    if installer.name != artifacts["installer"]:
        raise ValueError("Installer filename differs from the release version")
    output.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(installer, output / artifacts["installer"])
    for kind in ("source", "components"):
        shutil.copyfile(sources / artifacts[kind], output / artifacts[kind])
    write_json(output / "release.json", release)


def verify_input(output, expected):
    if json.loads((output / "release.json").read_text(encoding="utf-8")) != expected:
        raise ValueError("Unsigned artifact identity differs from the checked source")
    if {path.name for path in output.iterdir()} != set(names(expected["version"]).values()) | {"release.json"}:
        raise ValueError("Unsigned release inventory is incomplete or contains unexpected files")
    if any(not path.is_file() or path.is_symlink() for path in output.iterdir()):
        raise ValueError("Unsigned release inputs must be ordinary files")


def metadata(output, public_key):
    release = json.loads((output / "release.json").read_text(encoding="utf-8"))
    installer = names(release["version"])["installer"]
    signature = (output / (installer + ".sig")).read_text(encoding="utf-8").strip()
    write_json(output / "latest.json", {
        "version": release["version"], "notes": release["notes"],
        "platforms": {"windows-x86_64-nsis": {
            "signature": signature,
            "url": f"{REPOSITORY_URL}/releases/download/{release['tag']}/{installer}"}},
    })
    (output / "UPDATE_PUBLIC_KEY.txt").write_text(public_key.strip() + "\n", encoding="utf-8")
    records = []
    for path in sorted(output.iterdir(), key=lambda path: path.name):
        if not path.is_file() or path.name in ("SHA256SUMS", "SHA256SUMS.sig"):
            raise ValueError("Expected new ordinary release files before checksum generation")
        records.append(hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.name)
    (output / "SHA256SUMS").write_text("\n".join(records) + "\n", encoding="utf-8")


def verify_signature(verifier, public_key, signature, artifact):
    with tempfile.TemporaryDirectory(prefix="starframe-verify-") as directory:
        root = Path(directory)
        key, sig = root / "public.txt", root / "signature.txt"
        key.write_bytes(base64.b64decode(public_key.strip(), validate=True))
        sig.write_bytes(base64.b64decode(signature.strip(), validate=True))
        subprocess.run([str(verifier), str(key), str(sig), str(artifact)], check=True)


def verify(output, public_key, verifier, expected):
    release = json.loads((output / "release.json").read_text(encoding="utf-8"))
    if release != expected:
        raise ValueError("Release identity differs from the checked build")
    artifacts = names(release["version"])
    installer = artifacts["installer"]
    files = set(artifacts.values()) | {installer + ".sig", "release.json", "latest.json", "UPDATE_PUBLIC_KEY.txt"}
    if {p.name for p in output.iterdir()} != files | {"SHA256SUMS", "SHA256SUMS.sig"}:
        raise ValueError("Draft artifact inventory is incomplete or contains unexpected files")
    if any(not p.is_file() or p.is_symlink() for p in output.iterdir()):
        raise ValueError("Draft artifacts must be ordinary files")
    if (output / "UPDATE_PUBLIC_KEY.txt").read_text(encoding="utf-8").strip() != public_key.strip():
        raise ValueError("Included public key differs from the independently trusted key")
    checksum_file = output / "SHA256SUMS"
    verify_signature(verifier, public_key, (output / "SHA256SUMS.sig").read_text(), checksum_file)
    expected_sums = "".join(hashlib.sha256((output / name).read_bytes()).hexdigest() + "  " + name + "\n" for name in sorted(files))
    if checksum_file.read_text(encoding="utf-8") != expected_sums:
        raise ValueError("A release file differs from the signed checksum inventory")
    signature = (output / (installer + ".sig")).read_text(encoding="utf-8").strip()
    verify_signature(verifier, public_key, signature, output / installer)
    latest = json.loads((output / "latest.json").read_text(encoding="utf-8"))
    if latest != {"version": release["version"], "notes": release["notes"],
                  "platforms": {"windows-x86_64-nsis": {
                      "signature": signature,
                      "url": f"{REPOSITORY_URL}/releases/download/{release['tag']}/{installer}"}}}:
        raise ValueError("Updater metadata does not match the selected release and installer")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("prepare", "metadata", "verify", "verify-input"))
    parser.add_argument("--release", type=Path)
    parser.add_argument("--installer", type=Path)
    parser.add_argument("--sources", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verifier", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    key = json.loads((root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["plugins"]["updater"]["pubkey"]
    if args.mode == "prepare":
        prepare(json.loads(args.release.read_text(encoding="utf-8")), args.installer, args.sources, args.output)
    elif args.mode == "metadata":
        metadata(args.output, key)
    elif args.mode == "verify-input":
        verify_input(args.output, json.loads(args.release.read_text(encoding="utf-8")))
    else:
        verify(args.output, key, args.verifier, json.loads(args.release.read_text(encoding="utf-8")))
    print("Release artifact operation completed: " + args.mode)
