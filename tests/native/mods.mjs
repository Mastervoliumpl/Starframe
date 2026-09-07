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
const exists = (path) =>
  access(path).then(
    () => true,
    () => false,
  );
const setEnabled = async (page, enabled) =>
  action(page, {
    kind: 'set_enabled',
    modId: reference.modId,
    hash: artifactHash,
    enabled,
    expectedRevision: (await list(page)).revision,
  });
let running;
try {
  await withDesktop(
    data,
    async (page) => {
      await expect.poll(async () => (await list(page)).library.length).toBe(1);
      await page
        .getByRole('switch', { name: 'Enable Native fixture 1', exact: true })
        .click();
      await expect(
        page.getByRole('switch', {
          name: 'Enable Native fixture 1',
          exact: true,
        }),
      ).toBeChecked();
      await expect
        .poll(async () => (await activation()).mods.length, { timeout: 30000 })
        .toBe(1);
      expect(await readFile(deployed)).toEqual(bytes);
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
      await setEnabled(page, false);
      await setEnabled(page, true);
      await setEnabled(page, false);
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
      await expect
        .poll(async () => (await activation()).mods.length, { timeout: 30000 })
        .toBe(0);
      expect(await exists(deployed)).toBe(false);
      expect(await exists(join(artifactRoot, 'package/Fixture.dll'))).toBe(
        true,
      );
      await setEnabled(page, true);
      await expect
        .poll(async () => (await activation()).mods.length, { timeout: 30000 })
        .toBe(1);
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
      await expect
        .poll(async () => (await activation()).mods.length, { timeout: 30000 })
        .toBe(0);
      expect((await list(page)).library).toHaveLength(0);
      expect(await exists(artifactRoot)).toBe(false);
      expect(await readFile(settings, 'utf8')).toBe('retain user settings');
    },
    env,
  );
} finally {
  if (running?.exitCode === null) running.kill();
}
console.log(
  'PASS: native mod membership, withdrawn installed copy, game-running deferral, restart application and uninstall retention.',
);
