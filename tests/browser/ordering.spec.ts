import { expect, test } from '@playwright/test';

test('load priority supports keyboard and dragging without losing focus or membership', async ({
  page,
}) => {
  await page.goto('/?fixture&mods');
  const mods = page.getByRole('region', { name: 'My mods', exact: true });
  for (const name of ['Terrain tools fixture', 'Core library fixture']) {
    await mods
      .getByRole('switch', { name: `Enable ${name} 1.0`, exact: true })
      .check();
  }
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  const list = page.getByRole('list', { name: 'Effective load order' });
  await expect(list.getByRole('listitem').first()).toContainText(
    'Terrain tools fixture',
  );
  await page.screenshot({
    path: 'test-results/browser/load-order.png',
    fullPage: true,
  });
  const down = list.getByRole('button', {
    name: 'Move Terrain tools fixture down',
    exact: true,
  });
  await down.focus();
  await page.keyboard.press('Enter');
  await expect(list.getByRole('listitem').first()).toContainText(
    'Core library fixture',
  );
  await expect(down).toBeFocused();
  const firstHandle = list.getByRole('button', {
    name: 'Move Terrain tools fixture up or drag to reorder',
  });
  const target = list.getByRole('button', {
    name: 'Move Core library fixture up or drag to reorder',
  });
  await firstHandle.dragTo(target);
  await expect(list.getByRole('listitem').first()).toContainText(
    'Terrain tools fixture',
  );
  await page.setViewportSize({ width: 1024, height: 720 });
  await page.addStyleTag({ content: ':root { font-size: 200%; }' });
  await page.emulateMedia({ reducedMotion: 'reduce', forcedColors: 'active' });
  await expect(list).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: 'test-results/browser/load-order-200-percent.png',
    fullPage: true,
  });
});
