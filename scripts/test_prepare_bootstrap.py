import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
import zipfile
from prepare_bootstrap import prepare


class BootstrapPreparation(unittest.TestCase):
    def test_verified_archive_and_rejected_hash_or_inventory(self):
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w") as archive:
            archive.writestr("BepInEx/core/example.dll", b"fixture")
        data = buffer.getvalue()
        manifest = {"sha256": hashlib.sha256(data).hexdigest(), "files": [{"path": "BepInEx/core/example.dll", "size": 7, "sha256": hashlib.sha256(b"fixture").hexdigest()}]}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prepare(root / "valid", data, manifest)
            self.assertEqual((root / "valid/BepInEx/core/example.dll").read_bytes(), b"fixture")
            for index, bad in enumerate([{**manifest, "sha256": "0" * 64}, {**manifest, "files": []}]):
                with self.assertRaises(ValueError):
                    prepare(root / str(index), data, bad)
                self.assertFalse((root / str(index)).exists())
            with self.assertRaises(FileExistsError):
                prepare(root / "valid", data, manifest)

    def test_pinned_loader_inventory_has_no_runtime_mod_payloads(self):
        root = Path(__file__).resolve().parents[1]
        manifest = json.loads((root / "runtime/bootstrap.json").read_text(encoding="utf-8"))
        paths = {file["path"] for file in manifest["files"]}
        self.assertIn("BepInEx/core/BepInEx.Preloader.dll", paths)
        self.assertFalse(any(path.startswith(("BepInEx/plugins/", "BepInEx/patchers/")) for path in paths))
