import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.goto('/?fixture');
  await expect(
    page.getByRole('status').filter({ hasText: 'Desktop connected' }),
  ).toBeVisible();
});

test('game selection remains readable and keyboard accessible at large text sizes', async ({
  page,
}) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const find = page.getByRole('button', { name: 'Find in Steam' });
  await find.focus();
  await page.keyboard.press('Enter');
  await expect(find).toBeDisabled();
  await expect(
    page.getByRole('button', { name: 'My mods', exact: true }),
  ).toBeEnabled();
  const select = page.getByRole('button', { name: /^Use installation:/ });
  await expect(select).toBeEnabled();
  await select.focus();
  await page.keyboard.press('Enter');
  await expect(
    page.getByText('Game not running', { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Selected installation' }),
  ).toBeFocused();
  await page.setViewportSize({ width: 1024, height: 720 });
  await page.addStyleTag({ content: 'html { font-size: 175%; }' });
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  const folder = page.getByText('C:\\Fixture library\\Sanctuary', {
    exact: true,
  });
  await expect(folder).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.getByRole('button', { name: 'Choose game folder' }).click();
  await expect(page.getByRole('alert')).toContainText(
    'requires the desktop app',
  );
  await expect(folder).toBeVisible();
});

test('navigation and reconnect preserve local search, selection, scroll and notes under load', async ({
  page,
}) => {
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByLabel('Show fixture rows').check();
  await page
    .getByRole('checkbox', {
      name: 'Fixture mod 0001 Diagnostic fixture',
      exact: true,
    })
    .check();
  await page
    .getByLabel('Check notes (not saved)')
    .fill('Keep this unfinished note.');
  await page.getByLabel('Search fixture rows').fill('00');
  const rows = page.getByRole('region', { name: 'Fixture rows' });
  await rows.evaluate((el) => (el.scrollTop = 500));
  await page.getByRole('button', { name: 'Run responsiveness check' }).click();
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await expect(page.getByRole('progressbar')).toHaveCount(3);
  await page
    .getByRole('button', { name: 'Cancel', exact: true })
    .first()
    .click();
  await expect(page.getByText('cancelled', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByText('Connection details', { exact: true }).click();
  await page
    .getByRole('button', { name: 'Reconnect state subscription' })
    .click();
  await expect(page.getByLabel('Search fixture rows')).toHaveValue('00');
  await expect(page.getByLabel('Check notes (not saved)')).toHaveValue(
    'Keep this unfinished note.',
  );
  await expect(page.getByText(/rows · 1 selected/)).toBeVisible();
  expect(await rows.evaluate((el) => el.scrollTop)).toBe(500);
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await expect(page.getByText('cancelled', { exact: true })).toBeVisible();
});

test('failure remains visible and a subsequent check can complete', async ({
  page,
}) => {
  test.setTimeout(30000);
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByLabel('Simulate a failure').check();
  await page.getByRole('button', { name: 'Run responsiveness check' }).click();
  await page
    .getByRole('button', { name: 'View progress', exact: true })
    .click();
  await expect(page.getByText('failed', { exact: true })).toHaveCount(3, {
    timeout: 10000,
  });
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByLabel('Simulate a failure').uncheck();
  await page.getByRole('button', { name: 'Run responsiveness check' }).click();
  await page
    .getByRole('button', { name: 'View progress', exact: true })
    .click();
  await expect(page.getByText('completed', { exact: true })).toHaveCount(3, {
    timeout: 15000,
  });
});

test('narrow navigation traps focus and Escape restores it; layouts keep the full launch label', async ({
  page,
}) => {
  for (const width of [1600, 1280, 1024, 640]) {
    await page.setViewportSize({ width, height: 800 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await expect(
      page.getByRole('button', { name: 'Launch Sanctuary Shattered Sun' }),
    ).toBeVisible();
  }
  const menu = page.getByRole('button', { name: 'Menu', exact: true });
  await menu.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'Navigation' });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole('button', { name: 'Close menu' }),
  ).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(
    dialog.getByRole('button', { name: 'Help & logs', exact: true }),
  ).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(menu).toBeFocused();
  await menu.click();
  await dialog.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Settings', exact: true }),
  ).toBeFocused();
  await page.emulateMedia({ reducedMotion: 'reduce', forcedColors: 'active' });
  expect(
    await menu.evaluate((el) => getComputedStyle(el).transitionDuration),
  ).toBe('0s');
  await page.addStyleTag({
    content: ':root { font-size: 28px; line-height: 40px; }',
  });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});

test('100 fixture searches paint within the responsiveness budget', async ({
  page,
}, testInfo) => {
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByLabel('Show fixture rows').check();
  await page.getByRole('button', { name: 'Run responsiveness check' }).click();
  const samples = await page
    .getByLabel('Search fixture rows')
    .evaluate(async (element) => {
      const input = element as HTMLInputElement;
      const times: number[] = [];
      for (let index = 0; index < 100; index++) {
        const start = performance.now();
        input.value = index % 2 ? '' : String(index).padStart(2, '0');
        input.dispatchEvent(new Event('input', { bubbles: true }));
        await new Promise<void>((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        );
        times.push(performance.now() - start);
      }
      return times;
    });
  const p95 = [...samples].sort((a, b) => a - b)[94];
  await testInfo.attach('search-timing.json', {
    body: JSON.stringify({ p95, samples }),
    contentType: 'application/json',
  });
  expect(p95).toBeLessThan(100);
  await page.screenshot({ path: testInfo.outputPath('diagnostics.png') });
});
