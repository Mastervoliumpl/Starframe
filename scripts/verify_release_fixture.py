"""Exercise draft artifact signing with disposable keys and inert installer bytes."""

import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from release_artifacts import metadata, names, prepare, verify


def run(verifier):
    root = Path(__file__).resolve().parents[1]
    cli = root / "node_modules/@tauri-apps/cli/tauri.js"
    with tempfile.TemporaryDirectory(prefix="starframe-release-") as directory:
        fixture = Path(directory)
        key, other = fixture / "key", fixture / "other"
        for path in (key, other):
            subprocess.run(["node", str(cli), "signer", "generate", "--ci", "-w", str(path)],
                           capture_output=True, check=True, timeout=30)
        public = key.with_suffix(".pub").read_text().strip()
        release = dict(commit="a" * 40, version="0.6.0-dev.1", tag="v0.6.0-dev.1", prerelease=True, notes="Inert fixture")
        artifacts = names(release["version"])
        for name in artifacts.values():
            (fixture / name).write_bytes(b"inert fixture; never execute")
        output = fixture / "candidate"
        prepare(release, fixture / artifacts["installer"], fixture, output)

        def sign(path):
            subprocess.run(["node", str(cli), "signer", "sign", "-f", str(key), "-p", "", str(path)],
                           capture_output=True, check=True, timeout=30)

        sign(output / artifacts["installer"])
        metadata(output, public)
        sign(output / "SHA256SUMS")
        verify(output, public, verifier, release)

        def rejected(key_text=public):
            try:
                verify(output, key_text, verifier, release)
            except (ValueError, subprocess.CalledProcessError):
                return
            raise AssertionError("Changed release artifacts were accepted")

        installer = output / artifacts["installer"]
        original = installer.read_bytes()
        installer.write_bytes(original + b"changed")
        rejected()
        installer.write_bytes(original)
        checksum_signature = output / "SHA256SUMS.sig"
        signature = checksum_signature.read_bytes()
        checksum_signature.write_bytes(b"invalid")
        rejected()
        checksum_signature.write_bytes(signature)
        # A replacement key cannot authenticate a downloaded key file and its own bundle.
        wrong_public = other.with_suffix(".pub").read_text().strip()
        (output / "UPDATE_PUBLIC_KEY.txt").write_text(wrong_public + "\n")
        rejected(wrong_public)
        (output / "UPDATE_PUBLIC_KEY.txt").write_text(public + "\n")
        latest = output / "latest.json"
        value = json.loads(latest.read_text())
        value["version"] = "9.9.9"
        latest.write_text(json.dumps(value))
        rejected()
    print("Draft signatures passed; changed installer/metadata, invalid signature and replacement key were rejected.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verifier", type=Path, required=True)
    args = parser.parse_args()
    run(args.verifier.resolve(strict=True))
