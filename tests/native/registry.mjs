import { expect } from '@playwright/test';
import { mkdtemp, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { withDesktop } from './session.mjs';
import { installedRegistry } from './fixture-registry.mjs';

const root = await mkdtemp(join(tmpdir(), 'starframe-registry-'));
const offline = {
  HTTPS_PROXY: 'http://127.0.0.1:1',
  HTTP_PROXY: 'http://127.0.0.1:1',
  NO_PROXY: '',
};
const invoke = (page, command, args = {}) =>
  page.evaluate(
    ({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args),
    { command, args },
  );
await withDesktop(
  root,
  async (page) => {
    await expect
      .poll(
        async () =>
          (await invoke(page, 'mod_action', { action: { kind: 'list' } }))
            .library.length,
      )
      .toBe(0);
  },
  offline,
);
const reference = await installedRegistry(
  root,
  1,
  'Offline registry fixture',
  { 'LJ/lua/mod.lua': Buffer.from('return "inert fixture"') },
  {
    schemaVersion: 1,
    kind: 'code',
    loader: 'lua',
    entryPath: 'LJ/lua/mod.lua',
  },
);
for (let reopening = 0; reopening < 2; reopening++) {
  await withDesktop(
    root,
    async (page) => {
      await expect(
        page.getByRole('button', {
          name: 'Offline registry fixture',
          exact: true,
        }),
      ).toBeVisible({ timeout: 30000 });
      await page.getByRole('button', { name: 'Mods', exact: true }).click();
      await expect(
        page.getByRole('heading', {
          name: 'Sign in to browse Mods',
          exact: true,
        }),
      ).toBeVisible();
      await page
        .getByRole('button', { name: 'Open sign-in settings', exact: true })
        .click();
      await expect(
        page.getByRole('button', { name: 'Sign in with Steam', exact: true }),
      ).toBeEnabled();
      const query = {
        page: 1,
        pageSize: 10,
        query: '',
        includeTags: [],
        excludeTags: [],
        sort: 'updated',
        period: 'all',
        maintenance: 'all',
        modType: 'all',
        gameBuilds: [],
      };
      for (const [command, args] of [
        ['registry_list', { query }],
        ['registry_options', {}],
        ['registry_detail', { modId: 1 }],
        ['registry_release', { releaseId: reference.reference.releaseId }],
        ['registry_history', { modId: 1, page: 1, pageSize: 12 }],
        ['open_external', { page: 'registry:1' }],
        [
          'registry_install',
          { requestId: crypto.randomUUID(), reference: reference.reference },
        ],
      ]) {
        const error = await page.evaluate(
          async ({ command, args }) => {
            try {
              await window.__TAURI_INTERNALS__.invoke(command, args);
              return null;
            } catch (error) {
              return error;
            }
          },
          { command, args },
        );
        expect(error).not.toBeNull();
        expect(['auth_required', 'registry_trust_unavailable']).toContain(
          error.code,
        );
        expect(error.message).not.toMatch(/Bearer|Starframe:fixture/);
      }
      await page.getByRole('button', { name: 'My mods', exact: true }).click();
      const enabled = page.getByRole('switch', {
        name: 'Enable Offline registry fixture 1',
      });
      await expect(enabled).toBeEnabled();
      await enabled.click();
      await expect(enabled).toBeChecked();
      await enabled.click();
      await expect(enabled).not.toBeChecked();
      await expect(
        page.getByRole('button', { name: 'Import local mod', exact: true }),
      ).toBeEnabled();
      const view = await invoke(page, 'mod_action', {
        action: { kind: 'list' },
      });
      expect(view.library.map((entry) => entry.reference)).toEqual([reference]);
      expect(
        await invoke(page, 'package_action', { action: { kind: 'list' } }),
      ).toEqual([]);
      if (reopening === 1) {
        await mkdir(resolve('test-results/native'), { recursive: true });
        await page.screenshot({
          path: resolve('test-results/native/registry-offline.png'),
        });
      }
    },
    offline,
  );
}
console.log(
  'Native registry boundary passed: signed-out discovery/install errors, offline installed management, exact restart persistence and clean exit.',
);
