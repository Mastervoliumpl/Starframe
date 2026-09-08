"""Prepare managed-order and Lua-collision fixtures outside the game directory."""
import argparse
import hashlib
import json
from pathlib import Path

from prepare_runtime_fixture import prepare


def prepare_ordering(destination: Path, runtime: Path, fixture: Path, reverse: bool) -> None:
    prepare(destination, runtime, fixture, False)
    manifest_path = destination / 'Starframe/activation.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    ids = ['fixture.lua.a', 'fixture.lua.b']
    if reverse:
        ids.reverse()
    for mod_id in ids:
        relative = 'LJ/lua/starframe_order_fixture.lua'
        root = 'mods/' + mod_id
        path = destination / 'Starframe' / root / relative
        path.parent.mkdir(parents=True)
        content = f'return "{mod_id}"\n'.encode()
        path.write_bytes(content)
        manifest['installedMods'].append(dict(modId=mod_id, name=mod_id, version='fixture'))
        manifest['mods'].append(dict(modId=mod_id, source=dict(kind='catalog', releaseId=mod_id), root=root,
                                     entryAssembly=None, entryType=None, requires=[],
                                     files=[dict(path=relative, sha256=hashlib.sha256(content).hexdigest())]))
    manifest['deploymentRevision'] = '401' if reverse else '400'
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    inventory = [dict(path=p.relative_to(destination).as_posix(), sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                 for p in sorted(destination.rglob('*')) if p.is_file() and p.name != 'runtime-package.json']
    (destination / 'runtime-package.json').write_text(json.dumps(inventory, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--reverse', action='store_true')
    args = parser.parse_args()
    prepare_ordering(args.destination, args.runtime, args.fixture, args.reverse)
