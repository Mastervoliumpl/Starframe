/** @param {import('@playwright/test').Browser} browser */
export async function waitForDesktopPage(browser) {
  const context = browser.contexts()[0];
  const page =
    context.pages()[0] ??
    (await context.waitForEvent('page', { timeout: 30000 }));
  await page.waitForURL(/^http:\/\/tauri\.localhost\//, {
    waitUntil: 'domcontentloaded',
    timeout: 30000,
  });
  return page;
}
