import { chromium, expect } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { createServer } from 'node:net';

const executable = resolve('src-tauri/target/debug/starframe.exe');
async function withDesktop(root, check) {
  const probe = createServer();
  await new Promise((accept, reject) => {
    probe.once('error', reject);
    probe.listen(9224, '127.0.0.1', accept);
  });
  await new Promise((accept) => probe.close(accept));
  const child = spawn(executable, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      STARFRAME_TEST_DATA_DIR: root,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9224',
    },
  });
  let browser;
  try {
    for (let attempt = 0; attempt < 100; attempt++) {
      try {
        browser = await chromium.connectOverCDP('http://127.0.0.1:9224');
        break;
      } catch {
        if (child.exitCode !== null)
          throw new Error('The test desktop exited before connecting.');
        await delay(100);
      }
    }
    if (!browser) throw new Error('The test desktop did not connect.');
    await check(browser.contexts()[0].pages()[0]);
    execFileSync(
      'powershell.exe',
      [
        '-NoProfile',
        '-Command',
        `(Get-Process -Id ${child.pid}).CloseMainWindow()`,
      ],
      { windowsHide: true },
    );
    await expect.poll(() => child.exitCode).toBe(0);
  } finally {
    await browser?.close();
    if (child.exitCode === null) child.kill();
  }
}

const root = await mkdtemp(join(tmpdir(), 'starframe-storage-'));
for (let run = 0; run < 2; run++) {
  await withDesktop(root, async (page) => {
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(
      page.getByText('Saved locally: 0 library entries and 0 collections.'),
    ).toBeVisible();
  });
}
const original = await readFile(join(root, 'state.db'));
for (const mode of ['corrupt', 'newer']) {
  const invalid = await mkdtemp(join(tmpdir(), 'starframe-storage-'));
  const bytes =
    mode === 'corrupt' ? Buffer.alloc(8192, 0x5a) : Buffer.from(original);
  if (mode === 'newer') bytes.writeUInt32BE(99, 60);
  await writeFile(join(invalid, 'state.db'), bytes);
  await withDesktop(invalid, async (page) => {
    await expect(page.getByRole('alert')).toContainText(
      mode === 'corrupt' ? 'header is invalid' : 'newer Starframe version',
    );
    await page
      .getByRole('button', { name: 'Help & logs', exact: true })
      .click();
    await page
      .getByRole('button', { name: 'Run responsiveness check' })
      .click();
    await page.getByRole('button', { name: 'Downloads', exact: true }).click();
    await expect(page.getByRole('progressbar')).toHaveCount(3);
  });
  expect(await readFile(join(invalid, 'state.db'))).toEqual(bytes);
}
console.log(
  'Native saved-data checks passed: fresh startup, restart, corrupt/newer retention and usable navigation. All databases were temporary.',
);
