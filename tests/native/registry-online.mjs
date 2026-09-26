import { expect } from '@playwright/test';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { withDesktop } from './session.mjs';
import { registryApi } from './registry-api.mjs';

const root = await mkdtemp(join(tmpdir(), 'starframe-registry-online-'));
const api = await registryApi();
const offline = {
  HTTPS_PROXY: 'http://127.0.0.1:1',
  HTTP_PROXY: 'http://127.0.0.1:1',
  NO_PROXY: '127.0.0.1',
};
const invoke = (page, command, args = {}) =>
  page.evaluate(
    ({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args),
    { command, args },
  );
const library = (page) =>
  invoke(page, 'mod_action', { action: { kind: 'list' } });
const operations = (page) =>
  invoke(page, 'package_action', { action: { kind: 'list' } });
const reference = (release) => ({
  kind: 'registry',
  reference: {
    modId: release.modId,
    releaseId: release.releaseId,
    sha256: release.artifact.sha256,
  },
});
const references = api.releases.map(reference);
await writeFile(
  join(root, 'registry-fixture.json'),
  JSON.stringify(api.config),
);
await writeFile(
  join(root, 'registry-credential.fixture.json'),
  JSON.stringify(api.credential),
);
try {
  await withDesktop(
    root,
    async (page) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(page.getByText('Signed in as Fixture user.')).toBeVisible({
        timeout: 30000,
      });
      await page.getByRole('button', { name: 'Mods', exact: true }).click();
      const browser = page.getByRole('region', { name: 'Mods', exact: true });
      await expect(
        browser.getByRole('button', {
          name: 'Details for Native registry mod',
          exact: true,
        }),
      ).toBeVisible({ timeout: 30000 });
      await browser
        .getByRole('button', { name: 'Filters', exact: true })
        .click();
      await browser
        .getByRole('button', {
          name: 'Lua: Any. Click to include.',
          exact: true,
        })
        .click();
      await expect
        .poll(() =>
          api.requests.some((request) =>
            request.query.includes('includeTags=lua'),
          ),
        )
        .toBe(true);
      await browser
        .getByRole('button', {
          name: 'Details for Native registry mod',
          exact: true,
        })
        .click();
      const details = browser.getByRole('region', {
        name: 'Mod details',
        exact: true,
      });
      await expect(
        details.getByRole('heading', {
          name: 'Native registry mod',
          exact: true,
        }),
      ).toBeFocused();
      await expect(
        details.getByText(
          'Native fixture description. <script>Safe text.</script>',
          { exact: true },
        ),
      ).toBeVisible();
      await details.getByRole('button', { name: '1.0.0', exact: true }).click();
      await expect(
        details.getByRole('heading', { name: 'Release 1.0.0', exact: true }),
      ).toBeVisible();
      api.mode('tampered');
      await details
        .getByRole('button', { name: 'Install this release', exact: true })
        .click();
      await expect(details.getByRole('alert')).toContainText(
        /signature|trust|signed/i,
        { timeout: 30000 },
      );
      expect((await library(page)).library).toEqual([]);
      expect(await operations(page)).toEqual([]);
      expect(
        api.requests.some((request) => request.path.endsWith('/downloads')),
      ).toBe(false);
      api.mode('online');
      await details
        .getByRole('button', { name: 'Install this release', exact: true })
        .click();
      await expect
        .poll(
          async () =>
            (await operations(page)).filter(
              (operation) => operation.status === 'completed',
            ).length,
          { timeout: 30000 },
        )
        .toBe(1);
      expect(
        (await library(page)).library.map((entry) => entry.reference),
      ).toEqual([references[1]]);
      await expect(
        details.getByRole('button', { name: 'Installed', exact: true }),
      ).toBeDisabled();
      expect(api.receipts.has(api.releases[1].releaseId)).toBe(true);
      await details.getByRole('button', { name: '2.0.0', exact: true }).click();
      api.mode('stale');
      const granted = api.requests.filter((request) =>
        request.path.endsWith('/downloads'),
      ).length;
      await details
        .getByRole('button', { name: 'Install this release', exact: true })
        .click();
      await expect(details.getByRole('alert')).toContainText(
        /expired|fresh|trust|signed/i,
      );
      expect(
        api.requests.filter((request) => request.path.endsWith('/downloads'))
          .length,
      ).toBe(granted);
      expect((await library(page)).library.length).toBe(1);
      api.mode('online');
      await details
        .getByRole('button', { name: 'Install this release', exact: true })
        .click();
      await expect
        .poll(async () => (await library(page)).library.length, {
          timeout: 30000,
        })
        .toBe(2);
      expect((await library(page)).enabled).toEqual([]);
      expect(api.receipts.size).toBe(2);
      await expect(
        details.getByRole('button', { name: 'Installed', exact: true }),
      ).toBeDisabled();
      expect(await page.locator('body').innerText()).not.toContain(
        api.credential.token,
      );
      await browser.evaluate((element) => (element.scrollTop = 0));
      await mkdir(resolve('test-results/native'), { recursive: true });
      await page.screenshot({
        path: resolve('test-results/native/registry-online.png'),
      });
      api.mode('restricted');
      await browser
        .getByRole('button', { name: 'Refresh mods', exact: true })
        .click();
      await expect(browser.getByRole('alert')).toContainText(
        /permission|account/i,
      );
      await page
        .getByRole('button', { name: 'Collections', exact: true })
        .click();
      await expect(
        page.getByRole('button', { name: 'New collection', exact: true }),
      ).toBeEnabled();
      expect((await library(page)).library.length).toBe(2);
      api.mode('online');
    },
    offline,
  );
  expect(
    await readFile(
      join(root, 'registry-credential.fixture.json'),
      'utf8',
    ).catch(() => null),
  ).toBeNull();
  api.mode('offline');
  await withDesktop(
    root,
    async (page) => {
      await expect
        .poll(async () => (await library(page)).library.length, {
          timeout: 30000,
        })
        .toBe(2);
      expect(
        (await library(page)).library
          .map((entry) => JSON.stringify(entry.reference))
          .sort(),
      ).toEqual(references.map((entry) => JSON.stringify(entry)).sort());
      await page.getByRole('button', { name: 'Mods', exact: true }).click();
      await expect(
        page.getByRole('heading', {
          name: 'Sign in to browse Mods',
          exact: true,
        }),
      ).toBeVisible({ timeout: 30000 });
      await page.getByRole('button', { name: 'My mods', exact: true }).click();
      await expect(
        page.getByRole('button', { name: 'Import local mod', exact: true }),
      ).toBeEnabled();
      const enabled = page.getByRole('switch', {
        name: 'Enable Native registry mod 1.0.0',
        exact: true,
      });
      await enabled.click();
      await expect(enabled).toBeChecked();
      expect((await library(page)).enabled).toEqual([references[1]]);
      await invoke(page, 'auth_sign_out');
    },
    offline,
  );
  await writeFile(
    join(root, 'registry-credential.fixture.json'),
    JSON.stringify(api.credential),
  );
  api.mode('expired');
  await withDesktop(
    root,
    async (page) => {
      await page.getByRole('button', { name: 'Mods', exact: true }).click();
      await expect(
        page.getByRole('heading', {
          name: 'Sign in to browse Mods',
          exact: true,
        }),
      ).toBeVisible();
      expect((await library(page)).library.length).toBe(2);
      expect((await library(page)).enabled).toEqual([references[1]]);
      expect(await invoke(page, 'auth_restore')).toBeNull();
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(
        page.getByRole('button', { name: 'Choose game folder', exact: true }),
      ).toBeEnabled();
      await page.getByRole('button', { name: 'My mods', exact: true }).click();
      await expect(
        page.getByRole('button', { name: 'Import local mod', exact: true }),
      ).toBeEnabled();
    },
    offline,
  );
} finally {
  await api.close();
}
console.log(
  'Native signed registry acceptance passed: real discovery/detail/history RPCs, protected synthetic session, independently supplied fixture root, tamper refusal before grant, exact older/latest installs and receipts, separate collection selection and offline restart.',
);
