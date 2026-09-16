import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from release_runtime import ARCHIVE_FILES, digest, pack, verify, RUNTIME_FILES


class RuntimeReleaseChecks(unittest.TestCase):
    @patch("release_runtime.inputs", return_value={"runtime/source.cs": "reviewed"})
    @patch("release_runtime.read_version", return_value="0.6.0-dev.1")
    def test_closed_inventory_source_binding_and_tampering(self, *_):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "build"
            runtime.mkdir()
            (root / "docs/notices").mkdir(parents=True)
            (root / "LICENSE").write_text("Fixture project license")
            (root / "docs/notices/runtime-dependencies.txt").write_text("Fixture dependency notices")
            for name in RUNTIME_FILES:
                (runtime / name).write_bytes(b"inert " + name.encode())
            (runtime / "Trebuchet.dll").write_bytes(b"must not be included")
            archive, manifest = root / "runtime.zip", root / "runtime.json"
            pack(root, runtime, archive, manifest)
            verify(root, archive, manifest, root / "accepted")
            self.assertEqual({p.name for p in (root / "accepted").iterdir()}, set(ARCHIVE_FILES))
            original = manifest.read_text()
            record = json.loads(original)
            record["inputs"] = {"runtime/source.cs": "changed"}
            manifest.write_text(json.dumps(record))
            with self.assertRaises(ValueError):
                verify(root, archive, manifest, root / "wrong-source")
            manifest.write_text(original)
            with zipfile.ZipFile(archive, "a") as package:
                package.writestr("../escape.dll", b"unexpected")
            with self.assertRaises(ValueError):
                verify(root, archive, manifest, root / "changed")
            record = json.loads(original)
            record["archiveSha256"] = digest(archive.read_bytes())
            manifest.write_text(json.dumps(record))
            with self.assertRaises(ValueError):
                verify(root, archive, manifest, root / "bad-inventory")
            self.assertFalse((root / "escape.dll").exists())


if __name__ == "__main__":
    unittest.main()
