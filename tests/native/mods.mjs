import { expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { installedRegistry, runtimeId } from './fixture-registry.mjs';
import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile, access } from 'node:fs/promises';
import { join } from 'node:path';
import { withDesktop } from './session.mjs';
import { fixtureGame } from './fixture-game.mjs';

const { data, steam, engine, env } = await fixtureGame();
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const action = async (page, action) => {
  await page.waitForFunction(() => !!window.__TAURI_INTERNALS__?.invoke);
  const result = await page.evaluate(async (action) => {
    try {
      return {
        ok: true,
        value: await window.__TAURI_INTERNALS__.invoke('mod_action', {
          action,
        }),
      };
    } catch (error) {
      return { ok: false, error };
    }
  }, action);
  if (!result.ok) throw result.error;
  return result.value;
};
const list = (page) => action(page, { kind: 'list' });
await withDesktop(
  data,
  async (page) => {
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('button', { name: /^Use installation:/ }).click();
    await expect.poll(async () => (await list(page)).library.length).toBe(0);
    await page
      .getByRole('button', { name: 'Repair or reinstall runtime' })
      .click();
    await expect
      .poll(
        async () =>
          readFile(join(engine, 'Starframe/activation.json'), 'utf8').then(
            (value) => JSON.parse(value).mods.length,
            () => -1,
          ),
        { timeout: 30000 },
      )
      .toBe(0);
  },
  env,
);

const bytes = Buffer.from('inert managed mod fixture');
const reference = await installedRegistry(
  data,
  1,
  'Native fixture',
  { 'package/Fixture.dll': bytes },
  {
    schemaVersion: 1,
    kind: 'code',
    loader: 'bepinex5',
    sourceRoot: 'package',
    entryAssembly: 'Fixture.dll',
    entryType: 'Fixture.Entry',
  },
);
const artifactRoot = join(data, 'artifacts', reference.reference.sha256);
const luaBytes = Buffer.from('return "native order fixture"');
const luaPath = 'LJ/lua/starframe_fixture.lua';
const luaReference = await installedRegistry(
  data,
  2,
  'Lua native fixture',
  { [luaPath]: luaBytes },
  {
    schemaVersion: 1,
    kind: 'code',
    loader: 'lua',
    entryPath: luaPath,
  },
  [
    {
      kind: 'exact',
      modId: reference.reference.modId,
      releaseId: reference.reference.releaseId,
    },
  ],
);
const luaId = runtimeId(luaReference);
const luaFile = join(data, 'artifacts', luaReference.reference.sha256, luaPath);
let deployed;
const activation = async () =>
  JSON.parse(await readFile(join(engine, 'Starframe/activation.json'), 'utf8'));
const waitForDeployment = (page, count) =>
  expect
    .poll(
      async () => {
        // The worker replies after file work; reading activation alone can see an unfinished deployment.
        const confirmed = await list(page);
        const current = await activation();
        const mod = current.mods.find((m) => m.modId === runtimeId(reference));
        if (mod)
          deployed = join(engine, 'Starframe', mod.root, mod.entryAssembly);
        return {
          revisionMatches: current.deploymentRevision === confirmed.revision,
          mods: current.mods.length,
          payload: deployed
            ? await readFile(deployed).then(hash, (error) => {
                if (error.code === 'ENOENT') return null;
                throw error;
              })
            : null,
        };
      },
      { timeout: 30000 },
    )
    .toEqual({
      revisionMatches: true,
      mods: count,
      payload: count ? hash(bytes) : null,
    });
const exists = (path) =>
  access(path).then(
    () => true,
    () => false,
  );
let running;
let originalCollection;
let spareCollection;
const collectionAction = async (page, change) =>
  action(page, { ...change, expectedRevision: (await list(page)).revision });
try {
  await withDesktop(
    data,
    async (page) => {
      await expect.poll(async () => (await list(page)).library.length).toBe(2);
      await page
        .getByRole('switch', { name: 'Enable Native fixture 1', exact: true })
        .click();
      await expect(
        page.getByRole('switch', {
          name: 'Enable Native fixture 1',
          exact: true,
        }),
      ).toBeChecked();
      await waitForDeployment(page, 1);
      originalCollection = (await list(page)).activeCollection;
      const nativeRow = page
        .getByRole('region', { name: 'My mods', exact: true })
        .getByRole('listitem')
        .filter({
          has: page.getByRole('button', {
            name: 'Native fixture',
            exact: true,
          }),
        });
      await expect(
        nativeRow.getByText('Tested with this version', { exact: true }),
      ).toBeVisible();
      await writeFile(
        join(steam, 'steamapps/appmanifest_4511930.acf'),
        '"AppState" { "appid" "4511930" "installdir" "Fixture Sanctuary" "buildid" "222" "StateFlags" "4" }',
      );
      await expect(
        nativeRow.getByText('Not tested with this version', { exact: true }),
      ).toBeVisible({ timeout: 40000 });
      await expect(
        page.getByRole('button', {
          name: 'Launch Sanctuary Shattered Sun',
          exact: true,
        }),
      ).toBeEnabled();
      const created = await collectionAction(page, {
        kind: 'create_collection',
        name: 'Spare fixture',
      });
      spareCollection = created.collections.find(
        (c) => c.name === 'Spare fixture',
      ).id;
      await collectionAction(page, {
        kind: 'rename_collection',
        id: spareCollection,
        name: 'Empty fixture',
      });
      const keptSettings = join(
        engine,
        'BepInEx/config/collection-fixture.cfg',
      );
      await mkdir(join(engine, 'BepInEx/config'), { recursive: true });
      await writeFile(keptSettings, 'settings survive collection edits');
      await collectionAction(page, {
        kind: 'select_collection',
        id: spareCollection,
      });
      await waitForDeployment(page, 0);
      await collectionAction(page, {
        kind: 'select_collection',
        id: originalCollection,
      });
      await waitForDeployment(page, 1);
      expect(await readFile(keptSettings, 'utf8')).toBe(
        'settings survive collection edits',
      );
      running = spawn(join(engine, 'Sanctuary.exe'), ['-t', '127.0.0.1'], {
        windowsHide: true,
        stdio: 'ignore',
      });
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(
        page.getByText('Game running', { exact: true }),
      ).toBeVisible();
      const pendingCollection = await collectionAction(page, {
        kind: 'select_collection',
        id: spareCollection,
      });
      expect((await activation()).mods).toHaveLength(1);
      running.kill();
      await expect
        .poll(() => running.exitCode !== null || running.signalCode !== null)
        .toBe(true);
      await waitForDeployment(page, 0);
      expect((await activation()).deploymentRevision).toBe(
        pendingCollection.revision,
      );
      await collectionAction(page, {
        kind: 'select_collection',
        id: originalCollection,
      });
      await waitForDeployment(page, 1);
      expect(await readFile(deployed)).toEqual(bytes);
      await action(page, {
        kind: 'set_enabled',
        reference: luaReference,
        enabled: true,
        expectedRevision: (await list(page)).revision,
      });
      await waitForDeployment(page, 2);
      const beforeOrder = await list(page);
      const reordered = await action(page, {
        kind: 'reorder',
        modIds: [luaId, runtimeId(reference)],
        expectedRevision: beforeOrder.revision,
      });
      expect(reordered.enabled.map(runtimeId)).toEqual([
        luaId,
        runtimeId(reference),
      ]);
      expect(reordered.order.effective.map(runtimeId)).toEqual([
        runtimeId(reference),
        luaId,
      ]);
      await expect
        .poll(async () => (await activation()).deploymentRevision, {
          timeout: 30000,
        })
        .toBe(reordered.revision);
      expect((await activation()).mods.map((m) => m.modId)).toEqual([
        runtimeId(reference),
        luaId,
      ]);
      await page
        .getByRole('button', { name: 'Collections', exact: true })
        .click();
      await expect(
        page
          .getByRole('list', { name: 'Effective load order' })
          .getByRole('listitem')
          .first(),
      ).toContainText('Native fixture');
      await expect(
        page.getByRole('region', { name: 'Collections', exact: true }),
      ).toContainText('Native fixture must load before Lua native fixture');
      await page.screenshot({
        path: 'test-results/native/load-order-live.png',
      });
      const sharing = (change) =>
        page.evaluate(
          (action) =>
            window.__TAURI_INTERNALS__.invoke('sharing_action', { action }),
          change,
        );
      const exported = await sharing({
        kind: 'export',
        id: originalCollection,
      });
      await writeFile(luaFile, 'changed retained fixture');
      const imported = await sharing({
        kind: 'accept',
        text: exported.text,
        requestId: crypto.randomUUID(),
        expectedRevision: (await list(page)).revision,
      });
      await expect
        .poll(async () =>
          (await list(page)).imports
            .find((i) => i.collectionId === imported.collectionId)
            ?.entries.some((e) => e.status === 'unresolved'),
        )
        .toBe(true);
      const beforeImport = await activation();
      await collectionAction(page, {
        kind: 'select_collection',
        id: imported.collectionId,
      });
      await expect
        .poll(async () => (await list(page)).orderError)
        .not.toBeNull();
      expect((await activation()).deploymentRevision).toBe(
        beforeImport.deploymentRevision,
      );
      expect((await activation()).mods.map((m) => m.modId)).toEqual([
        runtimeId(reference),
        luaId,
      ]);
      await writeFile(luaFile, luaBytes);
      await collectionAction(page, {
        kind: 'select_collection',
        id: originalCollection,
      });
      await collectionAction(page, {
        kind: 'delete_collection',
        id: imported.collectionId,
      });
      await action(page, {
        kind: 'set_enabled',
        reference: luaReference,
        enabled: false,
        expectedRevision: (await list(page)).revision,
      });
      await waitForDeployment(page, 1);
      await page.getByRole('button', { name: 'My mods', exact: true }).click();
      await page.screenshot({ path: 'test-results/native/my-mods-live.png' });
      await page
        .getByRole('button', { name: 'Native fixture', exact: true })
        .click();
      await page.screenshot({
        path: 'test-results/native/mod-details-live.png',
      });
      await page.getByRole('button', { name: 'Back to list' }).click();
      running = spawn(join(engine, 'Sanctuary.exe'), ['-t', '127.0.0.1'], {
        windowsHide: true,
        stdio: 'ignore',
      });
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(
        page.getByText('Game running', { exact: true }),
      ).toBeVisible();
      await collectionAction(page, {
        kind: 'select_collection',
        id: spareCollection,
      });
      await collectionAction(page, {
        kind: 'select_collection',
        id: originalCollection,
      });
      await collectionAction(page, {
        kind: 'select_collection',
        id: spareCollection,
      });
      const deleted = await collectionAction(page, {
        kind: 'delete_collection',
        id: spareCollection,
      });
      expect(deleted.activeCollection).toBeNull();
      expect(deleted.library).toHaveLength(2);
      await expect(page.locator('footer')).toContainText(
        `Saved collection revision ${deleted.revision}`,
      );
      expect((await activation()).mods).toHaveLength(1);
      expect(await exists(deployed)).toBe(true);
    },
    env,
  );
  expect(running.exitCode).toBeNull();
  running.kill();
  await expect
    .poll(() => running.exitCode !== null || running.signalCode !== null)
    .toBe(true);
  await withDesktop(
    data,
    async (page) => {
      await waitForDeployment(page, 0);
      expect(await exists(deployed)).toBe(false);
      expect(await exists(join(artifactRoot, 'package/Fixture.dll'))).toBe(
        true,
      );
      const restored = await collectionAction(page, {
        kind: 'select_collection',
        id: originalCollection,
      });
      expect(
        restored.collections.find((c) => c.id === originalCollection).name,
      ).toBe('Default');
      await waitForDeployment(page, 1);
      // Direct IPC changed the collection; wait for the view's next refresh before opening a revision-bound dialog.
      await expect(
        page.getByRole('switch', {
          name: 'Enable Native fixture 1',
          exact: true,
        }),
      ).toBeChecked();
      expect(
        await readFile(
          join(engine, 'BepInEx/config/collection-fixture.cfg'),
          'utf8',
        ),
      ).toBe('settings survive collection edits');
      const settings = join(engine, 'BepInEx/config/fixture.cfg');
      await mkdir(join(engine, 'BepInEx/config'), { recursive: true });
      await writeFile(settings, 'retain user settings');
      const expectedRevision = (await list(page)).revision;
      const error = await action(page, {
        kind: 'uninstall',
        reference,
        expectedRevision,
        confirmReferences: false,
      }).catch((error) => error);
      expect(error.message).toContain('affects collections');
      await page
        .getByRole('button', {
          name: 'Uninstall Native fixture 1',
          exact: true,
        })
        .click();
      await expect(page.getByRole('dialog')).toContainText('Default');
      await page.getByRole('button', { name: 'Confirm uninstall' }).click();
      await expect(page.getByRole('dialog')).not.toBeVisible();
      await waitForDeployment(page, 0);
      expect((await list(page)).library).toHaveLength(1);
      expect(await exists(artifactRoot)).toBe(false);
      expect(await readFile(settings, 'utf8')).toBe('retain user settings');
    },
    env,
  );
} finally {
  if (running?.exitCode === null) running.kill();
}
console.log(
  'PASS: native named collection creation, rename, selection, deletion without uninstall, settings retention, game-exit and restart application, requested/effective ordering and Lua payload deployment.',
);
