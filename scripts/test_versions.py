import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

from versions import JSON_FIELDS, read_version, synchronize


class VersionChecks(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="starframe-versions-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.source = Path(__file__).resolve().parents[1]
        self.names = ["VERSION", *JSON_FIELDS, "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"]
        for name in self.names + ["scripts/versions.py"]:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(self.source / name, path)

    def contents(self):
        return {name: (self.root / name).read_bytes() for name in self.names}

    def run_cli(self, *args):
        return subprocess.run(
            [sys.executable, str(self.root / "scripts/versions.py"), *args],
            cwd=self.root,
            capture_output=True,
            text=True,
        )

    def test_valid_versions_and_invalid_input(self):
        for version in ("0.1.0-dev.1", "0.1.0-rc.2", "0.1.0", "1.2.3+build.01"):
            with self.subTest(version=version):
                (self.root / "VERSION").write_text(version + "\n", encoding="utf-8")
                self.assertEqual(read_version(self.root), version)
        for version in ("", "0.1", "v0.1.0", "01.1.0", "0.1.0-dev.01", "0.1.0-", "0.1.0+", "0.1.0\n1.2.3"):
            with self.subTest(version=version):
                (self.root / "VERSION").write_text(version, encoding="utf-8")
                before = self.contents()
                result = self.run_cli("--write")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("Invalid product version", result.stderr)
                self.assertEqual(self.contents(), before)

    def test_each_product_field_is_checked_and_repaired(self):
        for name, fields in JSON_FIELDS.items():
            for field in fields:
                with self.subTest(name=name, field=field):
                    document = json.loads((self.root / name).read_text(encoding="utf-8"))
                    parent = document
                    for key in field[:-1]:
                        parent = parent[key]
                    parent[field[-1]] = "9.9.9"
                    (self.root / name).write_text(json.dumps(document), encoding="utf-8")
                    before = self.contents()
                    errors = synchronize(self.root)
                    self.assertEqual(len(errors), 1)
                    self.assertIn(name, errors[0])
                    self.assertEqual(self.contents(), before)
                    synchronize(self.root, write=True)
                    self.assertEqual(synchronize(self.root), [])
        for name in ("src-tauri/Cargo.toml", "src-tauri/Cargo.lock"):
            with self.subTest(name=name):
                path = self.root / name
                text = path.read_text(encoding="utf-8")
                path.write_text(text.replace(
                    f'name = "starframe"\nversion = "{read_version(self.root)}"',
                    'name = "starframe"\nversion = "9.9.9"', 1
                ), encoding="utf-8")
                self.assertTrue(any(name in error for error in synchronize(self.root)))
                synchronize(self.root, write=True)
                self.assertEqual(synchronize(self.root), [])

    def test_prepare_preserves_dependencies_and_format_versions(self):
        original_npm = json.loads((self.root / "package-lock.json").read_text(encoding="utf-8"))
        original_cargo = tomllib.loads((self.root / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))
        for version in ("0.1.0-dev.2", "0.1.0-rc.1", "0.1.0"):
            with self.subTest(version=version):
                drift = version != read_version(self.root)
                (self.root / "VERSION").write_text(version + "\n", encoding="utf-8")
                before = self.contents()
                result = self.run_cli()
                self.assertEqual(result.returncode, 1 if drift else 0)
                if drift:
                    self.assertIn("differs from VERSION", result.stderr)
                self.assertEqual(self.contents(), before)
                result = self.run_cli("--write")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.run_cli().returncode, 0)
                after = self.contents()
                for name, fields in JSON_FIELDS.items():
                    for field in fields:
                        value = json.loads(after[name])
                        for key in field:
                            value = value[key]
                        self.assertEqual(value, version)
                self.assertEqual(tomllib.loads(after["src-tauri/Cargo.toml"].decode())["package"]["version"], version)
                self.assertEqual(self.run_cli("--write").returncode, 0)
                self.assertEqual(self.contents(), after)

                npm = json.loads(after["package-lock.json"])
                self.assertEqual(npm["lockfileVersion"], original_npm["lockfileVersion"])
                self.assertEqual(
                    {key: value for key, value in npm["packages"].items() if key},
                    {key: value for key, value in original_npm["packages"].items() if key},
                )
                cargo = tomllib.loads(after["src-tauri/Cargo.lock"].decode())
                self.assertEqual(next(p["version"] for p in cargo["package"] if p["name"] == "starframe"), version)
                self.assertEqual(cargo["version"], original_cargo["version"])
                self.assertEqual(
                    [p for p in cargo["package"] if p["name"] != "starframe"],
                    [p for p in original_cargo["package"] if p["name"] != "starframe"],
                )

    def test_missing_or_malformed_manifest_prevents_all_writes(self):
        (self.root / "VERSION").write_text("0.1.0-rc.1\n", encoding="utf-8")
        path = self.root / "src-tauri/Cargo.lock"
        for contents in ('version = 4\n', '[[package]]\nname = "starframe"\nversion = [', None):
            with self.subTest(contents=contents):
                if contents is None:
                    path.unlink()
                else:
                    path.write_text(contents, encoding="utf-8")
                before = {name: (self.root / name).read_bytes() for name in self.names if name != "src-tauri/Cargo.lock"}
                result = self.run_cli("--write")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("Cargo.lock", result.stderr)
                self.assertEqual(
                    {name: (self.root / name).read_bytes() for name in before}, before
                )

    def test_failed_replacement_leaves_parseable_files_and_can_be_retried(self):
        (self.root / "VERSION").write_text("0.1.0-rc.1\n", encoding="utf-8")
        before = self.contents()
        with patch("versions.os.replace", side_effect=OSError("fixture: write denied")):
            with self.assertRaises(OSError):
                synchronize(self.root, write=True)
        self.assertEqual(self.contents(), before)
        self.assertEqual(list(self.root.rglob(".version-*")), [])
        synchronize(self.root, write=True)
        self.assertEqual(synchronize(self.root), [])

    def test_invalid_json_fields_prevent_writes(self):
        (self.root / "VERSION").write_text("0.1.0-rc.1\n", encoding="utf-8")
        path = self.root / "src-tauri/tauri.conf.json"
        for text in ('{', '{}', '{"version": 1}', '{"version": null}'):
            with self.subTest(text=text):
                path.write_text(text, encoding="utf-8")
                before = self.contents()
                result = self.run_cli("--write")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("tauri.conf.json", result.stderr)
                self.assertEqual(self.contents(), before)

    def test_cargo_comments_and_same_named_registry_package_survive(self):
        path = self.root / "src-tauri/Cargo.lock"
        text = path.read_text(encoding="utf-8")
        registry = '\n[[package]]\nname = "starframe"\nversion = "9.9.9"\nsource = "registry+https://example.invalid"\n'
        path.write_text(text + registry, encoding="utf-8")
        manifest = self.root / "src-tauri/Cargo.toml"
        original = manifest.read_text(encoding="utf-8").replace(
            f'version = "{read_version(self.root)}"', 'version = "9.9.9" # product version'
        )
        manifest.write_text(original, encoding="utf-8")
        (self.root / "VERSION").write_text("0.1.0-rc.1\n", encoding="utf-8")
        synchronize(self.root, write=True)
        self.assertTrue(path.read_text(encoding="utf-8").endswith(registry))
        self.assertEqual(
            manifest.read_text(encoding="utf-8"),
            original.replace('"9.9.9" # product version', '"0.1.0-rc.1" # product version'),
        )


if __name__ == "__main__":
    unittest.main()
