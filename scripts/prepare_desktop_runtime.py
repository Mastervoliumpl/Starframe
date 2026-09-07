"""Stage a locally built runtime and pinned bootstrap for an internal desktop build."""
import argparse
import hashlib
import json
from pathlib import Path

from prepare_bootstrap import prepare as prepare_bootstrap

RUNTIME_FILES = (
    'Starframe.Bootstrap.dll', 'Starframe.Runtime.dll',
    'Microsoft.Bcl.AsyncInterfaces.dll', 'System.Buffers.dll',
    'System.IO.Pipelines.dll', 'System.Memory.dll', 'System.Numerics.Vectors.dll',
    'System.Runtime.CompilerServices.Unsafe.dll', 'System.Text.Encodings.Web.dll',
    'System.Text.Json.dll', 'System.Threading.Tasks.Extensions.dll',
)


def prepare(destination: Path, runtime: Path, archive: Path) -> None:
    if destination.exists():
        raise ValueError('Use a new staging directory; existing files were retained')
    payload = [(name, (runtime / name).read_bytes()) for name in RUNTIME_FILES]
    if any(not value or len(value) > 8_388_608 for _, value in payload):
        raise ValueError('Runtime output is missing or exceeds the deployment limit')
    manifest = json.loads((Path(__file__).resolve().parents[1] / 'runtime/bootstrap.json').read_text(encoding='utf-8'))
    prepare_bootstrap(destination / 'bootstrap', archive.read_bytes(), manifest)
    package = destination / 'runtime'
    plugin = package / 'BepInEx/plugins/Starframe'
    plugin.mkdir(parents=True)
    for name, value in payload:
        (plugin / name).write_bytes(value)
    activation = package / 'Starframe/activation.json'
    activation.parent.mkdir()
    activation.write_text(json.dumps(dict(schemaVersion=2, runtimeContractVersion=1,
        integrationId='starframe.bepinex', deploymentRevision='0', installedMods=[], mods=[])) + '\n', encoding='utf-8')
    inventory = [dict(path=p.relative_to(package).as_posix(), sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                 for p in sorted(package.rglob('*')) if p.is_file()]
    (package / 'runtime-package.json').write_text(json.dumps(inventory, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--runtime', required=True, type=Path)
    parser.add_argument('--archive', required=True, type=Path)
    args = parser.parse_args()
    prepare(args.destination, args.runtime, args.archive)
    print('Staged runtime at', args.destination)
