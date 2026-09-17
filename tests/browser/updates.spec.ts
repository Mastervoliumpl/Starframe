import { expect, test } from '@playwright/test';

test('updates remain discoverable after Later, preserve navigation and render plain release notes', async ({
  page,
}) => {
  await page.goto('/?fixture&update');
  const notice = page.getByRole('button', { name: 'Update available · 0.6.1' });
  await notice.click();
  await page.getByText('Release notes', { exact: true }).click();
  await expect(
    page.getByText('<script>Release notes remain plain text.</script>', {
      exact: true,
    }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Later', exact: true }).click();
  await expect(notice).toHaveCount(0);
  await expect(
    page.getByRole('heading', { name: 'Update available: 0.6.1' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Update', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Cancel update', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'My mods', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'My mods', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page
    .getByRole('button', { name: 'Cancel update', exact: true })
    .click();
  await page
    .getByRole('button', { name: 'Check for updates', exact: true })
    .click();
  await expect(
    page.getByText('Fixture offline. Installed mods remain usable.'),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Update available: 0.6.1' }),
  ).toBeVisible();
  const channel = page.getByLabel('Update channel', { exact: true });
  await channel.focus();
  await page.keyboard.press('Home');
  await expect(channel).toHaveValue('stable');
  await expect(
    page.getByRole('heading', { name: 'Update available: 0.6.1' }),
  ).toHaveCount(0);
});
