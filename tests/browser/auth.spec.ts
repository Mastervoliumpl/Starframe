import { expect, test } from '@playwright/test';

test('manager sign-in leaves local navigation usable and supports keyboard sign-out', async ({
  page,
}) => {
  await page.goto('/?fixture&auth=confirmed');
  await expect(
    page.getByRole('status').filter({ hasText: 'Desktop connected' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const signIn = page.getByRole('button', { name: 'Sign in with Steam' });
  await signIn.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('A1B2C3D4')).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Cancel sign-in' }),
  ).toBeFocused();
  await page.getByRole('button', { name: 'My mods', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'My mods' })).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByText('Signed in as')).toBeVisible({ timeout: 10000 });
  await expect(page.getByText('Fixture user')).toBeVisible();
  const signOut = page.getByRole('button', { name: 'Sign out' });
  await signOut.focus();
  await page.keyboard.press('Enter');
  await expect(
    page.getByRole('status').filter({ hasText: 'Signed out.' }),
  ).toBeVisible();
  await expect(signIn).toBeVisible();
  await expect(signIn).toBeFocused();
});

test('cancelled sign-in removes the code and keeps local controls usable', async ({
  page,
}) => {
  await page.goto('/?fixture&auth=confirmed');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Sign in with Steam' }).click();
  await expect(page.getByText('A1B2C3D4')).toBeVisible();
  await page.getByRole('button', { name: 'Cancel sign-in' }).click();
  await expect(page.getByText('A1B2C3D4')).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: 'Sign in with Steam' }),
  ).toBeFocused();
  await expect(
    page.getByRole('button', { name: 'Find in Steam' }),
  ).toBeEnabled();
});
