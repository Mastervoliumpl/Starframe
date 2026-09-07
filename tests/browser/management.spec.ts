import { expect, test } from '@playwright/test';

test('selection, warnings, enable and confirmed uninstall remain separate', async ({
  page,
}) => {
  await page.goto('/?fixture&mods');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Find in Steam' }).click();
  await page.getByRole('button', { name: /^Use installation:/ }).click();
  await page.getByRole('button', { name: 'My mods', exact: true }).click();
  const region = page.getByRole('region', { name: 'My mods', exact: true });
  await expect(
    region.getByText('Not tested with this version').first(),
  ).toBeVisible();
  await expect(region.getByText('Known compatibility problem')).toBeVisible();
  await region
    .getByRole('checkbox', {
      name: 'Select Terrain tools fixture 1.0',
      exact: true,
    })
    .check();
  const enabled = region.getByRole('switch', {
    name: 'Enable Terrain tools fixture 1.0',
    exact: true,
  });
  await expect(enabled).not.toBeChecked();
  await region
    .getByRole('button', { name: 'Enable selected', exact: true })
    .click();
  await expect(enabled).toBeChecked();
  await region
    .getByRole('switch', { name: 'Enable Withdrawn fixture 1.0', exact: true })
    .check();
  await expect(
    region.getByRole('switch', {
      name: 'Enable Withdrawn fixture 1.0',
      exact: true,
    }),
  ).toBeChecked();
  await region
    .getByRole('button', { name: 'Terrain tools fixture', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { name: 'Terrain tools fixture', exact: true }),
  ).toBeFocused();
  await page.getByRole('button', { name: 'Back to list' }).click();
  await expect(
    region.getByRole('button', { name: 'Terrain tools fixture', exact: true }),
  ).toBeFocused();
  await region
    .getByRole('button', {
      name: 'Uninstall Terrain tools fixture 1.0',
      exact: true,
    })
    .click();
  const dialog = page.getByRole('dialog', {
    name: 'Uninstall Terrain tools fixture?',
  });
  await expect(dialog).toContainText('Default');
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await expect(
    region.getByRole('button', {
      name: 'Uninstall Terrain tools fixture 1.0',
      exact: true,
    }),
  ).toBeFocused();
  await region
    .getByRole('button', {
      name: 'Uninstall Terrain tools fixture 1.0',
      exact: true,
    })
    .click();
  await dialog.getByRole('button', { name: 'Confirm uninstall' }).click();
  await expect(enabled).toHaveCount(0);
});

test('slow transfers keep search and navigation usable and errors remain retryable', async ({
  page,
}) => {
  await page.goto('/?fixture&mods&large');
  await page.getByRole('button', { name: 'Catalog', exact: true }).click();
  const region = page.getByRole('region', { name: 'Catalog', exact: true });
  const search = region.getByRole('searchbox');
  await search.fill('Failed transfer');
  await region.getByRole('button', { name: 'Install', exact: true }).click();
  await search.focus();
  await expect(region.getByText(/Downloading fixture bytes/)).toBeVisible();
  await expect(search).toBeFocused();
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  const downloads = page.getByRole('region', {
    name: 'Downloads',
    exact: true,
  });
  await expect(downloads.getByText(/HTTP 404/)).toBeVisible();
  await expect(
    downloads.getByRole('button', { name: 'Retry exact release' }),
  ).toBeEnabled();
  await page.getByRole('button', { name: 'Catalog', exact: true }).click();
  await expect(search).toHaveValue('Failed transfer');
  await search.fill('Mod fixture 4');
  await region
    .getByRole('checkbox', { name: 'Select Mod fixture 4 1.0', exact: true })
    .check();
  await region
    .getByRole('checkbox', { name: 'Select Mod fixture 40 1.0', exact: true })
    .check();
  await region.getByRole('button', { name: 'Install selected' }).click();
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await downloads
    .getByRole('button', { name: 'Cancel download' })
    .first()
    .click();
  await expect(
    downloads.getByText('Cancelled. No game files changed.').first(),
  ).toBeVisible();
});

test('large catalog uses bounded pages and details preserve context at enlarged text', async ({
  page,
}) => {
  await page.goto('/?fixture&mods&large');
  await page.getByRole('button', { name: 'Catalog', exact: true }).click();
  const region = page.getByRole('region', { name: 'Catalog', exact: true });
  await expect(region.locator('.mod-list > li')).toHaveCount(100);
  await region.getByRole('button', { name: 'Next', exact: true }).click();
  await expect(region.getByText('Page 2 of 12')).toBeVisible();
  await page.setViewportSize({ width: 1024, height: 720 });
  await page.addStyleTag({ content: ':root { font-size: 200%; }' });
  await page.emulateMedia({ reducedMotion: 'reduce', forcedColors: 'active' });
  await region
    .getByRole('button', { name: 'Mod fixture 100', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { name: 'Mod fixture 100', exact: true }),
  ).toBeFocused();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.getByRole('button', { name: 'Back to list' }).click();
  await expect(region.getByText('Page 2 of 12')).toBeVisible();
  await expect(
    region.getByRole('button', { name: 'Mod fixture 100', exact: true }),
  ).toBeFocused();
});
