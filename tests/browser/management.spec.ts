import { expect, test } from '@playwright/test';

test('registry archive status does not claim installation or offer catalog retry', async ({
  page,
}) => {
  await page.goto('/?fixture&registryArchive');
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  const downloads = page.getByRole('region', {
    name: 'Downloads',
    exact: true,
  });
  const saved = downloads
    .getByRole('listitem')
    .filter({ hasText: '11111111-1111-4111-8111-111111111111' });
  const failed = downloads
    .getByRole('listitem')
    .filter({ hasText: '22222222-2222-4222-8222-222222222222' });
  await expect(saved.getByText('Archive saved', { exact: true })).toBeVisible();
  await expect(saved.getByText('Installed in library')).toHaveCount(0);
  await expect(
    failed.getByRole('button', { name: 'Retry exact release' }),
  ).toHaveCount(0);
  await expect(failed.getByText(/Retry this release/)).toBeVisible();
  const verification = downloads
    .getByRole('listitem')
    .filter({ hasText: '44444444-4444-4444-8444-444444444444' });
  await expect(
    verification.getByRole('button', { name: 'Retry exact release' }),
  ).toHaveCount(0);
  await expect(
    verification.getByText(/Retry import in\s+Collections/),
  ).toBeVisible();
  await expect(
    downloads.getByRole('button', { name: 'Retry pending receipts' }),
  ).toBeEnabled();
});

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
  await expect(region.getByText('Not tested with this version')).toHaveCount(3);
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
