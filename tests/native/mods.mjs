import { expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { spawn, execFileSync } from 'node:child_process';
import {
  mkdtemp,
  mkdir,
  readFile,
  writeFile,
  copyFile,
  access,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { withDesktop } from './session.mjs';

const root = await mkdtemp(join(tmpdir(), 'starframe-mods-'));
const data = join(root, 'data');
const steam = join(root, 'Steam');
const game = join(steam, 'steamapps/common/Fixture Sanctuary');
const engine = join(game, 'engine');
const resources = join(root, 'integration');
await mkdir(join(engine, 'Sanctuary_Data/Managed'), { recursive: true });
await mkdir(join(engine, 'MonoBleedingEdge'), { recursive: true });
await copyFile(
  join(process.env.SystemRoot, 'System32/ping.exe'),
  join(engine, 'Sanctuary.exe'),
);
await copyFile(join(engine, 'Sanctuary.exe'), join(engine, 'UnityPlayer.dll'));
await writeFile(
  join(engine, 'Sanctuary_Data/app.info'),
  'Enhearten Media PTY\nSanctuary\n',
);
await writeFile(
  join(engine, 'Sanctuary_Data/boot.config'),
  'build-guid=0123456789abcdef0123456789abcdef\n',
);
await writeFile(
  join(steam, 'steamapps/appmanifest_4511930.acf'),
  '"AppState" { "appid" "4511930" "installdir" "Fixture Sanctuary" "buildid" "111" "StateFlags" "4" }',
);
await writeFile(
  join(steam, 'steamapps/libraryfolders.vdf'),
  '"libraryfolders" {}',
);
execFileSync(
  process.env.STARFRAME_TEST_PYTHON || 'python',
  [
    'scripts/prepare_bootstrap.py',
    join(resources, 'bootstrap'),
    ...(process.env.STARFRAME_TEST_BOOTSTRAP_ARCHIVE
      ? ['--archive', process.env.STARFRAME_TEST_BOOTSTRAP_ARCHIVE]
      : []),
  ],
  { windowsHide: true, timeout: 60000, stdio: 'inherit' },
);
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const runtime = join(resources, 'runtime');
const runtimeFiles = {
  'BepInEx/plugins/Starframe/Starframe.Bootstrap.dll': Buffer.from(
    'inert bootstrap fixture',
  ),
  'BepInEx/plugins/Starframe/Starframe.Runtime.dll': Buffer.from(
    'inert runtime fixture',
  ),
  'Starframe/activation.json': await readFile(
    'contracts/fixtures/activation-empty.json',
  ),
};
for (const [path, bytes] of Object.entries(runtimeFiles)) {
  await mkdir(join(runtime, path, '..'), { recursive: true });
  await writeFile(join(runtime, path), bytes);
}
await writeFile(
  join(runtime, 'runtime-package.json'),
  JSON.stringify(
    Object.entries(runtimeFiles).map(([path, bytes]) => ({
      path,
      sha256: hash(bytes),
    })),
  ),
);
const env = {
  STARFRAME_TEST_STEAM_ROOT: steam,
  STARFRAME_INTEGRATION_DIR: resources,
  HTTPS_PROXY: 'http://127.0.0.1:1',
  HTTP_PROXY: 'http://127.0.0.1:1',
  NO_PROXY: '',
};
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
    await page.getByRole('button', { name: 'Install or retry setup' }).click();
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
const artifactHash = hash('archive identity for native mod fixture');
const reference = {
  modId: 'fixture.native',
  hash: artifactHash,
  origin: 'catalog',
  releaseId: 'fixture.native.1',
};
const artifactRoot = join(data, 'artifacts', artifactHash);
await mkdir(join(artifactRoot, 'package'), { recursive: true });
await writeFile(join(artifactRoot, 'package/Fixture.dll'), bytes);
const catalog = {
  schemaVersion: 1,
  catalogRevision: '1',
  mods: [
    {
      id: reference.modId,
      name: 'Native fixture',
      author: 'Fixture',
      sourceUrl: 'https://example.invalid/source',
      releases: [
        {
          id: reference.releaseId,
          version: '1',
          withdrawn: true,
          withdrawalReason: 'Author removed this fixture release.',
          requires: [],
          testedGameBuilds: [],
          artifact: {
            url: 'https://example.invalid/fixture.zip',
            sha256: artifactHash,
            sizeBytes: 100,
            layout: {
              kind: 'starframe_managed_zip',
              root: 'package',
              entryAssembly: 'Fixture.dll',
              entryType: 'Fixture.Entry',
            },
          },
        },
      ],
    },
  ],
};
const db = new DatabaseSync(join(data, 'sqlite/state.db'));
const luaBytes = Buffer.from('return "native order fixture"');
const luaHash = hash(luaBytes);
const luaId = 'fixture.lua';
const luaPath = 'LJ/lua/starframe_fixture.lua';
await mkdir(join(data, 'artifacts', luaHash, 'LJ/lua'), { recursive: true });
await writeFile(join(data, 'artifacts', luaHash, luaPath), luaBytes);
catalog.schemaVersion = 2;
catalog.mods.push({
  id: luaId,
  name: 'Lua native fixture',
  author: 'Fixture',
  sourceUrl: 'https://example.invalid/source',
  releases: [
    {
      id: 'fixture.lua.1',
      version: '1',
      withdrawn: false,
      requires: [reference.releaseId],
      testedGameBuilds: [],
      artifact: {
        url: 'https://example.invalid/lua.zip',
        sha256: luaHash,
        sizeBytes: luaBytes.length,
        layout: { kind: 'starframe_lua_zip' },
      },
    },
  ],
});
db.prepare(
  'INSERT INTO library(mod_id,hash,origin,release_id,name,author,version) VALUES (?,?,?,?,?,?,?)',
).run(
  luaId,
  luaHash,
  'catalog',
  'fixture.lua.1',
  'Lua native fixture',
  'Fixture',
  '1',
);
db.prepare('INSERT INTO prepared_artifacts(hash,record) VALUES (?,?)').run(
  luaHash,
  JSON.stringify({
    hash: luaHash,
    files: [
      { path: luaPath, sha256: hash(luaBytes), sizeBytes: luaBytes.length },
    ],
  }),
);
db.prepare(
  'INSERT INTO library(mod_id,hash,origin,release_id,name,author,version) VALUES (?,?,?,?,?,?,?)',
).run(
  reference.modId,
  artifactHash,
  'catalog',
  reference.releaseId,
  'Native fixture',
  'Fixture',
  '1',
);
db.prepare('INSERT INTO prepared_artifacts(hash,record) VALUES (?,?)').run(
  artifactHash,
  JSON.stringify({
    hash: artifactHash,
    files: [
      {
        path: 'package/Fixture.dll',
        sha256: hash(bytes),
        sizeBytes: bytes.length,
      },
    ],
  }),
);
db.prepare(
  'INSERT INTO catalog_cache(id,record) VALUES (1,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record',
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
db.close();
const deployed = join(
  engine,
  'Starframe/mods',
  artifactHash,
  'package/Fixture.dll',
);
const activation = async () =>
  JSON.parse(await readFile(join(engine, 'Starframe/activation.json'), 'utf8'));
const waitForDeployment = (count) =>
  expect
    .poll(
      async () => ({
        mods: (await activation()).mods.length,
        payload: await readFile(deployed).then(hash, (error) => {
          if (error.code === 'ENOENT') return null;
          throw error;
        }),
      }),
      { timeout: 30000 },
    )
    .toEqual({ mods: count, payload: count ? hash(bytes) : null });
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
      await waitForDeployment(1);
      originalCollection = (await list(page)).activeCollection;
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
      await waitForDeployment(0);
      await collectionAction(page, {
        kind: 'select_collection',
        id: originalCollection,
      });
      await waitForDeployment(1);
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
      await waitForDeployment(0);
      expect((await activation()).deploymentRevision).toBe(
        pendingCollection.revision,
      );
      await collectionAction(page, {
        kind: 'select_collection',
        id: originalCollection,
      });
      await waitForDeployment(1);
      expect(await readFile(deployed)).toEqual(bytes);
      await action(page, {
        kind: 'set_enabled',
        modId: luaId,
        hash: luaHash,
        enabled: true,
        expectedRevision: (await list(page)).revision,
      });
      await waitForDeployment(2);
      const beforeOrder = await list(page);
      const reordered = await action(page, {
        kind: 'reorder',
        modIds: [luaId, reference.modId],
        expectedRevision: beforeOrder.revision,
      });
      expect(reordered.enabled.map((r) => r.modId)).toEqual([
        luaId,
        reference.modId,
      ]);
      expect(reordered.order.effective.map((r) => r.modId)).toEqual([
        reference.modId,
        luaId,
      ]);
      await expect
        .poll(async () => (await activation()).deploymentRevision, {
          timeout: 30000,
        })
        .toBe(reordered.revision);
      expect((await activation()).mods.map((m) => m.modId)).toEqual([
        reference.modId,
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
      await action(page, {
        kind: 'set_enabled',
        modId: luaId,
        hash: luaHash,
        enabled: false,
        expectedRevision: reordered.revision,
      });
      await waitForDeployment(1);
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
      await waitForDeployment(0);
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
      await waitForDeployment(1);
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
        modId: reference.modId,
        hash: artifactHash,
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
      await waitForDeployment(0);
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
