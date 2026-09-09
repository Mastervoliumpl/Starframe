import { expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { access, mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fixtureGame } from './fixture-game.mjs';
import { withDesktop } from './session.mjs';

const { root, data, engine, env } = await fixtureGame();
const exists = (path) =>
  access(path).then(
    () => true,
    () => false,
  );
const loader = join(engine, 'winhttp.dll');
const settings = join(engine, 'BepInEx/config/fixture.cfg');
const external = join(engine, 'BepInEx/plugins/External.dll');
const source = join(root, 'developer-source.dll');
let game;

try {
  await withDesktop(
    data,
    async (page) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await page.getByRole('button', { name: /^Use installation:/ }).click();
      await page
        .getByRole('button', { name: 'Install or retry setup' })
        .click();
      await expect.poll(() => exists(loader)).toBe(true);
      const remove = page.getByRole('button', {
        name: 'Remove Starframe from game',
        exact: true,
      });
      await expect(remove).toBeEnabled();
      await mkdir(join(engine, 'BepInEx/config'), { recursive: true });
      await writeFile(settings, 'retained game settings');
      await writeFile(source, 'retained developer source');

      game = spawn(join(engine, 'Sanctuary.exe'), ['-t', '127.0.0.1'], {
        windowsHide: true,
        stdio: 'ignore',
      });
      await expect(
        page.getByText('Game running', { exact: true }),
      ).toBeVisible();
      await expect(remove).toBeDisabled({ timeout: 15000 });
      await expect(
        page.getByText(/Close the game first; removal errors stay here/),
      ).toBeVisible();
      expect(await exists(loader)).toBe(true);
      game.kill();
      await expect(remove).toBeEnabled({ timeout: 15000 });

      await writeFile(external, 'unowned plugin fixture');
      await remove.click();
      await expect(
        page
          .getByRole('region', { name: 'Settings', exact: true })
          .getByText(
            'External plugins still use BepInEx. Loader files were retained.',
            { exact: true },
          ),
      ).toBeVisible();
      expect(await exists(loader)).toBe(true);
      expect(await readFile(external, 'utf8')).toBe('unowned plugin fixture');
      await rename(external, join(root, 'retained-external.dll'));
      await expect(remove).toBeEnabled();
      await remove.click();
      await expect.poll(() => exists(loader)).toBe(false);
      await expect(
        page.getByRole('button', { name: 'Install or retry setup' }),
      ).toBeEnabled();
      expect(await readFile(settings, 'utf8')).toBe('retained game settings');
      expect(await readFile(source, 'utf8')).toBe('retained developer source');
      await page.screenshot({
        path: 'test-results/native/game-cleanup.png',
        fullPage: true,
      });
    },
    env,
  );

  await withDesktop(
    data,
    async (page) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(
        page.getByRole('button', { name: 'Install or retry setup' }),
      ).toBeEnabled();
      expect(await exists(loader)).toBe(false);
      expect(await readFile(settings, 'utf8')).toBe('retained game settings');
    },
    env,
  );
  console.log(
    'Native cleanup passed: running-game guard, external-plugin refusal, retry, retained settings/source and restart.',
  );
} finally {
  if (game && game.exitCode === null) game.kill();
}
