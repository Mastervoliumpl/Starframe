import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  mkdtemp,
  mkdir,
  readFile,
  writeFile,
  copyFile,
  realpath,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export async function fixtureGame() {
  const root = await realpath(await mkdtemp(join(tmpdir(), 'starframe-mods-')));
  const data = join(root, 'data');
  const steam = join(root, 'Steam');
  const game = join(steam, 'steamapps/common/Fixture Sanctuary');
  const engine = join(game, 'engine');
  const resources = join(root, 'integration');
  await mkdir(join(engine, 'Sanctuary_Data/Managed'), { recursive: true });
  await mkdir(join(engine, 'MonoBleedingEdge'), { recursive: true });
  await copyFile(
    join(process.env.SystemRoot, 'System32/ping.exe'),
    join(engine, 'Sanctuary.exe'),
  );
  await copyFile(
    join(engine, 'Sanctuary.exe'),
    join(engine, 'UnityPlayer.dll'),
  );
  await writeFile(
    join(engine, 'Sanctuary_Data/app.info'),
    'Enhearten Media PTY\nSanctuary\n',
  );
  await writeFile(
    join(engine, 'Sanctuary_Data/boot.config'),
    'build-guid=0123456789abcdef0123456789abcdef\n',
  );
  await writeFile(
    join(steam, 'steamapps/appmanifest_4511930.acf'),
    '"AppState" { "appid" "4511930" "installdir" "Fixture Sanctuary" "buildid" "111" "StateFlags" "4" }',
  );
  await writeFile(
    join(steam, 'steamapps/libraryfolders.vdf'),
    '"libraryfolders" {}',
  );
  execFileSync(
    process.env.STARFRAME_TEST_PYTHON || 'python',
    [
      'scripts/prepare_bootstrap.py',
      join(resources, 'bootstrap'),
      ...(process.env.STARFRAME_TEST_BOOTSTRAP_ARCHIVE
        ? ['--archive', process.env.STARFRAME_TEST_BOOTSTRAP_ARCHIVE]
        : []),
    ],
    { windowsHide: true, timeout: 60000, stdio: 'inherit' },
  );
  const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
  const runtime = join(resources, 'runtime');
  const runtimeFiles = {
    'BepInEx/plugins/Starframe/Starframe.Bootstrap.dll': Buffer.from(
      'inert bootstrap fixture',
    ),
    'BepInEx/plugins/Starframe/Starframe.Runtime.dll': Buffer.from(
      'inert runtime fixture',
    ),
    'Starframe/activation.json': await readFile(
      'contracts/fixtures/activation-empty.json',
    ),
  };
  for (const [path, bytes] of Object.entries(runtimeFiles)) {
    await mkdir(join(runtime, path, '..'), { recursive: true });
    await writeFile(join(runtime, path), bytes);
  }
  await writeFile(
    join(runtime, 'runtime-package.json'),
    JSON.stringify(
      Object.entries(runtimeFiles).map(([path, bytes]) => ({
        path,
        sha256: hash(bytes),
      })),
    ),
  );
  const env = {
    STARFRAME_TEST_STEAM_ROOT: steam,
    STARFRAME_INTEGRATION_DIR: resources,
    HTTPS_PROXY: 'http://127.0.0.1:1',
    HTTP_PROXY: 'http://127.0.0.1:1',
    NO_PROXY: '',
  };
  return { root, data, steam, engine, env };
}
