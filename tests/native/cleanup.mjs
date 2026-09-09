import { once } from 'node:events';
import { expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  access,
  mkdir,
  readFile,
  rename,
  unlink,
  writeFile,
} from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fixtureGame } from './fixture-game.mjs';
import { withDesktop } from './session.mjs';

const first = await fixtureGame();
const second = await fixtureGame();
const { root, data, engine, env } = first;
const exists = (path) =>
  access(path).then(
    () => true,
    () => false,
  );
const loader = join(engine, 'winhttp.dll');
const runtime = join(engine, 'BepInEx/plugins/Starframe/Starframe.Runtime.dll');
const settings = join(engine, 'BepInEx/config/fixture.cfg');
const external = join(engine, 'BepInEx/plugins/External.dll');
const source = join(root, 'developer-source.dll');
let game;
async function uninstall(keep) {
  const child = spawn(
    resolve('src-tauri/target/debug/starframe.exe'),
    [
      '--installer-uninstall',
      'io.github.mastervoliumpl.starframe',
      keep ? 'keep' : 'delete',
    ],
    {
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, ...env, STARFRAME_TEST_DATA_DIR: data },
    },
  );
  let output = '';
  child.stdout.on('data', (bytes) => {
    output += bytes.toString();
  });
  child.stderr.on('data', (bytes) => {
    output += bytes.toString();
  });
  const code = await new Promise((resolve, reject) => {
    child.on('exit', resolve);
    child.on('error', reject);
  });
  return { code, output };
}
try {
  await withDesktop(
    data,
    async (page) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await page.getByRole('button', { name: /^Use installation:/ }).click();
      await expect(page.locator('#launch-reason')).toContainText(
        /Runtime installed|ready to launch/i,
        { timeout: 20000 },
      );
      expect(await exists(loader)).toBe(true);
      const repair = page.getByRole('button', {
        name: 'Repair or reinstall runtime',
        exact: true,
      });
      await mkdir(join(engine, 'BepInEx/config'), { recursive: true });
      await writeFile(settings, 'retained game settings');
      await writeFile(source, 'retained developer source');
      await unlink(runtime);
      await repair.click();
      await expect.poll(() => exists(runtime)).toBe(true);
      await expect(page.locator('#launch-reason')).toContainText(
        /Runtime installed|ready to launch/i,
      );
      const original = await readFile(runtime);
      await writeFile(runtime, 'changed outside Starframe');
      await repair.click();
      await expect(page.locator('#launch-reason')).toContainText(/changed/);
      expect(await readFile(runtime, 'utf8')).toBe('changed outside Starframe');
      await writeFile(runtime, original);
      await repair.click();
      await expect(page.locator('#launch-reason')).toContainText(
        /Runtime installed|ready to launch/i,
      );
      game = spawn(join(engine, 'Sanctuary.exe'), ['-t', '127.0.0.1'], {
        windowsHide: true,
        stdio: 'ignore',
      });
      await expect(
        page.getByText('Game running', { exact: true }),
      ).toBeVisible();
      await expect(repair).toBeDisabled();
    },
    env,
  );
  const running = await uninstall(false);
  expect(running.code, running.output).not.toBe(0);
  expect(await exists(loader)).toBe(true);
  expect(await exists(join(data, 'sqlite/state.db'))).toBe(true);
  const gameExited = once(game, 'exit');
  game.kill();
  await gameExited;

  // Change the packaged runtime while the app is closed: startup must apply it even at the same saved collection revision.
  const resources = env.STARFRAME_INTEGRATION_DIR;
  const packageFile = join(
    resources,
    'runtime/BepInEx/plugins/Starframe/Starframe.Runtime.dll',
  );
  const updated = Buffer.from('updated inert runtime fixture');
  await writeFile(packageFile, updated);
  const inventoryPath = join(resources, 'runtime/runtime-package.json');
  const inventory = JSON.parse(await readFile(inventoryPath, 'utf8'));
  inventory.find((entry) =>
    entry.path.endsWith('/Starframe.Runtime.dll'),
  ).sha256 = createHash('sha256').update(updated).digest('hex');
  await writeFile(inventoryPath, JSON.stringify(inventory));
  await withDesktop(
    data,
    async (page) => {
      await expect
        .poll(() => readFile(runtime, 'utf8'))
        .toBe(updated.toString());
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(page.locator('#launch-reason')).toContainText(
        /Runtime installed|ready to launch/i,
      );
    },
    env,
  );
  await withDesktop(
    data,
    async (page) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await page.getByRole('button', { name: /^Use installation:/ }).click();
      await expect(page.locator('#launch-reason')).toContainText(
        /Runtime installed|ready to launch/i,
        { timeout: 20000 },
      );
      expect(await exists(join(second.engine, 'winhttp.dll'))).toBe(true);
    },
    second.env,
  );

  await writeFile(external, 'unowned plugin fixture');
  const conflict = await uninstall(false);
  expect(conflict.code, conflict.output).not.toBe(0);
  expect(await exists(loader)).toBe(true);
  expect(await readFile(external, 'utf8')).toBe('unowned plugin fixture');
  expect(await exists(join(data, 'sqlite/state.db'))).toBe(true);
  await rename(external, join(root, 'retained-external.dll'));
  const kept = await uninstall(true);
  expect(kept.code, kept.output).toBe(0);
  expect(await exists(loader)).toBe(false);
  expect(await exists(join(second.engine, 'winhttp.dll'))).toBe(false);
  expect(await exists(join(data, 'sqlite/state.db'))).toBe(true);
  expect(await readFile(settings, 'utf8')).toBe('retained game settings');
  expect(await readFile(source, 'utf8')).toBe('retained developer source');
  const removed = await uninstall(false);
  expect(removed.code, removed.output).toBe(0);
  expect(await exists(join(data, 'sqlite/state.db'))).toBe(false);
  expect(await readFile(settings, 'utf8')).toBe('retained game settings');
  expect(await readFile(source, 'utf8')).toBe('retained developer source');
  console.log(
    'Native lifecycle passed: automatic setup/update, missing-file repair, changed-file refusal, stopped-game guards, all recorded deployments, cleanup retry, explicit retention and default data deletion.',
  );
} finally {
  if (game && game.exitCode === null) game.kill();
}
