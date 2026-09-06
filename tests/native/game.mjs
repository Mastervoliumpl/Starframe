import { expect } from '@playwright/test';
import {
  mkdtemp,
  mkdir,
  copyFile,
  writeFile,
  readFile,
} from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { withDesktop } from './session.mjs';

const root = await mkdtemp(join(tmpdir(), 'starframe-game-'));
const steam = join(root, 'Steam');
const game = join(steam, 'steamapps/common/Fixture Sanctuary');
const engine = join(game, 'engine');
await mkdir(join(engine, 'Sanctuary_Data/Managed'), { recursive: true });
await mkdir(join(engine, 'MonoBleedingEdge'), { recursive: true });
// A Windows ping process exercises path observation without starting a game.
const executable = join(engine, 'Sanctuary.exe');
await copyFile(join(process.env.SystemRoot, 'System32/ping.exe'), executable);
await copyFile(executable, join(engine, 'UnityPlayer.dll'));
await writeFile(
  join(engine, 'Sanctuary_Data/app.info'),
  'Enhearten Media PTY\nSanctuary\n',
);
await writeFile(
  join(engine, 'Sanctuary_Data/boot.config'),
  'build-guid=0123456789abcdef0123456789abcdef\n',
);
const manifest = join(steam, 'steamapps/appmanifest_4511930.acf');
const acf = (build) =>
  `"AppState" { "appid" "4511930" "installdir" "Fixture Sanctuary" "buildid" "${build}" "StateFlags" "4" }`;
await writeFile(manifest, acf('111'));
await writeFile(
  join(steam, 'steamapps/libraryfolders.vdf'),
  '"libraryfolders" {}',
);
const env = { STARFRAME_TEST_STEAM_ROOT: steam };
const data = join(root, 'data');
const original = await readFile(executable);
await withDesktop(
  data,
  async (page, pid) => {
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(
      page.getByRole('button', { name: /^Use installation:/ }),
    ).toBeEnabled();
    await page.getByRole('button', { name: /^Use installation:/ }).click();
    await expect(
      page.getByText('Game location saved. Game files were not changed.'),
    ).toBeVisible();
    await expect(
      page.getByText('Game not running', { exact: true }),
    ).toBeVisible();
    const choose = page.getByRole('button', { name: 'Choose game folder' });
    const picker = (folder) =>
      execFileSync(
        'powershell.exe',
        [
          '-NoProfile',
          '-ExecutionPolicy',
          'Bypass',
          '-File',
          'tests/native/choose-folder.ps1',
          '-AppProcessId',
          String(pid),
          ...(folder ? ['-Folder', folder] : []),
        ],
        { windowsHide: true, timeout: 15000 },
      );
    await choose.click();
    picker();
    await expect(choose).toBeEnabled();
    await expect(
      page.getByRole('heading', { name: 'Selected installation' }),
    ).toBeVisible();
    await choose.click();
    picker(root);
    await expect(page.getByRole('alert')).toContainText(
      'Required game file is missing',
    );
    await expect(
      page.getByRole('heading', { name: 'Selected installation' }),
    ).toBeVisible();
    await choose.click();
    picker(game);
    await expect(choose).toBeEnabled();
    await expect(page.getByRole('alert')).toHaveCount(0);
    const helper = spawn(executable, ['-t', '127.0.0.1'], {
      windowsHide: true,
      stdio: 'ignore',
    });
    try {
      await expect(page.getByText('Game running', { exact: true })).toBeVisible(
        { timeout: 10000 },
      );
      await page
        .getByRole('button', { name: 'Help & logs', exact: true })
        .click();
      await page
        .getByRole('button', { name: 'Run responsiveness check' })
        .click();
      await page
        .getByRole('button', { name: 'Downloads', exact: true })
        .click();
      await expect(page.getByRole('progressbar')).toHaveCount(3);
    } finally {
      helper.kill();
    }
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(
      page.getByText('Game not running', { exact: true }),
    ).toBeVisible({ timeout: 10000 });
    await writeFile(manifest, acf('222'));
    await expect(
      page.locator('dd').filter({ hasText: 'Steam 222' }),
    ).toBeVisible({ timeout: 40000 });
    await mkdir(resolve('test-results/native'), { recursive: true });
    await page.screenshot({ path: 'test-results/native/game-settings.png' });
  },
  env,
);
await withDesktop(
  data,
  async (page) => {
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(
      page.getByRole('heading', { name: 'Selected installation' }),
    ).toBeVisible();
    await expect(
      page.locator('dd').filter({ hasText: 'Steam 222' }),
    ).toBeVisible();
    await writeFile(
      join(engine, 'Sanctuary_Data/app.info'),
      'Different product\n',
    );
    await expect(
      page.getByRole('heading', { name: 'Saved location unavailable' }),
    ).toBeVisible({ timeout: 40000 });
    await expect(page.getByRole('alert')).toContainText('product identity');
    await page.getByRole('button', { name: 'Find in Steam' }).click();
    await expect(
      page.getByRole('button', { name: 'Find in Steam' }),
    ).toBeEnabled();
    await expect(
      page.getByRole('heading', { name: 'Saved location unavailable' }),
    ).toBeVisible();
  },
  env,
);
expect(await readFile(executable)).toEqual(original);
console.log(
  'Native game checks passed: discovery, saved selection, external process start/exit, build refresh, invalid layout retention and usable navigation. Only temporary fixtures were used.',
);
