import { expect, test } from '@playwright/test';

test('local imports use normal controls and retain keyboard focus without catalog checks', async ({
  page,
}) => {
  await page.goto('/?fixture');
  const region = page.getByRole('region', { name: 'My mods', exact: true });
  const trigger = region.getByRole('button', {
    name: 'Import local mod',
    exact: true,
  });
  await trigger.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', {
    name: 'Import local mod',
    exact: true,
  });
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(trigger).toBeFocused();
  await trigger.click();
  await dialog
    .getByRole('button', { name: 'Choose folder', exact: true })
    .click();
  await expect(
    dialog.getByRole('textbox', { name: 'Source path' }),
  ).toHaveValue('C:\\fixture\\local-build');
  await dialog
    .getByRole('button', { name: 'Import copy', exact: true })
    .click();
  await expect(dialog).not.toBeVisible();
  await expect(trigger).toBeFocused();
  const enabled = region.getByRole('switch', {
    name: 'Enable Local build fixture dev.1',
  });
  await expect(enabled).not.toBeChecked();
  await expect(region.getByText(/Build .* · Following source/)).toBeVisible();
  await enabled.check();
  await expect(enabled).toBeChecked();
  await region
    .getByRole('button', { name: 'Local build fixture', exact: true })
    .click();
  const details = region.getByRole('complementary', { name: 'Mod details' });
  await expect(details).toContainText('C:\\fixture\\local-build');
  await expect(details).toContainText(
    'inactive collections keep their exact builds',
  );
  await expect(
    details.getByRole('heading', { name: 'Compatibility', exact: true }),
  ).toHaveCount(0);
  await expect(
    details.getByRole('button', { name: 'View author/source' }),
  ).toHaveCount(0);
  await expect(
    details.getByRole('heading', { name: 'Required builds' }),
  ).toBeVisible();
  await details.getByRole('button', { name: 'Copy source path' }).click();
  await expect(details.getByRole('status')).toContainText(
    /Source path copied|Could not copy/,
  );
  await details.getByRole('button', { name: 'Open source folder' }).click();
  await details.getByRole('button', { name: 'Back to list' }).click();
  await region
    .getByRole('button', { name: 'Uninstall Local build fixture dev.1' })
    .click();
  const uninstall = page.getByRole('dialog', {
    name: 'Uninstall Local build fixture?',
  });
  await expect(uninstall).toContainText(
    'Your source DLL, source folder and local metadata are kept.',
  );
  await uninstall.getByRole('button', { name: 'Confirm uninstall' }).click();
  await expect(enabled).toHaveCount(0);
});

test('import dialog reflows and keeps errors visible', async ({ page }) => {
  await page.goto('/?fixture');
  await page
    .getByRole('button', { name: 'Import local mod', exact: true })
    .click();
  const dialog = page.getByRole('dialog', {
    name: 'Import local mod',
    exact: true,
  });
  await dialog
    .getByRole('textbox', { name: 'Source path' })
    .fill('C:\\missing');
  await dialog
    .getByRole('button', { name: 'Import copy', exact: true })
    .click();
  await expect(dialog.getByRole('alert')).toContainText('fixture source paths');
  for (const width of [1280, 853, 640]) {
    await page.setViewportSize({ width, height: 800 });
    expect(
      await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth),
    ).toBe(true);
    await expect(
      dialog.getByRole('button', { name: 'Choose DLL' }),
    ).toBeVisible();
  }
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  await dialog.getByRole('button', { name: 'Choose DLL' }).click();
  await expect(
    dialog.getByRole('textbox', { name: 'Source path' }),
  ).toHaveValue('C:\\fixture\\Local.dll');
  await page.screenshot({ path: 'test-results/local-import-dialog.png' });
});
