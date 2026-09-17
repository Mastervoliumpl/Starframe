"""Copy current sources into an isolated, separately identified updater test build."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

from versions import version_updates

root = Path(__file__).resolve().parents[1]
destination = root / 'test-results/0.6.0-updates/source'
version = sys.argv[1]
if version not in ('0.6.0-dev.0', '0.6.0-dev.1'):
    raise SystemExit('Use one of the two disposable fixture versions.')
if not destination.exists():
    names = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=root).decode().split('\0')
    for name in names:
        if not name or name == 'AGENTS.md':
            continue
        source = root / name
        if source.is_file():
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
    shutil.copytree(root / 'src-tauri/target/installer/integration', destination / 'src-tauri/target/installer/integration')

(destination / 'VERSION').write_text(version + '\n', encoding='utf-8')
for name, content in version_updates(destination, version).items():
    (destination / name).write_text(content, encoding='utf-8')
config_path = destination / 'src-tauri/tauri.conf.json'
config = json.loads(config_path.read_text('utf-8'))
config.update(productName='Starframe Updater Test', identifier='io.github.mastervoliumpl.starframe.updater-test')
config_path.write_text(json.dumps(config, indent=2) + '\n', encoding='utf-8')
# Only root-version fields changed; retain the reviewed dependency inventory and refresh its input hashes.
inventory_path = destination / 'docs/notices/desktop-dependencies.json'
inventory = json.loads(inventory_path.read_text('utf-8'))
for name in inventory['inputs']:
    inventory['inputs'][name] = hashlib.sha256((destination / name).read_text('utf-8').encode()).hexdigest()
inventory_path.write_text(json.dumps(inventory, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
print(f'Prepared isolated Starframe Updater Test sources at {version}.')
