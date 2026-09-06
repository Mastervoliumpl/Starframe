/** @param {import('@playwright/test').Browser} browser */
export async function waitForDesktopPage(browser) {
  const context = browser.contexts()[0];
  return context.pages()[0] ?? context.waitForEvent('page', { timeout: 30000 });
}
