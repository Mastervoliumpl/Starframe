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
    await page
      .getByRole('button', { name: 'Collections', exact: true })
      .click();
    await page
      .getByRole('button', { name: 'Import collection', exact: true })
      .click();
    const shared = {
      format: 'starframe-collection',
      schemaVersion: 1,
      name: 'Native shared collection',
      entries: [
        {
          modId: 'fixture.core',
          hash: archiveHash,
          origin: 'catalog',
          releaseId: 'fixture.core.1',
        },
      ],
    };
    const dialog = page.getByRole('dialog');
    await dialog
      .getByRole('textbox', { name: 'Or paste collection JSON' })
      .fill(JSON.stringify(shared));
    await dialog.getByRole('button', { name: 'Review import' }).click();
    await expect(dialog).toContainText(
      'Already downloaded; verify local files before reuse.',
    );
    await dialog.getByRole('button', { name: 'Accept import' }).click();
    await expect(page.getByText('1 of 1 exact packages ready')).toBeVisible();
    await page
      .getByRole('button', {
        name: 'Export collection Native shared collection',
      })
      .click();
    expect(
      JSON.parse(
        await dialog
          .getByRole('textbox', { name: 'Collection JSON', exact: true })
          .inputValue(),
      ),
    ).toEqual(shared);
    await page.screenshot({
      path: 'test-results/native/collection-export-live.png',
    });
    await dialog.getByRole('button', { name: 'Close', exact: true }).click();
    const invalidShare = await page.evaluate(async () =>
      window.__TAURI_INTERNALS__
        .invoke('sharing_action', {
          action: { kind: 'review', text: '{"format":"untrusted"}' },
        })
        .catch((error) => error),
    );
    expect(invalidShare.code).toBe('sharing_failed');
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
    await page
      .getByRole('button', { name: 'Collections', exact: true })
      .click();
    await expect(
      page.getByRole('heading', {
        name: 'Native shared collection',
        exact: true,
      }),
    ).toBeVisible();
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
// Seed retained state only; real signatures and corrected-history acceptance are covered by Rust TUF fixtures.
const history = [];
for (const state of ['suspected', 'confirmed', 'cleared']) {
  const db = new DatabaseSync(join(root, 'sqlite', 'state.db'));
  const cached = JSON.parse(
    db.prepare('SELECT record FROM catalog_cache WHERE id=1').get().record,
  );
  history.push({
    recordedAt: 1788819700 + history.length,
    state,
    explanation: `Synthetic ${state} evidence`,
    evidence: ['https://example.invalid/evidence'],
    recommendedAction:
      state === 'confirmed' ? 'Disable this fixture.' : 'Use is permitted.',
  });
  const now = Math.floor(Date.now() / 1000);
  db.prepare(
    'INSERT INTO catalog_security (id, record) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET record=excluded.record',
  ).run(
    JSON.stringify({
      catalogSha256: hash(JSON.stringify(cached.catalog)),
      receivedAt: now - 100,
      expires: now - 1,
      advisories: {
        schemaVersion: 1,
        revision: String(history.length),
        advisories: [
          {
            id: 'fixture.finding',
            title: 'Synthetic native finding',
            affected: [
              {
                releaseId: 'fixture.core.1',
                sha256: archiveHash,
                payloadSha256: [hash(bytes)],
              },
            ],
            history,
          },
        ],
      },
    }),
  );
  db.close();
  await withDesktop(
    root,
    async (page) => {
      const enabled = page.getByRole('switch', {
        name: 'Enable Native fixture 1',
      });
      if (state === 'confirmed') {
        await expect(enabled).toBeDisabled();
        await expect(
          page.getByRole('alert').filter({ hasText: 'installed mod matches' }),
        ).toBeVisible();
        const blocked = await page.evaluate(
          (requestId) =>
            window.__TAURI_INTERNALS__
              .invoke('package_action', {
                action: {
                  kind: 'prepare',
                  requestId,
                  releaseId: 'fixture.core.1',
                },
              })
              .catch((error) => error),
          randomUUID(),
        );
        expect(blocked.message).toContain('fixture.finding');
        await page.screenshot({
          path: 'test-results/native/security-confirmed.png',
        });
      } else {
        await expect(enabled).toBeEnabled();
      }
      await expect(
        page.getByRole('button', { name: 'Uninstall Native fixture 1' }),
      ).toBeEnabled();
      await page
        .getByRole('button', { name: 'Native fixture', exact: true })
        .click();
      const details = page.getByRole('complementary', { name: 'Mod details' });
      await expect(
        details.getByRole('heading', { name: 'Synthetic native finding' }),
      ).toBeVisible();
      await details
        .getByText('Evidence and correction history', { exact: true })
        .click();
      await expect(
        details.getByText(`Synthetic ${state} evidence`, { exact: true }),
      ).toHaveCount(2);
      await page.getByRole('button', { name: 'Catalog', exact: true }).click();
      await expect(
        page.getByText(
          'Catalog security information has expired or the clock changed.',
          { exact: false },
        ),
      ).toBeVisible();
    },
    offline,
  );
}
expect(await readFile(join(directory, 'Core.dll'), 'utf8')).toBe(
  'changed native bytes',
);
console.log(
  'Native packages passed: command permissions, offline reuse, retained failure, expired advisory controls, correction history and restart.',
);
