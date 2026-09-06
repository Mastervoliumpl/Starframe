import { chromium, expect } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { mkdir, writeFile, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { resolve } from 'node:path';
import { createServer } from 'node:net';
import { setTimeout as delay } from 'node:timers/promises';

const executable = resolve('src-tauri/target/debug/starframe.exe');
const output = resolve('test-results/native');
const dataDirectory = await mkdtemp(join(tmpdir(), 'starframe-native-'));
await mkdir(output, { recursive: true });
const probe = createServer();
await new Promise((accept, reject) => {
  probe.once('error', reject);
  probe.listen(9223, '127.0.0.1', accept);
});
await new Promise((accept) => probe.close(accept));
const child = spawn(executable, [], {
  windowsHide: true,
  stdio: ['ignore', 'inherit', 'inherit'],
  env: {
    ...process.env,
    STARFRAME_TEST_DATA_DIR: dataDirectory,
    STARFRAME_TEST_DEBUG_PORT: '9223',
  },
});
let browser;
let second;
try {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9223', {
        timeout: 1000,
      });
      break;
    } catch {
      if (child.exitCode !== null)
        throw new Error('The native app exited before connecting.');
      await delay(100);
    }
  }
  if (!browser)
    throw new Error('The native webview did not expose its test connection.');
  const page = browser.contexts()[0].pages()[0];
  await expect(
    page.getByRole('status').filter({ hasText: 'Desktop connected' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(
    page.getByText('Saved locally: 0 library entries and 0 collections.'),
  ).toBeVisible();
  await page.getByRole('button', { name: 'My mods', exact: true }).click();
  const rejected = await page.evaluate(async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const invalidPage = await invoke('open_external', {
      page: 'file:///C:/invalid',
    }).catch((error) => error);
    const rawOpener = await invoke('plugin:opener|open_url', {
      url: 'javascript:alert(1)',
    }).catch((error) => String(error));
    return { invalidPage, rawOpener };
  });
  expect(rejected.invalidPage.code).toBe('invalid_link');
  expect(rejected.rawOpener).toContain('not allowed');
  await page.screenshot({ path: resolve(output, 'my-mods.png') });
  second = spawn(executable, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      STARFRAME_TEST_DATA_DIR: dataDirectory,
    },
  });
  await expect.poll(() => second.exitCode).toBe(0);
  if (child.exitCode !== null)
    throw new Error(
      'The original desktop exited when the second instance started.',
    );
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByLabel('Show fixture rows').check();
  await page.getByLabel('Check notes (not saved)').fill('Native check draft');
  await page
    .getByRole('checkbox', {
      name: 'Fixture mod 0001 Diagnostic fixture',
      exact: true,
    })
    .check();
  const list = page.getByRole('region', { name: 'Fixture rows' });
  await list.evaluate((el) => (el.scrollTop = 400));
  await page.getByRole('button', { name: 'Run responsiveness check' }).click();
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await expect(page.getByRole('progressbar')).toHaveCount(3);
  await expect
    .poll(() => page.getByRole('progressbar').first().getAttribute('value'))
    .not.toBe('0');
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByText('Connection details', { exact: true }).click();
  await page
    .getByRole('button', { name: 'Reconnect state subscription' })
    .click();
  await expect(page.getByLabel('Check notes (not saved)')).toHaveValue(
    'Native check draft',
  );
  expect(await list.evaluate((el) => el.scrollTop)).toBe(400);
  await expect(page.getByText('1000 rows · 1 selected')).toBeVisible();
  const samples = await page
    .getByLabel('Search fixture rows')
    .evaluate(async (element) => {
      const times = [];
      for (let index = 0; index < 100; index++) {
        const start = performance.now();
        element.value = index % 2 ? '' : String(index).padStart(2, '0');
        element.dispatchEvent(new Event('input', { bubbles: true }));
        await new Promise((done) =>
          requestAnimationFrame(() => requestAnimationFrame(done)),
        );
        times.push(performance.now() - start);
      }
      return times;
    });
  const p95 = [...samples].sort((a, b) => a - b)[94];
  const report = {
    browser: browser.version(),
    environment: await page.evaluate(() => ({
      userAgent: navigator.userAgent,
      devicePixelRatio,
      width: innerWidth,
      height: innerHeight,
    })),
    p95,
    samples,
    duplicateInstance: 'second exited; first stayed open',
    reconnect: 'draft, selection and scroll retained',
    cancellation: 'native worker confirmed cancelled',
  };
  await writeFile(
    resolve(output, 'report.json'),
    JSON.stringify(report, null, 2),
  );
  expect(p95).toBeLessThan(100);
  await page.screenshot({ path: resolve(output, 'diagnostics.png') });
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await page
    .getByRole('button', { name: 'Cancel', exact: true })
    .first()
    .click();
  await expect(page.getByText('cancelled', { exact: true })).toBeVisible();
  await page.screenshot({ path: resolve(output, 'downloads.png') });
  execFileSync(
    'powershell.exe',
    [
      '-NoProfile',
      '-Command',
      `(Get-Process -Id ${child.pid}).CloseMainWindow()`,
    ],
    { windowsHide: true },
  );
  await expect.poll(() => child.exitCode, { timeout: 5000 }).toBe(0);
  console.log(
    JSON.stringify(
      {
        ...report,
        samples: `${samples.length} samples`,
        close: 'native window closed while work was active; process exited',
      },
      null,
      2,
    ),
  );
} finally {
  await browser?.close();
  if (second?.exitCode === null) second.kill();
  if (child.exitCode === null) child.kill();
}
