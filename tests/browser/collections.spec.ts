import { expect, test } from '@playwright/test';

test('collections preserve membership and form edits, support keyboard selection, and delete without uninstall', async ({
  page,
}) => {
  await page.goto('/?fixture&mods');
  const myMods = page.getByRole('region', { name: 'My mods', exact: true });
  const terrain = myMods.getByRole('switch', {
    name: 'Enable Terrain tools fixture 1.0',
    exact: true,
  });
  await terrain.check();
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  const newCollection = page.getByRole('button', {
    name: 'New collection',
    exact: true,
  });
  await newCollection.click();
  const dialog = page.getByRole('dialog');
  const name = dialog.getByRole('textbox', { name: 'Collection name' });
  await expect(name).toBeFocused();
  await name.fill('Co-op');
  await page.waitForTimeout(1200);
  await expect(name).toHaveValue('Co-op');
  await dialog
    .getByRole('button', { name: 'Create collection', exact: true })
    .click();
  await expect(dialog).not.toBeVisible();
  await expect(newCollection).toBeFocused();
  await page
    .getByRole('button', { name: 'Use collection Co-op', exact: true })
    .click();
  await expect(
    page.getByRole('button', { name: 'Use collection Co-op', exact: true }),
  ).toBeFocused();
  await expect(
    page.getByRole('heading', { name: 'No enabled mods' }),
  ).toBeVisible();
  await page
    .getByRole('button', { name: 'Add or remove mods in My mods' })
    .click();
  await expect(terrain).not.toBeChecked();
  const core = myMods.getByRole('switch', {
    name: 'Enable Core library fixture 1.0',
    exact: true,
  });
  await core.check();
  const selector = myMods.getByRole('combobox', { name: 'Active collection' });
  await selector.selectOption({ label: 'Default' });
  await expect(terrain).toBeChecked();
  await expect(core).not.toBeChecked();
  await selector.focus();
  await page.keyboard.press('Home');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(selector).toHaveValue(
    (await selector
      .locator('option')
      .filter({ hasText: /^Co-op$/ })
      .getAttribute('value')) ?? '',
  );
  await expect(core).toBeChecked();
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  await page
    .getByRole('button', { name: 'Rename collection Co-op', exact: true })
    .click();
  await name.fill('  ');
  await dialog.getByRole('button', { name: 'Save name', exact: true }).click();
  await expect(name).toHaveAttribute('aria-invalid', 'true');
  await name.fill('Skirmish');
  await dialog.getByRole('button', { name: 'Save name', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Skirmish', exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: 'test-results/browser/collections.png',
    fullPage: true,
  });
  await page
    .getByRole('button', { name: 'Delete collection Default', exact: true })
    .click();
  await page.keyboard.press('Escape');
  await expect(
    page.getByRole('button', {
      name: 'Delete collection Default',
      exact: true,
    }),
  ).toBeFocused();
  await page
    .getByRole('button', { name: 'Delete collection Skirmish', exact: true })
    .click();
  await expect(dialog).toContainText('active collection');
  await dialog
    .getByRole('button', { name: 'Delete collection', exact: true })
    .click();
  await expect(newCollection).toBeFocused();
  await page.getByRole('button', { name: 'My mods', exact: true }).click();
  await expect(myMods.getByRole('switch')).toHaveCount(3);
  await expect(core).not.toBeChecked();
  await expect(terrain).not.toBeChecked();
  await selector.selectOption({ label: 'Default' });
  await expect(terrain).toBeChecked();
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  await page.setViewportSize({ width: 1024, height: 720 });
  await page.addStyleTag({ content: ':root {font-size:200%;}' });
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: 'test-results/browser/collections-200-percent.png',
    fullPage: true,
  });
});
