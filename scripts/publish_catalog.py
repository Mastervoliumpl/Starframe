"""Prepare and verify a static TUF catalog using the pinned tuftool publisher."""

import argparse
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


def run(*args: str | Path) -> None:
    subprocess.run([str(arg) for arg in args], check=True)


def metadata(path: Path, limit: int = 262144) -> dict:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise ValueError(f"Expected bounded metadata file: {path.name}")
    return json.loads(path.read_bytes())["signed"]


def publish(
    *,
    source: Path,
    root: Path,
    key: Path,
    output: Path,
    previous: Path | None,
    tuftool: Path,
    validator: Path,
    signer: Path,
    previous_root: Path | None = None,
) -> None:
    if output.exists():
        raise ValueError("Publication output must be a new directory.")
    run(validator, source / "releases.json")
    now = datetime.now(timezone.utc).replace(microsecond=0)
    expires = now + timedelta(days=30)
    trusted = metadata(root, 65536)
    if trusted["consistent_snapshot"] is not True:
        raise ValueError("Catalog publication requires versioned metadata and hashed target filenames.")
    if datetime.fromisoformat(trusted["expires"].replace("Z", "+00:00")) <= expires:
        raise ValueError("Rotate the trust root before it has fewer than thirty days left.")
    root_keys = set(trusted["roles"]["root"]["keyids"])
    if any(root_keys.intersection(trusted["roles"][role]["keyids"]) for role in ("targets", "snapshot", "timestamp")):
        raise ValueError("The offline root key must be separate from the online catalog key.")
    if previous_root is not None and (
        previous is None or metadata(previous_root, 65536)["version"] >= trusted["version"]
    ):
        raise ValueError("Rotation requires a previous publication and an older reviewed trust root.")
    version = max(1, int(now.timestamp()))
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="catalog-publish-", dir=output.parent) as temporary:
        work = Path(temporary)
        if previous is not None:
            run(tuftool, "download", "--allow-expired-repo", "--root", previous_root or root,
                "--metadata-url", (previous / "metadata").resolve().as_uri() + "/",
                "--targets-url", (previous / "targets").resolve().as_uri() + "/",
                work / "previous")
            run(validator, source / "releases.json", "--previous-directory", work / "previous")
            timestamp = metadata(previous / "metadata/timestamp.json", 65536)
            snapshot_version = timestamp["meta"]["snapshot.json"]["version"]
            snapshot = metadata(previous / "metadata" / f"{snapshot_version}.snapshot.json")
            version = max(version, timestamp["version"] + 1, snapshot_version + 1,
                          snapshot["meta"]["targets.json"]["version"] + 1)
        inputs = work / "input"
        inputs.mkdir()
        for name, source_name in (("catalog.json", "releases.json"), ("advisories.json", "advisories.json")):
            shutil.copyfile(source / source_name, inputs / name)
        expiry = expires.isoformat().replace("+00:00", "Z")
        candidate = work / "candidate"
        run(signer, root, key, inputs, candidate, str(version), expiry)
        if previous is not None:
            for retained in (previous / "metadata").glob("*.root.json"):
                number = retained.name.removesuffix(".root.json")
                if number.isdecimal() and number == str(int(number)) and 0 < int(number) < trusted["version"]:
                    metadata(retained, 65536)
                    shutil.copyfile(retained, candidate / "metadata" / retained.name)
        if previous_root is not None:
            run(tuftool, "download", "--root", previous_root,
                "--metadata-url", (candidate / "metadata").resolve().as_uri() + "/",
                "--targets-url", (candidate / "targets").resolve().as_uri() + "/", work / "rotated")
        verified = work / "verified"
        run(tuftool, "download", "--root", root,
            "--metadata-url", (candidate / "metadata").resolve().as_uri() + "/",
            "--targets-url", (candidate / "targets").resolve().as_uri() + "/", verified)
        for name in ("catalog.json", "advisories.json"):
            if (verified / name).read_bytes() != (inputs / name).read_bytes():
                raise ValueError("Verified targets differ from the reviewed input.")
        shutil.copytree(candidate, output, symlinks=False)
    print(f"Verified catalog metadata version {version}; expires {expiry}. Output is ready for publication.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("catalog"))
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--key", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--previous", type=Path)
    parser.add_argument("--previous-root", type=Path, help="Reviewed old public root for verifying a key rotation")
    parser.add_argument("--tuftool", type=Path, required=True)
    parser.add_argument("--validator", type=Path, required=True)
    parser.add_argument("--signer", type=Path, required=True)
    publish(**vars(parser.parse_args()))
