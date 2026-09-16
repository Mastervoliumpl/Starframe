import base64
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from release_artifacts import metadata, names, prepare, verify
from release_sources import source_records


class ReleaseArtifactChecks(unittest.TestCase):
    @patch("release_artifacts.verify_signature")
    def test_signed_inventory_and_metadata_reject_replacement(self, signature_check):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            release = dict(version="0.6.0-dev.1", tag="v0.6.0-dev.1", notes="Fixture", commit="a" * 40)
            artifacts = names(release["version"])
            for name in artifacts.values():
                (root / name).write_bytes(b"inert " + name.encode())
            output = root / "release"
            prepare(release, root / artifacts["installer"], root, output)
            (output / (artifacts["installer"] + ".sig")).write_text(base64.b64encode(b"fixture").decode())
            key = base64.b64encode(b"fixture key").decode()
            metadata(output, key)
            (output / "SHA256SUMS.sig").write_text("fixture")
            verify(output, key, root / "verifier", release)
            self.assertEqual(signature_check.call_count, 2)
            latest = output / "latest.json"
            original = latest.read_bytes()
            latest.write_text(original.decode().replace("github.com", "example.com"))
            with self.assertRaises(ValueError):
                verify(output, key, root / "verifier", release)
            latest.write_bytes(original)
            with self.assertRaises(ValueError):
                verify(output, "another key", root / "verifier", release)
            with self.assertRaises(ValueError):
                verify(output, key, root / "verifier", release | {"commit": "b" * 40})
            (output / "private.key").write_text("must never upload")
            with self.assertRaises(ValueError):
                verify(output, key, root / "verifier", release)

    def test_source_inventory_covers_reviewed_copyleft_and_installer_sources(self):
        root = Path(__file__).resolve().parents[1]
        records = source_records(root)
        self.assertEqual({item["name"] for item in records},
                         {"cssparser", "cssparser-macros", "dtoa-short", "option-ext", "selectors", "UnityDoorstop", "NSIS"})
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory)
            (fixture / "scripts").mkdir()
            (fixture / "docs/notices").mkdir(parents=True)
            for name in ("scripts/release-sources.json", "docs/notices/desktop-dependencies.json", "docs/notices/runtime-dependencies.json"):
                (fixture / name).write_bytes((root / name).read_bytes())
            manifest = fixture / "scripts/release-sources.json"
            changed = json.loads(manifest.read_text())
            changed["UnityDoorstop"]["version"] = "unreviewed"
            manifest.write_text(json.dumps(changed))
            with self.assertRaises(ValueError):
                source_records(fixture)


if __name__ == "__main__":
    unittest.main()
