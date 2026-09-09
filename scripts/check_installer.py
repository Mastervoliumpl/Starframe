"""Reject incomplete or unexpected runtime resources before NSIS bundling."""

import hashlib
import json
from pathlib import Path
import stat

from prepare_desktop_runtime import RUNTIME_FILES
from versions import read_version
from check_dependency_notices import check as check_notices


def check(root: Path, integration: Path) -> None:
    read_version(root)
    lock = json.loads((root / 'package-lock.json').read_text(encoding='utf-8'))
    if lock['packages']['node_modules/@tauri-apps/cli']['version'] != '2.11.4':
        raise ValueError('Review the pinned NSIS template and hooks before changing the Tauri CLI')
    actual = set()
    pending = [integration]
    while pending:
        path = pending.pop()
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode) or getattr(info, 'st_file_attributes', 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT:
            raise ValueError('Installer resources must not contain links or junctions')
        if path.is_dir():
            pending.extend(path.iterdir())
        elif path.is_file():
            actual.add(path.relative_to(integration).as_posix())
        else:
            raise ValueError('Installer resources must be ordinary files')
    bootstrap = json.loads((root / 'runtime/bootstrap.json').read_text(encoding='utf-8'))
    expected = {f'bootstrap/{item["path"]}': item['sha256'] for item in bootstrap['files']}
    runtime_names = {f'BepInEx/plugins/Starframe/{name}' for name in RUNTIME_FILES}
    runtime_names.add('Starframe/activation.json')
    inventory = integration / 'runtime/runtime-package.json'
    entries = json.loads(inventory.read_text(encoding='utf-8'))
    if not isinstance(entries, list) or len(entries) != len(runtime_names):
        raise ValueError('Runtime inventory is incomplete or has extra entries')
    if {item['path'] for item in entries} != runtime_names:
        raise ValueError('Runtime inventory must contain only the supported runtime files')
    expected.update({f'runtime/{item["path"]}': item['sha256'] for item in entries})
    if actual != set(expected) | {'runtime/runtime-package.json'}:
        raise ValueError('Installer resources contain missing or unexpected files')
    for name, digest in expected.items():
        path = integration / name
        if not 0 < path.stat().st_size <= 8_388_608:
            raise ValueError(f'Installer resource exceeds supported size: {name}')
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError(f'Installer resource hash mismatch: {name}')
    activation = json.loads((integration / 'runtime/Starframe/activation.json').read_text(encoding='utf-8'))
    if activation != dict(schemaVersion=2, runtimeContractVersion=1,
                          integrationId='starframe.bepinex', deploymentRevision='0',
                          installedMods=[], mods=[]):
        raise ValueError('Installer runtime must have an empty initial activation')


if __name__ == '__main__':
    repository = Path(__file__).resolve().parents[1]
    check(repository, repository / 'src-tauri/target/installer/integration')
    check_notices(repository, repository / 'src-tauri/target/installer/integration')
    print('Installer runtime inventory and hashes verified. Signing and redistribution review remain separate checks.')
