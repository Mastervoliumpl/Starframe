import { expect, test } from '@playwright/test';

test('dependency browsing suggests a matching exact choice and preserves explicit install and Back focus', async ({
  page,
}) => {
  await page.goto('/?fixture&mods&registry');
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  const browser = page.getByRole('region', { name: 'Mods', exact: true });
  const trigger = browser.getByRole('button', {
    name: 'Details for Mod fixture 1',
    exact: true,
  });
  await trigger.click();
  const details = browser.getByRole('region', {
    name: 'Mod details',
    exact: true,
  });
  await details
    .getByRole('button', {
      name: 'Find releases for dependency 2',
      exact: true,
    })
    .click();
  await expect(
    details.getByRole('heading', { name: 'Mod fixture 2', exact: true }),
  ).toBeFocused();
  await expect(
    details.getByRole('heading', { name: 'Release 1.0.0', exact: true }),
  ).toBeVisible();
  await expect(
    details.getByText(/newest matching available release on this history page/),
  ).toBeVisible();
  await expect(
    details.getByRole('button', { name: '2.0.0', exact: true }),
  ).toHaveCount(0);
  await expect(
    details.getByRole('button', { name: 'Install this release', exact: true }),
  ).toBeEnabled();
  await details
    .getByRole('button', { name: 'Show all releases', exact: true })
    .click();
  await expect(
    details.getByRole('heading', { name: 'Release 2.0.0', exact: true }),
  ).toBeVisible();
  await details
    .getByRole('button', { name: 'Back to mods', exact: true })
    .click();
  await expect(trigger).toBeFocused();
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await expect(page.getByRole('progressbar')).toHaveCount(0);
});

test('sign-out and registry failures preserve installed and local management', async ({
  page,
}) => {
  await page.goto('/?fixture&mods&registry=offline');
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  const browser = page.getByRole('region', { name: 'Mods', exact: true });
  await expect(browser.getByRole('alert')).toContainText(
    'Fixture registry unavailable',
  );
  await page.getByRole('button', { name: 'My mods', exact: true }).click();
  const enabled = page.getByRole('switch', {
    name: 'Enable Terrain tools fixture 1.0',
  });
  await enabled.check();
  await expect(enabled).toBeChecked();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Sign out', exact: true }).click();
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  await expect(
    browser.getByRole('heading', { name: 'Sign in to browse Mods' }),
  ).toBeVisible();
  await expect(
    browser.getByRole('button', { name: 'Details for Mod fixture 1' }),
  ).toHaveCount(0);
  await page.getByRole('button', { name: 'My mods', exact: true }).click();
  await expect(enabled).toBeChecked();
  await enabled.uncheck();
  await expect(enabled).not.toBeChecked();
  await expect(
    page.getByRole('button', { name: 'Import local mod' }),
  ).toBeEnabled();
});

test('native browse uses bounded pages and remembers filters and keyboard return', async ({
  page,
}) => {
  await page.goto('/?fixture&mods&registry&large');
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  const browser = page.getByRole('region', { name: 'Mods', exact: true });
  await expect(browser.locator('.results > li')).toHaveCount(10);
  await browser.getByRole('button', { name: 'Next page', exact: true }).click();
  await expect(
    browser.getByText('Page 2 of 120', { exact: true }),
  ).toBeVisible();
  const entry = browser.getByRole('button', {
    name: 'Details for Mod fixture 11',
    exact: true,
  });
  await entry.click();
  const details = browser.getByRole('region', {
    name: 'Mod details',
    exact: true,
  });
  await expect(
    details.getByRole('heading', { name: 'Mod fixture 11', exact: true }),
  ).toBeFocused();
  await details
    .getByRole('button', { name: 'Back to mods', exact: true })
    .click();
  await expect(entry).toBeFocused();
  await expect(
    browser.getByText('Page 2 of 120', { exact: true }),
  ).toBeVisible();
  await browser
    .getByRole('combobox', { name: 'Layout', exact: true })
    .selectOption('cards');
  await expect(browser.locator('.results > li')).toHaveCount(6);
  await browser.getByRole('button', { name: 'Filters', exact: true }).click();
  const tag = browser.getByRole('button', {
    name: 'Lua: Any. Click to include.',
    exact: true,
  });
  await tag.click();
  await expect(
    browser.getByRole('button', { name: 'Filters (1)', exact: true }),
  ).toBeVisible();
  await browser
    .getByRole('button', {
      name: 'Lua: Include. Click to exclude.',
      exact: true,
    })
    .click();
  await expect(
    browser.getByRole('button', {
      name: 'Lua: Exclude. Click to clear.',
      exact: true,
    }),
  ).toBeVisible();
  await browser
    .getByRole('combobox', { name: 'Mod type', exact: true })
    .selectOption('map');
  await expect(browser.locator('.results > li')).toHaveCount(6);
  await browser.getByRole('searchbox').fill('Author 2');
  await browser.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(
    browser.getByRole('button', {
      name: 'Details for Mod fixture 2',
      exact: true,
    }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  await expect(browser.getByRole('searchbox')).toHaveValue('Author 2');
  await expect(
    browser.getByRole('combobox', { name: 'Layout', exact: true }),
  ).toHaveValue('cards');
});

test('details render safe text, exact historical releases, security and accessible screenshots', async ({
  page,
}) => {
  await page.route('https://api.starframemanager.com/v1/media/**', (route) =>
    route.fulfill({
      contentType: 'image/svg+xml',
      body: '<svg xmlns="http://www.w3.org/2000/svg" width="400" height="240"><rect width="400" height="240" fill="#334155"/><text x="30" y="130" font-size="26" fill="#f8fafc">Fixture screenshot</text></svg>',
    }),
  );
  await page.goto('/?fixture&registry');
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  const browser = page.getByRole('region', { name: 'Mods', exact: true });
  await browser
    .getByRole('button', { name: 'Details for Mod fixture 1', exact: true })
    .click();
  const details = browser.getByRole('region', {
    name: 'Mod details',
    exact: true,
  });
  await expect(
    details.getByText(
      'Fixture detail. <script>Descriptions stay plain text.</script>',
      { exact: true },
    ),
  ).toBeVisible();
  await details
    .getByRole('button', { name: 'Preview screenshot 1', exact: true })
    .click();
  const gallery = page.getByRole('dialog', { name: 'Screenshot preview' });
  await expect(gallery).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(gallery).not.toBeVisible();
  await expect(
    details.getByRole('button', { name: 'Preview screenshot 1', exact: true }),
  ).toBeFocused();
  await details.getByRole('button', { name: '1.0.0', exact: true }).click();
  await expect(
    details.getByRole('heading', { name: 'Release 1.0.0', exact: true }),
  ).toBeVisible();
  await details
    .getByRole('button', { name: 'Install this release', exact: true })
    .click();
  await expect(details.getByRole('alert')).toContainText(
    'Fixture install rejected',
  );
  await details
    .getByRole('button', { name: 'Back to mods', exact: true })
    .click();
  await browser
    .getByRole('button', { name: 'Details for Mod fixture 3', exact: true })
    .click();
  await expect(
    details.getByText('Confirmed fixture security block.', { exact: true }),
  ).toBeVisible();
  await expect(
    details.getByRole('button', { name: 'Install this release', exact: true }),
  ).toBeDisabled();
});

test('pending discovery and installs keep navigation responsive and discard stale results', async ({
  page,
}) => {
  await page.goto('/?fixture&registry=slow');
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  const browser = page.getByRole('region', { name: 'Mods', exact: true });
  const search = browser.getByRole('searchbox');
  await search.fill('slow');
  await browser.getByRole('button', { name: 'Search', exact: true }).click();
  await search.fill('Mod fixture 1');
  await browser.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(
    browser.getByRole('button', {
      name: 'Details for Mod fixture 1',
      exact: true,
    }),
  ).toBeVisible();
  await page.waitForTimeout(1000);
  await expect(
    browser.getByRole('button', {
      name: 'Details for Mod fixture 1',
      exact: true,
    }),
  ).toBeVisible();
  await browser
    .getByRole('button', { name: 'Details for Mod fixture 1', exact: true })
    .click();
  await browser
    .getByRole('button', { name: 'Install this release', exact: true })
    .click();
  await expect(
    browser.getByRole('button', { name: 'Preparing install…', exact: true }),
  ).toBeDisabled();
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Collections', exact: true }),
  ).toBeFocused();
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  await expect(search).toHaveValue('Mod fixture 1');
  await expect(browser.getByRole('alert')).toContainText(
    'Fixture install rejected',
  );
});

test('detail layout fits the approved sizes and enlarged text without losing context', async ({
  page,
}) => {
  await page.goto('/?fixture&registry&large');
  await page.getByRole('button', { name: 'Mods', exact: true }).click();
  const browser = page.getByRole('region', { name: 'Mods', exact: true });
  for (const width of [1280, 1600, 1024]) {
    await page.setViewportSize({
      width,
      height: width === 1600 ? 1000 : width === 1024 ? 720 : 800,
    });
    await browser
      .getByRole('button', { name: 'Details for Mod fixture 1', exact: true })
      .click();
    const heading = browser.getByRole('heading', {
      name: 'Mod fixture 1',
      exact: true,
    });
    await expect(heading).toBeFocused();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.screenshot({
      path: `test-results/0.7.0-trust/mods-ui-${width}.png`,
    });
    await browser
      .getByRole('button', { name: 'Back to mods', exact: true })
      .click();
  }
  await page.addStyleTag({ content: ':root { font-size: 200%; }' });
  await page.emulateMedia({ reducedMotion: 'reduce', forcedColors: 'active' });
  await browser
    .getByRole('button', { name: 'Details for Mod fixture 1', exact: true })
    .click();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await browser
    .getByRole('button', { name: 'Back to mods', exact: true })
    .click();
  await expect(
    browser.getByRole('button', {
      name: 'Details for Mod fixture 1',
      exact: true,
    }),
  ).toBeFocused();
});
