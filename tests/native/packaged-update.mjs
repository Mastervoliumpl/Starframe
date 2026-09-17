// Run only after preparing the two separately identified NSIS fixtures.
import { chromium, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { execFileSync, spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { waitForDesktopPage } from './page.mjs';
import { waitForDebugPortRelease } from './port.mjs';
import { fixtureGame } from './fixture-game.mjs';

const evidence = resolve('test-results/0.6.0-updates');
const installed = join(evidence, 'installed');
const executable = join(installed, 'starframe.exe');
const product = 'Starframe Updater Test';
const registry = `HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${product}`;
const powershell = (command) =>
  execFileSync('powershell.exe', ['-NoProfile', '-Command', command], {
    windowsHide: true,
    encoding: 'utf8',
  }).trim();
if (powershell(`Test-Path -LiteralPath '${registry}'`) !== 'False')
  throw new Error(
    'An updater fixture is already registered. Inspect it before retrying.',
  );
await waitForDebugPortRelease(9224);
const { root, data, engine, env } = await fixtureGame();
await mkdir(data, { recursive: true });
const installer = await readFile(join(evidence, 'new-setup.exe'));
const signature = (
  await readFile(join(evidence, 'new-setup.exe.sig'), 'utf8')
).trim();
const pubkey = (
  await readFile(join(evidence, 'fixture.key.pub'), 'utf8')
).trim();
const version = '0.6.0-dev.1';
const official = `https://github.com/Mastervoliumpl/Starframe/releases/download/v${version}`;
const server = createServer((req, res) => {
  if (req.url.startsWith('/repos/'))
    res.end(
      JSON.stringify([
        {
          tag_name: `v${version}`,
          draft: false,
          prerelease: true,
          body: 'Disposable packaged updater verification.',
          assets: [
            {
              name: 'latest.json',
              browser_download_url: `${official}/latest.json`,
            },
            {
              name: `Starframe_${version}_x64-setup.exe`,
              browser_download_url: `${official}/Starframe_${version}_x64-setup.exe`,
            },
          ],
        },
      ]),
    );
  else if (req.url.endsWith('latest.json'))
    res.end(
      JSON.stringify({
        version,
        platforms: {
          'windows-x86_64-nsis': {
            signature,
            url: `${official}/Starframe_${version}_x64-setup.exe`,
          },
        },
      }),
    );
  else if (req.url.endsWith('.exe')) res.end(installer);
  else res.writeHead(404).end();
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
await writeFile(
  join(data, 'update-fixture.json'),
  JSON.stringify({
    origin: `http://127.0.0.1:${server.address().port}/`,
    pubkey,
  }),
);
const environment = {
  ...process.env,
  ...env,
  NO_PROXY: '127.0.0.1',
  STARFRAME_TEST_DATA_DIR: data,
  STARFRAME_TEST_DEBUG_PORT: '9224',
};
const run = (file, args) =>
  new Promise((done, reject) => {
    const child = spawn(file, args, {
      windowsHide: true,
      stdio: 'ignore',
      env: environment,
    });
    const timer = setTimeout(
      () =>
        reject(
          new Error(
            'Fixture process exceeded two minutes. Inspect it before cleanup.',
          ),
        ),
      120000,
    );
    child.once('error', reject);
    child.once('exit', (code) => {
      clearTimeout(timer);
      if (code === 0) done();
      else reject(new Error(`Fixture process returned ${code}`));
    });
  });
async function connect() {
  for (let i = 0; i < 120; i++) {
    try {
      return await chromium.connectOverCDP('http://127.0.0.1:9224', {
        timeout: 1000,
      });
    } catch {
      await delay(500);
    }
  }
  throw new Error('Updated desktop did not reopen its test connection.');
}
let browser;
let desktop;
try {
  await run(join(evidence, 'old-setup.exe'), ['/S', `/D=${installed}`]);
  desktop = spawn(executable, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: environment,
  });
  browser = await connect();
  let page = await waitForDesktopPage(browser);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Starframe 0.6.0-dev.0', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: /^Use installation:/ }),
  ).toBeEnabled({ timeout: 30000 });
  await page.getByRole('button', { name: /^Use installation:/ }).click();
  await expect(
    page
      .getByText('The saved mod setup is ready to launch.', { exact: true })
      .first(),
  ).toBeVisible({ timeout: 30000 });
  const invoke = (command, action) =>
    page.evaluate(
      ({ command, action }) =>
        window.__TAURI_INTERNALS__.invoke(command, { action }),
      { command, action },
    );
  let mods = await invoke('mod_action', { kind: 'list' });
  mods = await invoke('mod_action', {
    kind: 'create_collection',
    name: 'Preserved update collection',
    expectedRevision: mods.revision,
  });
  const source = join(root, 'source');
  await mkdir(join(source, 'LJ/lua'), { recursive: true });
  await writeFile(
    join(source, 'LJ/lua/update.lua'),
    "return 'retained update fixture'\n",
  );
  await writeFile(
    join(source, 'starframe.local.json'),
    JSON.stringify({
      schemaVersion: 1,
      modId: 'fixture.update',
      name: 'Retained update fixture',
      author: 'Test fixture',
      version: '1',
      layout: { kind: 'starframe_lua_zip' },
    }),
  );
  await invoke('package_action', {
    kind: 'import_local',
    requestId: crypto.randomUUID(),
    path: source,
  });
  await expect
    .poll(
      async () => (await invoke('mod_action', { kind: 'list' })).library.length,
      { timeout: 30000 },
    )
    .toBe(1);
  mods = await invoke('mod_action', { kind: 'list' });
  mods = await invoke('mod_action', {
    kind: 'set_enabled',
    reference: mods.library[0].reference,
    enabled: true,
    expectedRevision: mods.revision,
  });
  const retained = {
    library: mods.library,
    collections: mods.collections,
    activeCollection: mods.activeCollection,
  };
  const config = join(engine, 'BepInEx/config/update-fixture.cfg');
  await mkdir(join(engine, 'BepInEx/config'), { recursive: true });
  await writeFile(config, 'retained-setting=true\n');
  await expect(
    page.getByRole('button', { name: 'Update', exact: true }),
  ).toBeEnabled({ timeout: 30000 });
  await page.getByRole('button', { name: 'Update', exact: true }).click();
  await expect.poll(() => desktop.exitCode, { timeout: 90000 }).toBe(0);
  await browser.close();
  browser = undefined;
  browser = await connect();
  page = await waitForDesktopPage(browser);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Starframe 0.6.0-dev.1', exact: true }),
  ).toBeVisible({ timeout: 30000 });
  await expect(
    page
      .getByText('The saved mod setup is ready to launch.', { exact: true })
      .first(),
  ).toBeVisible({ timeout: 30000 });
  const updated = await invoke('mod_action', { kind: 'list' });
  expect({
    library: updated.library,
    collections: updated.collections,
    activeCollection: updated.activeCollection,
  }).toEqual(retained);
  expect(await readFile(config, 'utf8')).toBe('retained-setting=true\n');
  expect(await readFile(join(source, 'LJ/lua/update.lua'), 'utf8')).toContain(
    'retained update fixture',
  );
  expect(
    powershell(`(Get-ItemProperty -LiteralPath '${registry}').DisplayVersion`),
  ).toBe(version);
  await page.screenshot({
    path: join(evidence, 'packaged-update-complete.png'),
  });
  // The restarted process is identified by the fixture's exact executable path.
  powershell(
    `Get-Process -Name starframe | Where-Object { $_.Path -eq '${executable}' } | ForEach-Object { $_.CloseMainWindow() | Out-Null }`,
  );
  await browser.close();
  browser = undefined;
  await waitForDebugPortRelease(9224);
  await run(join(installed, 'uninstall.exe'), ['/S', '/KEEPDATA']);
  await writeFile(
    join(evidence, 'packaged-result.json'),
    JSON.stringify(
      {
        oldVersion: '0.6.0-dev.0',
        newVersion: version,
        signatureVerified: true,
        automaticRestart: true,
        libraryCollectionsAndSettingsRetained: true,
        runtimeReady: true,
        fixtureUninstalled: true,
      },
      null,
      2,
    ),
  );
  console.log(
    'Packaged updater passed: signed NSIS 0.6.0-dev.0 -> 0.6.0-dev.1, automatic restart, retained library/collection/config/source and automatic runtime readiness. Disposable fixture uninstalled.',
  );
} finally {
  await browser?.close();
  if (desktop && desktop.exitCode === null) desktop.kill();
  server.closeAllConnections();
  await new Promise((done) => server.close(done));
}
