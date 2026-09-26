import { expect } from '@playwright/test';
import { randomUUID } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { withDesktop } from './session.mjs';
import { installedRegistry } from './fixture-registry.mjs';

const data = await mkdtemp(join(tmpdir(), 'starframe-packages-'));
const offline = {
  HTTPS_PROXY: 'http://127.0.0.1:1',
  HTTP_PROXY: 'http://127.0.0.1:1',
  NO_PROXY: '',
};
const invoke = (page, command, action) =>
  page.evaluate(
    ({ command, action }) =>
      window.__TAURI_INTERNALS__.invoke(command, { action }),
    { command, action },
  );
const view = (page) => invoke(page, 'mod_action', { kind: 'list' });
const sharing = (page, action) => invoke(page, 'sharing_action', action);
await withDesktop(
  data,
  async (page) => {
    await expect.poll(async () => (await view(page)).library.length).toBe(0);
  },
  offline,
);
const bytes = Buffer.from('return "inert offline native fixture"');
const path = 'LJ/lua/fixture.lua';
const reference = await installedRegistry(
  data,
  1,
  'Native fixture',
  { [path]: bytes },
  {
    schemaVersion: 1,
    kind: 'code',
    loader: 'lua',
    entryPath: path,
  },
);
const file = join(data, 'artifacts', reference.reference.sha256, path);
const document = {
  format: 'starframe-collection',
  schemaVersion: 2,
  name: 'Native shared collection',
  entries: [reference],
};
let collection;
await withDesktop(
  data,
  async (page) => {
    await expect.poll(async () => (await view(page)).library.length).toBe(1);
    const review = await sharing(page, {
      kind: 'review',
      text: JSON.stringify(document),
    });
    expect(review.entries[0].status).toBe('pending');
    const requestId = randomUUID();
    const action = {
      kind: 'accept',
      text: JSON.stringify(document),
      requestId,
      expectedRevision: (await view(page)).revision,
    };
    const accepted = await sharing(page, action);
    expect((await sharing(page, action)).collectionId).toBe(
      accepted.collectionId,
    );
    collection = accepted.collectionId;
    await expect
      .poll(async () => (await view(page)).imports[0].entries[0].status)
      .toBe('ready');
    const operations = await invoke(page, 'package_action', { kind: 'list' });
    expect(operations).toHaveLength(1);
    expect(operations[0].kind).toBe('registry_verification');
    expect(operations[0].status).toBe('completed');
    expect(operations[0].receiptId).toBeNull();
    await page
      .getByRole('button', { name: 'Collections', exact: true })
      .click();
    await expect(page.getByText('1 of 1 exact packages ready')).toBeVisible();
    await page
      .getByRole('button', {
        name: 'Export collection Native shared collection',
      })
      .click();
    expect(
      JSON.parse(
        await page
          .getByRole('textbox', { name: 'Collection JSON', exact: true })
          .inputValue(),
      ),
    ).toEqual(document);
    await page.keyboard.press('Escape');
    await writeFile(file, 'changed retained fixture');
    await sharing(page, { kind: 'retry', id: collection });
    await expect
      .poll(async () => (await view(page)).imports[0].entries[0].status)
      .toBe('unresolved');
    expect((await view(page)).collections[0].entries).toEqual([reference]);
  },
  offline,
);
await withDesktop(
  data,
  async (page) => {
    expect((await view(page)).imports[0].entries[0].status).toBe('unresolved');
    await writeFile(file, bytes);
    await sharing(page, { kind: 'retry', id: collection });
    await expect
      .poll(async () => (await view(page)).imports[0].entries[0].status)
      .toBe('ready');
    const before = await view(page);
    await invoke(page, 'mod_action', {
      kind: 'select_collection',
      id: collection,
      expectedRevision: before.revision,
    });
    expect((await view(page)).orderError).toBeNull();
  },
  offline,
);
// Retained state only. Signature and revision acceptance use synthetic signed Rust fixtures.
for (const [index, status] of ['blocked', 'cleared'].entries()) {
  const db = new DatabaseSync(join(data, 'sqlite/state.db'));
  db.prepare(
    'INSERT INTO registry_decisions(sha256,revision,record) VALUES (?,?,?) ON CONFLICT(sha256) DO UPDATE SET revision=excluded.revision,record=excluded.record',
  ).run(
    reference.reference.sha256,
    index + 1,
    JSON.stringify({
      sha256: reference.reference.sha256,
      revision: index + 1,
      status,
      reason: 'Synthetic native security decision',
    }),
  );
  db.close();
  await withDesktop(
    data,
    async (page) => {
      const control = page.getByRole('switch', {
        name: 'Enable Native fixture 1',
        exact: true,
      });
      if (status === 'blocked') {
        const current = await view(page);
        expect(current.blocked[reference.reference.sha256]).toBe(
          'Synthetic native security decision',
        );
        expect(current.orderError).toContain('signed registry');
        const rejected = await sharing(page, {
          kind: 'review',
          text: JSON.stringify(document),
        });
        expect(rejected.entries[0].status).toBe('unresolved');
        await control.click();
        await expect(control).not.toBeChecked();
        await expect(control).toBeDisabled();
      } else {
        await expect(control).toBeEnabled();
        expect((await view(page)).orderError).toBeNull();
      }
    },
    offline,
  );
}
expect(await readFile(file)).toEqual(bytes);
await withDesktop(
  data,
  async (page) => {
    const missing = {
      kind: 'registry',
      reference: { modId: 2, releaseId: randomUUID(), sha256: 'ab'.repeat(32) },
    };
    const mixed = {
      ...document,
      name: 'Scoped online requirement',
      entries: [reference, missing],
    };
    const reply = await sharing(page, {
      kind: 'accept',
      text: JSON.stringify(mixed),
      requestId: randomUUID(),
      expectedRevision: (await view(page)).revision,
    });
    const imported = async () =>
      (await view(page)).imports.find(
        (entry) => entry.collectionId === reply.collectionId,
      );
    await expect
      .poll(async () => (await imported()).entries[0].status)
      .toBe('ready');
    await expect
      .poll(async () => (await imported()).entries[1].status)
      .toBe('unresolved');
    expect((await imported()).entries[1].message).toContain(
      'independently provisioned registry root',
    );
    expect((await imported()).entries.map((entry) => entry.reference)).toEqual(
      mixed.entries,
    );
    expect(
      (await invoke(page, 'package_action', { kind: 'list' })).every(
        (entry) => entry.kind === 'registry_verification',
      ),
    ).toBe(true);
    expect((await view(page)).orderError).toBeNull();
  },
  offline,
);
console.log(
  'Native exact-reference import, offline verification, corruption, restart, export, retained security controls and scoped missing registry requirements passed.',
);
