import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import json
from prepare_desktop_runtime import prepare, RUNTIME_FILES


class DesktopRuntimeTests(unittest.TestCase):
    def test_staging_excludes_game_and_probe_libraries_and_rejects_reuse(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            runtime = root / 'build'
            runtime.mkdir()
            for name in (*RUNTIME_FILES, 'UnityEngine.dll', 'Starframe.UiProbe.dll'):
                (runtime / name).write_bytes(b'fixture')
            archive = root / 'archive.zip'
            archive.write_bytes(b'archive fixture')
            destination = root / 'integration'
            with patch('prepare_desktop_runtime.prepare_bootstrap') as bootstrap:
                prepare(destination, runtime, archive)
                bootstrap.assert_called_once()
            files = json.loads((destination / 'runtime/runtime-package.json').read_text())
            self.assertEqual(len(files), len(RUNTIME_FILES) + 1)
            self.assertFalse(any('UnityEngine' in f['path'] or 'UiProbe' in f['path'] for f in files))
            activation = json.loads((destination / 'runtime/Starframe/activation.json').read_text())
            self.assertEqual(activation['mods'], [])
            with self.assertRaisesRegex(ValueError, 'existing files'):
                prepare(destination, runtime, archive)

    def test_missing_runtime_fails_before_staging(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            destination = root / 'integration'
            with self.assertRaises(FileNotFoundError):
                prepare(destination, root, root / 'missing.zip')
            self.assertFalse(destination.exists())
