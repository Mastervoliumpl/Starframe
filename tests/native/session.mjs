import { chromium, expect } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';

import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { createServer } from 'node:net';

const executable = resolve('src-tauri/target/debug/starframe.exe');
export async function withDesktop(root, check, env = {}) {
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
      ...env,
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
    await check(browser.contexts()[0].pages()[0], child.pid);
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
