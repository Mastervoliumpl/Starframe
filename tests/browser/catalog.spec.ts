import { expect, test } from '@playwright/test';

for (const state of ['confirmed', 'suspected', 'cleared']) {
  test(`${state} findings preserve management and evidence history`, async ({
    page,
  }) => {
    await page.goto(`/?fixture&mods&security=${state}&catalog=expired`);
    const enabled = page.getByRole('switch', {
      name: 'Enable Terrain tools fixture 1.0',
    });
    await expect(enabled).toBeChecked();
    await enabled.uncheck();
    await expect(enabled).not.toBeChecked();
    if (state === 'confirmed') {
      await expect(enabled).toBeDisabled();
      await expect(
        page.getByRole('alert').filter({ hasText: 'installed mod matches' }),
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
    await expect(
      details.getByText(
        state === 'confirmed'
          ? 'Confirmed · downloads and activation blocked'
          : state === 'suspected'
            ? 'Unconfirmed · use is permitted'
            : 'Cleared · this finding no longer blocks use',
      ),
    ).toBeVisible();
    const history = details.getByText('Evidence and correction history', {
      exact: true,
    });
    await history.focus();
    await page.keyboard.press('Enter');
    await expect(
      details.getByText('https://example.invalid/security-evidence', {
        exact: true,
      }),
    ).toBeVisible();
    if (state === 'cleared')
      await expect(
        details.getByText('https://example.invalid/correction', {
          exact: true,
        }),
      ).toBeVisible();
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
