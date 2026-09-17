import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from check_installer import check
from prepare_desktop_runtime import prepare, RUNTIME_FILES


class InstallerChecks(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix='starframe-installer-')
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / 'VERSION').write_text('0.6.0-dev.1', encoding='utf-8')
        (self.root / 'package-lock.json').write_text(json.dumps({
            'packages': {'node_modules/@tauri-apps/cli': {'version': '2.11.4'}}
        }), encoding='utf-8')
        (self.root / 'runtime').mkdir()
        (self.root / 'runtime/bootstrap.json').write_text(json.dumps({'files': [
            {'path': 'winhttp.dll', 'sha256': hashlib.sha256(b'bootstrap fixture').hexdigest()}
        ]}), encoding='utf-8')
        runtime = self.root / 'build'
        runtime.mkdir()
        for name in RUNTIME_FILES:
            (runtime / name).write_bytes(b'runtime fixture')
        archive = self.root / 'bootstrap.zip'
        archive.write_bytes(b'fixture')
        self.integration = self.root / 'integration'
        with patch('prepare_desktop_runtime.prepare_bootstrap'):
            prepare(self.integration, runtime, archive)
        (self.integration / 'bootstrap').mkdir()
        (self.integration / 'bootstrap/winhttp.dll').write_bytes(b'bootstrap fixture')

    def test_prepared_payload_passes_and_altered_bootstrap_is_rejected(self):
        check(self.root, self.integration)
        (self.integration / 'bootstrap/winhttp.dll').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            check(self.root, self.integration)

    def test_extra_game_reference_and_missing_dependency_are_rejected(self):
        extra = self.integration / 'runtime/UnityEngine.dll'
        extra.write_bytes(b'proprietary reference fixture')
        with self.assertRaisesRegex(ValueError, 'unexpected files'):
            check(self.root, self.integration)
        extra.unlink()
        (self.integration / 'runtime/BepInEx/plugins/Starframe/System.Text.Json.dll').unlink()
        with self.assertRaisesRegex(ValueError, 'missing or unexpected'):
            check(self.root, self.integration)

    def test_nonempty_activation_is_rejected_even_with_matching_hash(self):
        activation = self.integration / 'runtime/Starframe/activation.json'
        document = json.loads(activation.read_text())
        document['installedMods'] = [{'modId': 'fixture', 'name': 'Fixture', 'version': '1'}]
        activation.write_text(json.dumps(document))
        inventory = self.integration / 'runtime/runtime-package.json'
        entries = json.loads(inventory.read_text())
        for entry in entries:
            if entry['path'] == 'Starframe/activation.json':
                entry['sha256'] = hashlib.sha256(activation.read_bytes()).hexdigest()
        inventory.write_text(json.dumps(entries))
        with self.assertRaisesRegex(ValueError, 'empty initial activation'):
            check(self.root, self.integration)

    def test_inventory_cannot_substitute_or_duplicate_files(self):
        inventory = self.integration / 'runtime/runtime-package.json'
        original = json.loads(inventory.read_text())
        for bad_path in ('../../outside.dll', 'BepInEx/plugins/Starframe/UnityEngine.dll', original[1]['path']):
            with self.subTest(path=bad_path):
                entries = [dict(item) for item in original]
                entries[0]['path'] = bad_path
                inventory.write_text(json.dumps(entries))
                with self.assertRaisesRegex(ValueError, 'supported runtime files'):
                    check(self.root, self.integration)
