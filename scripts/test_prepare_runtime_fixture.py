import json
from pathlib import Path
import tempfile
import unittest
from prepare_runtime_fixture import prepare


class RuntimeFixturePreparation(unittest.TestCase):
    def test_package_inventory_and_failure_variant(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / 'runtime'
            runtime.mkdir()
            for name in ['Starframe.Bootstrap.dll', 'Starframe.Runtime.dll', 'System.Text.Json.dll', 'System.Memory.dll', 'System.Buffers.dll', 'System.Threading.Tasks.Extensions.dll']:
                (runtime / name).write_bytes(b'fixture')
            fixture = root / 'Starframe.FixtureMods.dll'
            fixture.write_bytes(b'fixture')
            for fail in [False, True]:
                output = root / str(fail)
                prepare(output, runtime, fixture, fail)
                activation = json.loads((output / 'Starframe/activation.json').read_text())
                self.assertEqual(activation['mods'][0]['entryType'], 'Starframe.FixtureMods.' + ('Failing' if fail else 'First'))
                inventory = json.loads((output / 'runtime-package.json').read_text())
                self.assertEqual(len(inventory), 9)
                self.assertFalse(any('disabled' in item['path'] for item in inventory))
            (runtime / 'System.Memory.dll').unlink()
            with self.assertRaises(ValueError):
                prepare(root / 'missing-dependency', runtime, fixture, False)
            self.assertFalse((root / 'missing-dependency').exists())
            (runtime / 'System.Memory.dll').write_bytes(b'fixture')
            (runtime / 'UnityEngine.dll').write_bytes(b'never publish')
            with self.assertRaises(ValueError):
                prepare(root / 'invalid', runtime, fixture, False)
