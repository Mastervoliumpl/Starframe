import { chromium, expect } from '@playwright/test';
import { waitForDesktopPage } from './page.mjs';
import { spawn, execFileSync } from 'node:child_process';

import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { waitForDebugPortRelease } from './port.mjs';

const executable = resolve('src-tauri/target/debug/starframe.exe');
export async function withDesktop(root, check, env = {}) {
  await waitForDebugPortRelease(9224);
  const child = spawn(executable, [], {
    windowsHide: true,
    stdio: ['ignore', 'inherit', 'inherit'],
    env: {
      ...process.env,
      ...env,
      STARFRAME_TEST_DATA_DIR: root,
      STARFRAME_TEST_DEBUG_PORT: '9224',
    },
  });
  let browser;
  try {
    const deadline = Date.now() + 30000;
    while (Date.now() < deadline) {
      try {
        browser = await chromium.connectOverCDP('http://127.0.0.1:9224', {
          timeout: 1000,
        });
        break;
      } catch {
        if (child.exitCode !== null)
          throw new Error('The test desktop exited before connecting.');
        await delay(100);
      }
    }
    if (!browser) throw new Error('The test desktop did not connect.');
    await check(await waitForDesktopPage(browser), child.pid);
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
    await waitForDebugPortRelease(9224);
  }
}
