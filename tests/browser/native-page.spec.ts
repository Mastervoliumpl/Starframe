import { expect, test } from '@playwright/test';
import { createServer, type AddressInfo } from 'node:net';
import { waitForDebugPortRelease } from '../native/port.mjs';
import { waitForDesktopPage } from '../native/page.mjs';

test('native connection waits for the app document and also accepts an existing page', async ({
  browser,
}) => {
  const context = await browser.newContext();
  try {
    expect(context.pages()).toHaveLength(0);
    const connectedPage = waitForDesktopPage(browser);
    const page = await context.newPage();
    await page.route('http://tauri.localhost/', (route) =>
      route.fulfill({
        contentType: 'text/html',
        body: '<h1>Native document fixture</h1>',
      }),
    );
    await page.goto('http://tauri.localhost/');
    expect(await connectedPage).toBe(page);
    await expect(page.getByRole('heading')).toHaveText(
      'Native document fixture',
    );
    expect(await waitForDesktopPage(browser)).toBe(page);
  } finally {
    await context.close();
  }
});

test('native port check waits for the listener to close and accepts a released endpoint', async () => {
  const server = createServer();
  await new Promise<void>((accept) => server.listen(0, '127.0.0.1', accept));
  const port = (server.address() as AddressInfo).port;
  server.once('connection', (socket) => {
    socket.end();
    server.close();
  });
  await waitForDebugPortRelease(port);
  expect(server.listening).toBe(false);
});
