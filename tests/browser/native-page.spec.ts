import { expect, test } from '@playwright/test';
import { createServer, type AddressInfo } from 'node:net';
import { waitForDebugPortRelease } from '../native/port.mjs';
import { waitForDesktopPage } from '../native/page.mjs';

test('native connection waits for a page and also accepts an existing page', async ({
  browser,
}) => {
  const context = await browser.newContext();
  try {
    expect(context.pages()).toHaveLength(0);
    const connectedPage = waitForDesktopPage(browser);
    const page = await context.newPage();
    expect(await connectedPage).toBe(page);
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
