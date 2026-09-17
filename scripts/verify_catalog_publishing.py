"""Exercise catalog publication with temporary RSA keys and real TUF signatures."""

import argparse
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import subprocess
import tempfile

from publish_catalog import metadata, publish, run


def verify(tuftool: Path, validator: Path, signer: Path, work: Path) -> None:
    work.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="publishing-fixture-", dir=work) as temporary:
        fixture = Path(temporary).resolve()
        root = fixture / "root.json"
        offline = fixture / "offline.pem"
        online = fixture / "online.pem"
        run(tuftool, "root", "init", root)
        run(tuftool, "root", "expire", root, "in 365 days")
        run(tuftool, "root", "gen-rsa-key", root, offline, "--role", "root")
        run(tuftool, "root", "gen-rsa-key", root, online,
            "--role", "targets", "--role", "snapshot", "--role", "timestamp")
        for role in ("root", "targets", "snapshot", "timestamp"):
            run(tuftool, "root", "set-threshold", root, role, "1")
        run(tuftool, "root", "sign", root, "--key", offline)
        source = fixture / "source"
        source.mkdir()
        catalog = {"schemaVersion": 1, "catalogRevision": "1", "mods": []}
        advisories = {"schemaVersion": 1, "revision": "1", "advisories": []}
        (source / "releases.json").write_text(json.dumps(catalog))
        (source / "advisories.json").write_text(json.dumps(advisories))
        options = dict(source=source, root=root, key=online, tuftool=tuftool, validator=validator, signer=signer)
        first = fixture / "first"
        before = datetime.now(timezone.utc)
        publish(**options, output=first, previous=None)
        timestamp = metadata(first / "metadata/timestamp.json")
        expiry = datetime.fromisoformat(timestamp["expires"].replace("Z", "+00:00"))
        assert timedelta(days=30) - timedelta(seconds=1) <= expiry - before <= timedelta(days=30)
        expected_bytes = {
            path.relative_to(first): path.read_bytes()
            for folder in ("metadata", "targets") for path in (first / folder).iterdir()
        }
        run("git", "init", first)
        run("git", "-C", first, "-c", "core.autocrlf=true", "add", "--", ".gitattributes", "metadata", "targets")
        identity = ("-c", "user.name=Catalog fixture", "-c", "user.email=fixture@example.invalid")
        run("git", "-C", first, *identity, "commit", "-m", "Signed fixture")
        protected = fixture / "protected-checkout"
        run("git", "-c", "core.autocrlf=true", "clone", first, protected)
        for name, content in expected_bytes.items():
            assert (protected / name).read_bytes() == content

        # Older publications have no attributes; the workflow must also preserve their bytes.
        run("git", "-C", first, "rm", "--", ".gitattributes")
        run("git", "-C", first, *identity, "commit", "-m", "Legacy publication fixture")
        changed = fixture / "converted-checkout"
        run("git", "-c", "core.autocrlf=true", "clone", first, changed)
        assert any((changed / name).read_bytes() != content for name, content in expected_bytes.items())
        preserved = fixture / "preserved-checkout"
        run("git", "-c", "core.autocrlf=false", "clone", first, preserved)
        for name, content in expected_bytes.items():
            assert (preserved / name).read_bytes() == content
        second = fixture / "second"
        publish(**options, output=second, previous=preserved)
        assert metadata(second / "metadata/timestamp.json")["version"] > timestamp["version"]
        assert json.loads((source / "releases.json").read_text()) == catalog
        assert json.loads((source / "advisories.json").read_text()) == advisories

        def rejected(**changes: object) -> None:
            candidate = fixture / "rejected"
            try:
                publish(**(options | {"previous": second} | changes), output=candidate)
            except (ValueError, subprocess.CalledProcessError):
                assert not candidate.exists()
            else:
                raise AssertionError("Publication unexpectedly succeeded")

        rejected(key=offline)
        (source / "advisories.json").write_text(json.dumps(advisories | {"revision": "0"}))
        rejected()
        (source / "advisories.json").write_text(json.dumps(advisories))
        target = next((second / "targets").glob("*.advisories.json"))
        original = target.read_bytes()
        target.write_bytes(b"tampered")
        rejected()
        target.write_bytes(original)

        expired = fixture / "expired"
        inputs = fixture / "input"
        inputs.mkdir()
        (inputs / "catalog.json").write_text(json.dumps(catalog))
        (inputs / "advisories.json").write_text(json.dumps(advisories))
        run(signer, root, online, inputs, expired, "1", "2000-01-01T00:00:00Z")
        publish(**options, output=fixture / "recovered", previous=expired)

        previous_root = fixture / "previous-root.json"
        previous_root.write_bytes(root.read_bytes())
        old_online_id = metadata(root)["roles"]["targets"]["keyids"][0]
        replacement = fixture / "replacement.pem"
        run(tuftool, "root", "bump-version", root)
        run(tuftool, "root", "remove-key", root, old_online_id)
        run(tuftool, "root", "gen-rsa-key", root, replacement,
            "--role", "targets", "--role", "snapshot", "--role", "timestamp")
        run(tuftool, "root", "sign", root, "--key", offline)
        rotated = fixture / "rotated"
        rotated_options = options | {"key": replacement, "previous_root": previous_root}
        publish(**rotated_options, output=rotated, previous=second)
        assert (rotated / "metadata/1.root.json").read_bytes() == previous_root.read_bytes()
        rejected(previous_root=previous_root)

        second_root = fixture / "second-root.json"
        second_root.write_bytes(root.read_bytes())
        old_offline_id = metadata(root)["roles"]["root"]["keyids"][0]
        replacement_offline = fixture / "replacement-offline.pem"
        run(tuftool, "root", "bump-version", root)
        run(tuftool, "root", "remove-key", root, old_offline_id)
        run(tuftool, "root", "gen-rsa-key", root, replacement_offline, "--role", "root")
        run(tuftool, "root", "sign", root, "--key", replacement_offline)
        rejected(key=replacement, previous_root=previous_root, previous=rotated)
        run(tuftool, "root", "sign", root, "--key", offline, "--cross-sign", second_root)
        publish(**rotated_options, output=fixture / "rotated-again", previous=rotated)
    print("Catalog publication passed: exact Git checkout bytes, thirty-day renewal, increasing metadata versions, unchanged targets, wrong/revoked-key rejection, invalid/tampered input rejection, expired-repository recovery and online/offline key rotation from an older client root.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tuftool", type=Path, required=True)
    parser.add_argument("--validator", type=Path, required=True)
    parser.add_argument("--signer", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("test-results/catalog-publishing"))
    verify(**vars(parser.parse_args()))
