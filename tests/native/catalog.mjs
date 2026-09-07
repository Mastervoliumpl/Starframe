import { expect } from '@playwright/test';
import { DatabaseSync } from 'node:sqlite';
import { mkdtemp, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { withDesktop } from './session.mjs';

const root = await mkdtemp(join(tmpdir(), 'starframe-catalog-'));
const offline = {
  HTTPS_PROXY: 'http://127.0.0.1:1',
  HTTP_PROXY: 'http://127.0.0.1:1',
  NO_PROXY: '',
};
await withDesktop(
  root,
  async (page) => {
    await page.getByRole('button', { name: 'Catalog', exact: true }).click();
    await expect(page.getByText('No catalog is cached yet.')).toBeVisible();
  },
  offline,
);

const catalog = { schemaVersion: 1, catalogRevision: '41', mods: [] };
const cache = {
  catalog,
  etag: '"native-fixture"',
  lastModified: null,
  lastChecked: 1788819700,
  lastSuccess: 1788819700,
  error: null,
};
const db = new DatabaseSync(join(root, 'sqlite', 'state.db'));
db.prepare(
  'INSERT INTO catalog_cache (id, record) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET record=excluded.record',
).run(JSON.stringify(cache));
db.close();
await withDesktop(
  root,
  async (page) => {
    await page.getByRole('button', { name: 'Catalog', exact: true }).click();
    const region = page.getByRole('region', { name: 'Catalog', exact: true });
    await expect(region.getByRole('status')).toContainText(
      'Catalog revision 41',
      { timeout: 30000 },
    );
    await expect(region.getByRole('alert')).toContainText(
      'Catalog request failed',
      { timeout: 30000 },
    );
    await expect(
      region.getByText('Cached revision 41 remains available.'),
    ).toBeVisible();
    await expect(region.getByText(/Last successful check:/)).toBeVisible();
    await mkdir(resolve('test-results/native'), { recursive: true });
    await page.screenshot({
      path: resolve('test-results/native/catalog-offline.png'),
    });
  },
  offline,
);
const reopened = new DatabaseSync(join(root, 'sqlite', 'state.db'));
const retained = JSON.parse(
  reopened.prepare('SELECT record FROM catalog_cache WHERE id=1').get().record,
);
expect(retained.catalog).toEqual(catalog);
expect(retained.etag).toBe(cache.etag);
expect(retained.lastSuccess).toBe(cache.lastSuccess);
expect(retained.error).toContain('Catalog request failed');
reopened.close();
console.log(
  'Native catalog check passed: offline startup, cached restart, persistent error and clean exit.',
);
