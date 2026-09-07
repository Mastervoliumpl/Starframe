"""Prepare a development fixture runtime package; never write into the game."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def prepare(destination: Path, runtime: Path, fixture: Path, fail: bool) -> None:
    required = ['Starframe.Bootstrap.dll', 'Starframe.Runtime.dll', 'System.Text.Json.dll', 'System.Memory.dll', 'System.Buffers.dll', 'System.Threading.Tasks.Extensions.dll']
    if any(not (runtime / name).is_file() for name in required):
        raise ValueError('Build the bootstrap with its complete Mono dependency output')
    destination.mkdir(parents=True, exist_ok=False)
    plugin = destination / 'BepInEx/plugins/Starframe'
    plugin.mkdir(parents=True)
    for source in runtime.glob('*.dll'):
        if source.name.startswith(('UnityEngine', 'BepInEx')):
            raise ValueError('Game/bootstrap references must not be copied into the runtime package')
        shutil.copyfile(source, plugin / source.name)
    active = []
    for mod_id, entry, requires in [('fixture.first', 'Failing' if fail else 'First', []), ('fixture.second', 'Second', ['fixture.first'])]:
        root = 'mods/' + mod_id
        target = destination / 'Starframe' / root / fixture.name
        target.parent.mkdir(parents=True)
        shutil.copyfile(fixture, target)
        active.append(dict(modId=mod_id, source=dict(kind='catalog', releaseId=mod_id), root=root, entryAssembly=fixture.name,
                           entryType='Starframe.FixtureMods.' + entry, requires=requires,
                           files=[dict(path=fixture.name, sha256=hashlib.sha256(target.read_bytes()).hexdigest())]))
    manifest = dict(schemaVersion=2, runtimeContractVersion=1, integrationId='starframe.bepinex', deploymentRevision='101' if fail else '100',
                    installedMods=[dict(modId=key, name=key, version='fixture') for key in ['fixture.first', 'fixture.second', 'fixture.disabled']], mods=active)
    (destination / 'Starframe/activation.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    inventory = [dict(path=p.relative_to(destination).as_posix(), sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                 for p in sorted(destination.rglob('*')) if p.is_file()]
    (destination / 'runtime-package.json').write_text(json.dumps(inventory, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--fail', action='store_true')
    args = parser.parse_args()
    prepare(args.destination, args.runtime, args.fixture, args.fail)
