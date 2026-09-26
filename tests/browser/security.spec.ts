import { expect, test } from '@playwright/test';

for (const state of ['blocked', 'cleared']) {
  test(`${state} registry decisions preserve offline management`, async ({
    page,
  }) => {
    await page.goto(`/?fixture&mods&security=${state}`);
    const enabled = page.getByRole('switch', {
      name: 'Enable Terrain tools fixture 1.0',
    });
    await expect(enabled).toBeChecked();
    await enabled.uncheck();
    await expect(enabled).not.toBeChecked();
    if (state === 'blocked') {
      await expect(enabled).toBeDisabled();
      await expect(
        page
          .getByRole('alert')
          .filter({ hasText: 'retained registry security block' }),
      ).toBeVisible();
    } else {
      await enabled.check();
      await expect(enabled).toBeChecked();
      await expect(
        page.getByRole('button', { name: 'Review affected mods' }),
      ).toHaveCount(0);
    }
    await expect(
      page.getByRole('button', { name: 'Uninstall Terrain tools fixture 1.0' }),
    ).toBeEnabled();
    await page
      .getByRole('button', { name: 'Terrain tools fixture', exact: true })
      .click();
    const details = page.getByRole('complementary', { name: 'Mod details' });
    await expect(
      details.getByRole('heading', { name: 'Terrain tools fixture' }),
    ).toBeFocused();
    if (state === 'blocked') {
      await expect(
        details.getByRole('heading', { name: 'Registry security decision' }),
      ).toBeVisible();
      await expect(
        details.getByText(
          'Confirmed fixture security block. Activation is blocked.',
        ),
      ).toBeVisible();
    } else {
      await expect(
        details.getByRole('heading', { name: 'Registry security decision' }),
      ).toHaveCount(0);
    }
    await expect(
      details.getByText('Tested build: Steam 100 · Unity fixture'),
    ).toBeVisible();
    await details.getByRole('button', { name: 'Back to list' }).click();
    await expect(
      page.getByRole('button', { name: 'Terrain tools fixture', exact: true }),
    ).toBeFocused();
    await page
      .getByRole('button', { name: 'Uninstall Terrain tools fixture 1.0' })
      .click();
    const confirmation = page.getByRole('dialog', {
      name: 'Uninstall Terrain tools fixture?',
    });
    await expect(confirmation).toContainText('Settings are kept');
    await page.keyboard.press('Escape');
    await expect(confirmation).not.toBeVisible();
    await page.setViewportSize({ width: 1024, height: 720 });
    await page.addStyleTag({ content: ':root { font-size: 200%; }' });
    await page.emulateMedia({
      forcedColors: 'active',
      reducedMotion: 'reduce',
    });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(
      page.getByRole('heading', { name: 'Settings', exact: true }),
    ).toBeFocused();
  });
}
