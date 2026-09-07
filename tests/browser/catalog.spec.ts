import { expect, test } from '@playwright/test';

test('catalog updates preserve keyboard focus and app version', async ({
  page,
}) => {
  await page.goto('/?fixture&catalog=update');
  await page.getByRole('button', { name: 'Catalog', exact: true }).click();
  const catalog = page.getByRole('region', { name: 'Catalog', exact: true });
  await expect(catalog.getByRole('status')).toContainText('Checking');
  const source = catalog.getByRole('button', {
    name: 'View Starframe on GitHub',
  });
  await source.focus();
  const version = await page.locator('.build-version').innerText();
  await expect(catalog.getByRole('status')).toContainText('Catalog revision 2');
  await expect(source).toBeFocused();
  await expect(page.locator('.build-version')).toHaveText(version, {
    useInnerText: true,
  });
  await page.getByRole('button', { name: 'My mods', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(
    page.getByRole('heading', { name: 'My mods', exact: true }),
  ).toBeFocused();
});

test('offline catalog retains revision and check time at enlarged text sizes', async ({
  page,
}) => {
  await page.goto('/?fixture&catalog=offline');
  await page.getByRole('button', { name: 'Catalog', exact: true }).click();
  const catalog = page.getByRole('region', { name: 'Catalog', exact: true });
  await expect(catalog.getByRole('alert')).toContainText(
    'Fixture connection failed',
  );
  await expect(
    catalog.getByText('Cached revision 1 remains available.'),
  ).toBeVisible();
  await expect(catalog.getByText(/Last successful check:/)).toBeVisible();
  await page.setViewportSize({ width: 1024, height: 720 });
  await page.addStyleTag({ content: ':root { font-size: 200%; }' });
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  await expect(
    catalog.getByRole('heading', { name: 'Approved release catalog' }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
