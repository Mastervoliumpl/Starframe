import { expect } from '@playwright/test';
import { createHash, randomUUID } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { withDesktop } from './session.mjs';

const root = await mkdtemp(join(tmpdir(), 'starframe-packages-'));
const offline = {
  HTTPS_PROXY: 'http://127.0.0.1:1',
  HTTP_PROXY: 'http://127.0.0.1:1',
  NO_PROXY: '',
};
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const bytes = Buffer.from('inert native fixture');
const archiveHash = hash('native archive identity fixture');
const request = randomUUID();
await withDesktop(
  root,
  async (page) => {
    await page.getByRole('button', { name: 'Catalog', exact: true }).waitFor();
    await expect
      .poll(() =>
        page.evaluate(() =>
          window.__TAURI_INTERNALS__.invoke('package_action', {
            action: { kind: 'list' },
          }),
        ),
      )
      .toEqual([]);
  },
  offline,
);
const directory = join(root, 'artifacts', archiveHash, 'package');
await mkdir(directory, { recursive: true });
await writeFile(join(directory, 'Core.dll'), bytes);
const catalog = {
  schemaVersion: 1,
  catalogRevision: '41',
  mods: [
    {
      id: 'fixture.core',
      name: 'Native fixture',
      author: 'Test fixture',
      sourceUrl: 'https://example.invalid/source',
      releases: [
        {
          id: 'fixture.core.1',
          version: '1',
          withdrawn: false,
          requires: [],
          testedGameBuilds: [],
          artifact: {
            url: 'https://example.invalid/core.zip',
            sha256: archiveHash,
            sizeBytes: 123,
            layout: {
              kind: 'starframe_managed_zip',
              root: 'package',
              entryAssembly: 'Core.dll',
              entryType: 'Fixture.Core',
            },
          },
        },
      ],
    },
  ],
};
const db = new DatabaseSync(join(root, 'sqlite', 'state.db'));
db.prepare(
  'INSERT INTO catalog_cache (id, record) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET record=excluded.record',
).run(
  JSON.stringify({
    catalog,
    etag: null,
    lastModified: null,
    lastChecked: null,
    lastSuccess: null,
    error: null,
  }),
);
db.prepare('INSERT INTO prepared_artifacts (hash, record) VALUES (?, ?)').run(
  archiveHash,
  JSON.stringify({
    hash: archiveHash,
    files: [
      {
        path: 'package/Core.dll',
        sha256: hash(bytes),
        sizeBytes: bytes.length,
      },
    ],
  }),
);
db.close();
const action = (page, action) =>
  page.evaluate(
    (action) => window.__TAURI_INTERNALS__.invoke('package_action', { action }),
    action,
  );
let failedRequest;
await withDesktop(
  root,
  async (page) => {
    await page.getByRole('button', { name: 'Catalog', exact: true }).waitFor();
    await expect.poll(() => action(page, { kind: 'list' })).toEqual([]);
    const invalid = await page.evaluate(async (requestId) => {
      return window.__TAURI_INTERNALS__
        .invoke('package_action', {
          action: { kind: 'prepare', requestId, releaseId: '../unapproved' },
        })
        .catch((error) => error);
    }, randomUUID());
    expect(invalid.code).toBe('package_failed');
    await action(page, {
      kind: 'prepare',
      requestId: request,
      releaseId: 'fixture.core.1',
    });
    await expect
      .poll(
        async () =>
          (await action(page, { kind: 'list' })).find(
            (op) => op.requestId === request,
          )?.status,
      )
      .toBe('completed');
    const duplicate = await action(page, {
      kind: 'prepare',
      requestId: request,
      releaseId: 'fixture.core.1',
    });
    expect(duplicate.filter((op) => op.requestId === request)).toHaveLength(1);
    await page.getByRole('button', { name: 'Catalog', exact: true }).click();
    await expect(page.getByText('Catalog revision 41')).toBeVisible();
    await writeFile(
      join(directory, 'Core.dll'),
      Buffer.from('changed native bytes'),
    );
    failedRequest = randomUUID();
    await action(page, {
      kind: 'prepare',
      requestId: failedRequest,
      releaseId: 'fixture.core.1',
    });
    await expect
      .poll(
        async () =>
          (await action(page, { kind: 'list' })).find(
            (op) => op.requestId === failedRequest,
          )?.status,
      )
      .toBe('failed');
  },
  offline,
);
await withDesktop(
  root,
  async (page) => {
    await page.getByRole('button', { name: 'Catalog', exact: true }).waitFor();
    await expect
      .poll(
        async () =>
          (await action(page, { kind: 'list' })).find(
            (op) => op.requestId === failedRequest,
          )?.status,
      )
      .toBe('failed');
  },
  offline,
);
expect(await readFile(join(directory, 'Core.dll'), 'utf8')).toBe(
  'changed native bytes',
);
const reopened = new DatabaseSync(join(root, 'sqlite', 'state.db'));
expect(
  reopened.prepare('SELECT count(*) AS count FROM library').get().count,
).toBe(1);
reopened.close();
console.log(
  'Native packages passed: command permissions, verified reuse, duplicate requests, retained failure and restart.',
);
