import { expect } from '@playwright/test';
import { createServer } from 'node:http';
import { execFileSync, spawn } from 'node:child_process';
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { withDesktop } from './session.mjs';

const root = await mkdtemp(join(tmpdir(), 'starframe-updates-'));
const cli = resolve('node_modules/@tauri-apps/cli/tauri.js');
const runCli = (...args) =>
  execFileSync(process.execPath, [cli, ...args], {
    windowsHide: true,
    stdio: 'pipe',
    timeout: 30000,
  });
const key = join(root, 'test.key');
runCli('signer', 'generate', '--ci', '-w', key);
const artifact = join(root, 'fixture.bin');
const bytes = Buffer.from(
  'Inert Starframe updater signature fixture. This is not an executable.',
);
await writeFile(artifact, bytes);
runCli('signer', 'sign', '-f', key, '-p', '', artifact);
const signature = (await readFile(`${artifact}.sig`, 'utf8')).trim();
const nextKey = join(root, 'next.key');
runCli('signer', 'generate', '--ci', '-w', nextKey);
runCli('signer', 'sign', '-f', nextKey, '-p', '', artifact);
const nextSignature = (await readFile(`${artifact}.sig`, 'utf8')).trim();
const official = 'https://github.com/Mastervoliumpl/Starframe/releases';
const installedVersion = (await readFile(resolve('VERSION'), 'utf8')).trim();
const versionParts = /^(\d+)\.(\d+)\.\d+/.exec(installedVersion);
if (!versionParts) throw new Error('Invalid installed version fixture.');
let version = `${versionParts[1]}.${Number(versionParts[2]) + 1}.0-alpha.1`;
let mode = 'available';
let requests = 0;
const server = createServer((req, res) => {
  if (req.url.startsWith('/repos/')) {
    requests++;
    if (mode === 'offline') {
      res.writeHead(503, { 'Retry-After': '600' }).end();
      return;
    }
    const prefix = `${official}/download/v${version}`;
    res.setHeader('Content-Type', 'application/json');
    res.end(
      JSON.stringify(
        mode === 'withdrawn'
          ? []
          : [
              {
                tag_name: `v${version}`,
                draft: false,
                prerelease: version.includes('-'),
                body: 'Native updater fixture notes.',
                assets: [
                  {
                    name: 'latest.json',
                    browser_download_url: `${prefix}/latest.json`,
                  },
                  {
                    name: `Starframe_${version}_x64-setup.exe`,
                    browser_download_url: `${prefix}/Starframe_${version}_x64-setup.exe`,
                  },
                ],
              },
            ],
      ),
    );
  } else if (req.url.endsWith('/latest.json')) {
    res.setHeader('Content-Type', 'application/json');
    res.end(
      JSON.stringify({
        version,
        notes: 'Native fixture',
        platforms: {
          'windows-x86_64-nsis': {
            signature:
              mode === 'bad-signature'
                ? 'invalid'
                : mode === 'next-key'
                  ? nextSignature
                  : signature,
            url: `${official}/download/v${version}/Starframe_${version}_x64-setup.exe`,
          },
        },
      }),
    );
  } else if (req.url.endsWith('.exe')) {
    res.end(
      mode === 'tampered'
        ? Buffer.concat([bytes, Buffer.from('changed')])
        : bytes,
    );
  } else {
    res.writeHead(404).end();
  }
});
await new Promise((ready) => server.listen(0, '127.0.0.1', ready));
await writeFile(
  join(root, 'update-fixture.json'),
  JSON.stringify({
    origin: `http://127.0.0.1:${server.address().port}/`,
    pubkey: (await readFile(`${key}.pub`, 'utf8')).trim(),
  }),
);
const env = {
  HTTPS_PROXY: 'http://127.0.0.1:1',
  HTTP_PROXY: 'http://127.0.0.1:1',
  NO_PROXY: '127.0.0.1',
};
const settings = async (page) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Check for updates', exact: true }),
  ).toBeEnabled({ timeout: 30000 });
};
const install = (page) =>
  page.getByRole('button', { name: 'Update', exact: true }).click();
try {
  await withDesktop(
    root,
    async (page) => {
      await settings(page);
      await expect(
        page.getByRole('heading', { name: `Update available: ${version}` }),
      ).toBeVisible();
      await page
        .getByRole('heading', { name: `Update available: ${version}` })
        .scrollIntoViewIfNeeded();
      await mkdir(resolve('test-results/0.6.0-updates'), { recursive: true });
      await page.screenshot({
        path: resolve('test-results/0.6.0-updates/update-settings.png'),
      });
      await page.getByRole('button', { name: 'Later', exact: true }).click();
      await expect(
        page.getByRole('button', { name: `Update available · ${version}` }),
      ).toHaveCount(0);
    },
    env,
  );
  await withDesktop(
    root,
    async (page) => {
      await settings(page);
      await expect(
        page.getByRole('button', { name: `Update available · ${version}` }),
      ).toHaveCount(0);
      await expect(
        page.getByRole('heading', { name: `Update available: ${version}` }),
      ).toBeVisible();
      await page
        .getByLabel('Update channel', { exact: true })
        .selectOption('stable');
      await expect(
        page.getByText('No newer release is available on this channel.'),
      ).toBeVisible({ timeout: 30000 });
      await page
        .getByLabel('Update channel', { exact: true })
        .selectOption('preview');
      await expect(
        page.getByRole('heading', { name: `Update available: ${version}` }),
      ).toBeVisible({ timeout: 30000 });
      for (const failure of ['bad-signature', 'tampered', 'next-key']) {
        mode = failure;
        await install(page);
        await expect(
          page.getByText(
            /The update download or signature verification failed/,
          ),
        ).toBeVisible({ timeout: 30000 });
        await expect(
          page.getByRole('button', { name: 'Update', exact: true }),
        ).toBeEnabled();
      }
      mode = 'withdrawn';
      await install(page);
      await expect(
        page.getByText(
          'The release changed or was removed. Check for updates again.',
        ),
      ).toBeVisible({ timeout: 30000 });
      mode = 'available';
      const fakeGame = join(root, 'Sanctuary.exe');
      await copyFile(
        join(process.env.SystemRoot, 'System32/ping.exe'),
        fakeGame,
      );
      const game = spawn(fakeGame, ['-t', '127.0.0.1'], {
        windowsHide: true,
        stdio: 'ignore',
      });
      try {
        mode = 'next-key';
        await writeFile(
          join(root, 'update-fixture.json'),
          JSON.stringify({
            origin: `http://127.0.0.1:${server.address().port}/`,
            pubkey: (await readFile(`${nextKey}.pub`, 'utf8')).trim(),
          }),
        );
        await install(page);
        await expect(
          page.getByText(
            'Installer verified. Waiting for the game and active file work to finish…',
          ),
        ).toBeVisible({ timeout: 30000 });
        await page
          .getByRole('button', { name: 'My mods', exact: true })
          .click();
        await page
          .getByRole('button', { name: 'Settings', exact: true })
          .click();
        await page
          .getByRole('button', { name: 'Cancel update', exact: true })
          .click();
        await expect(
          page.getByText(
            'Update cancelled. The installed version is unchanged.',
          ),
        ).toBeVisible();
      } finally {
        game.kill();
      }
      mode = 'offline';
      await page
        .getByRole('button', { name: 'Check for updates', exact: true })
        .click();
      await expect(
        page.getByText(/GitHub update check returned 503/),
      ).toBeVisible({ timeout: 30000 });
      const checked = requests;
      await page
        .getByRole('button', { name: 'Check for updates', exact: true })
        .click();
      await expect(
        page.getByRole('heading', { name: `Update available: ${version}` }),
      ).toBeVisible();
      expect(requests).toBe(checked);
    },
    env,
  );
  const stopped = requests;
  await new Promise((done) => setTimeout(done, 2500));
  expect(requests).toBe(stopped);
  console.log(
    'Native updater passed: signed download, tamper/signature rejection, withdrawal, channel selection, retained Later, waiting for game, cancellation, offline backoff and shutdown. No installer executed.',
  );
} finally {
  server.closeAllConnections();
  await new Promise((done) => server.close(done));
}
