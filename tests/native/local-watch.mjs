import { expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { withDesktop } from './session.mjs';
import { fixtureGame } from './fixture-game.mjs';

const { root, data, engine, env } = await fixtureGame();
const source = join(root, 'source');
const content = join(source, 'LJ/lua/watched.lua');
const manifest = join(source, 'starframe.local.json');
const metadata = JSON.stringify({
  schemaVersion: 1,
  modId: 'fixture.local.watched',
  name: 'Watched local fixture',
  author: 'Test fixture',
  version: 'dev.1',
  layout: { kind: 'starframe_lua_zip' },
});
await mkdir(join(source, 'LJ/lua'), { recursive: true });
await writeFile(content, "return 'original'\n");
await writeFile(manifest, metadata);
const list = (page) =>
  page.evaluate(() =>
    window.__TAURI_INTERNALS__.invoke('mod_action', {
      action: { kind: 'list' },
    }),
  );
const activationPath = join(engine, 'Starframe/activation.json');
const activation = async () =>
  JSON.parse(await readFile(activationPath, 'utf8'));
const head = async (page) =>
  (await list(page)).localWatches[0]?.source.reference;
const rebuilt = async (page, previous) => {
  await expect
    .poll(async () => (await head(page))?.hash, { timeout: 40000 })
    .not.toBe(previous.hash);
  const current = await head(page);
  expect(current).toBeDefined();
  expect((await list(page)).enabled).toEqual([current]);
  return current;
};
const deployed = async (text) => {
  await expect
    .poll(
      async () => {
        try {
          const current = await activation();
          if (current.mods.length !== 1) return '';
          return await readFile(
            join(
              engine,
              'Starframe',
              current.mods[0].root,
              'LJ/lua/watched.lua',
            ),
            'utf8',
          );
        } catch {
          return '';
        }
      },
      { timeout: 40000 },
    )
    .toBe(text);
};
let running;
let originalActivation;
let latest;
try {
  await withDesktop(
    data,
    async (page) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await page.getByRole('button', { name: /^Use installation:/ }).click();
      await page
        .getByRole('button', { name: 'Repair or reinstall runtime' })
        .click();
      await expect
        .poll(
          async () =>
            activation().then(
              (a) => a.mods.length,
              () => -1,
            ),
          { timeout: 40000 },
        )
        .toBe(0);
      await page.getByRole('button', { name: 'My mods', exact: true }).click();
      await page
        .getByRole('button', { name: 'Import local mod', exact: true })
        .click();
      const dialog = page.getByRole('dialog', {
        name: 'Import local mod',
        exact: true,
      });
      await dialog.getByRole('textbox', { name: 'Source path' }).fill(source);
      await dialog
        .getByRole('button', { name: 'Import copy', exact: true })
        .click();
      const enabled = page.getByRole('switch', {
        name: 'Enable Watched local fixture dev.1',
      });
      await expect(enabled).toBeVisible();
      await enabled.click();
      await expect(enabled).toBeChecked();
      await deployed("return 'original'\n");
      latest = await head(page);
      originalActivation = await readFile(activationPath, 'utf8');
      running = spawn(join(engine, 'Sanctuary.exe'), ['-t', '127.0.0.1'], {
        windowsHide: true,
        stdio: 'ignore',
      });
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(page.getByText('Game running', { exact: true })).toBeVisible(
        { timeout: 15000 },
      );
      await page.getByRole('button', { name: 'My mods', exact: true }).click();
      const search = page.getByRole('searchbox', {
        name: 'Search installed mods',
      });
      await search.fill('Watched local');
      await writeFile(content, "return 'build two'\n");
      latest = await rebuilt(page, latest);
      await expect(search).toBeFocused();
      await expect(search).toHaveValue('Watched local');
      await writeFile(content, "return 'build three'\n");
      latest = await rebuilt(page, latest);
      expect(await readFile(activationPath, 'utf8')).toBe(originalActivation);
      await writeFile(manifest, '{');
      await expect
        .poll(async () => (await list(page)).localWatches[0].state, {
          timeout: 40000,
        })
        .toBe('error');
      expect(await head(page)).toEqual(latest);
      await expect(
        page.getByText(/The previous copy is kept. Watching will retry./),
      ).toBeVisible();
      await page
        .getByText(/The previous copy is kept. Watching will retry./)
        .scrollIntoViewIfNeeded();
      await mkdir('test-results/native', { recursive: true });
      await page.screenshot({
        path: 'test-results/native/local-watch-error.png',
      });
      await writeFile(manifest, metadata);
    },
    env,
  );
  expect(running.exitCode).toBeNull();
  await writeFile(content, "return 'built with desktop closed'\n");
  expect(await readFile(activationPath, 'utf8')).toBe(originalActivation);
  await withDesktop(
    data,
    async (page) => {
      await expect(
        page.getByRole('button', { name: 'Import local mod', exact: true }),
      ).toBeEnabled();
      latest = await rebuilt(page, latest);
      expect(await readFile(activationPath, 'utf8')).toBe(originalActivation);
      running.kill();
      await expect
        .poll(() => running.exitCode !== null || running.signalCode !== null)
        .toBe(true);
      await deployed("return 'built with desktop closed'\n");
      const settings = join(engine, 'BepInEx/config/watched-fixture.cfg');
      await mkdir(join(engine, 'BepInEx/config'), { recursive: true });
      await writeFile(settings, 'retained=true\n');
      const row = page
        .locator('.mod-list > li')
        .filter({ hasText: `Build ${latest.hash.slice(0, 8)}` });
      await expect(row).toContainText('Following source');
      await row.scrollIntoViewIfNeeded();
      await page.screenshot({
        path: 'test-results/native/local-watch-latest.png',
      });
      await row
        .getByRole('button', { name: 'Uninstall Watched local fixture dev.1' })
        .click();
      await page
        .getByRole('button', { name: 'Confirm uninstall', exact: true })
        .click();
      await expect
        .poll(async () => (await list(page)).localWatches.length)
        .toBe(0);
      await expect
        .poll(async () => (await activation()).mods.length, { timeout: 40000 })
        .toBe(0);
      expect(await readFile(content, 'utf8')).toBe(
        "return 'built with desktop closed'\n",
      );
      expect(await readFile(settings, 'utf8')).toBe('retained=true\n');
      expect((await list(page)).library).toHaveLength(3);
    },
    env,
  );
} finally {
  if (running?.exitCode === null) running.kill();
}
console.log(
  'Native local rebuilds, game-running deferral, desktop restart, latest deployment, focus and source/settings retention passed.',
);
