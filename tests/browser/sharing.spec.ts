import { expect, test } from '@playwright/test';

const reference = (index: number) => ({
  modId: `fixture.mod${index}`,
  hash: index.toString(16).padStart(64, '0'),
  origin: 'catalog',
  releaseId: `fixture.mod${index}.1`,
});
const document = JSON.stringify({
  format: 'starframe-collection',
  schemaVersion: 1,
  name: 'Shared setup',
  entries: [reference(1), reference(0), reference(3), reference(2)],
});

test('one acceptance creates a collection, verifies cached entries, and keeps failed and withdrawn references', async ({
  page,
}) => {
  await page.goto('/?fixture&mods');
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  const open = page.getByRole('button', {
    name: 'Import collection',
    exact: true,
  });
  await open.click();
  const dialog = page.getByRole('dialog');
  const input = dialog.getByRole('textbox', {
    name: 'Or paste collection JSON',
  });
  await expect(input).toBeFocused();
  await input.fill(document);
  await page.waitForTimeout(1200);
  await expect(input).toHaveValue(document);
  await dialog.getByRole('button', { name: 'Review import' }).click();
  await expect(
    dialog.getByText('Already downloaded; verify local files before reuse.'),
  ).toHaveCount(2);
  await expect(
    dialog.getByText(
      'Exact release unavailable or withdrawn. No substitute was selected.',
    ),
  ).toBeVisible();
  await page.screenshot({ path: 'test-results/0.4.0/sharing-review.png' });
  await dialog.getByRole('button', { name: 'Accept import' }).click();
  await expect(dialog).not.toBeVisible();
  await expect(open).toBeFocused();
  const collection = page
    .getByRole('list', { name: 'Saved collections' })
    .getByRole('listitem')
    .filter({
      has: page.getByRole('heading', { name: 'Shared setup', exact: true }),
    });
  await expect(
    collection.getByText('2 of 4 exact packages ready'),
  ).toBeVisible();
  await collection.getByText('Import details', { exact: true }).click();
  await expect(
    collection.getByText(/Download unavailable: HTTP 404/),
  ).toBeVisible();
  await expect(
    collection.getByRole('button', { name: 'Retry import' }),
  ).toBeVisible();
  await page
    .getByRole('button', { name: 'Export collection Shared setup' })
    .click();
  const exported = await dialog
    .getByRole('textbox', { name: 'Collection JSON', exact: true })
    .inputValue();
  expect(JSON.parse(exported)).toEqual(JSON.parse(document));
  const download = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Save collection file' }).click();
  expect((await download).suggestedFilename()).toBe(
    'collection.starframe-collection.json',
  );
});

test('import rejects an unsupported format and untrusted links without creating a collection', async ({
  page,
}) => {
  await page.goto('/?fixture&mods');
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  await page
    .getByRole('button', { name: 'Import collection', exact: true })
    .click();
  const dialog = page.getByRole('dialog');
  const input = dialog.getByRole('textbox', {
    name: 'Or paste collection JSON',
  });
  const unsupported = document.replace(
    '"schemaVersion":1',
    '"schemaVersion":99',
  );
  await dialog.getByLabel('Choose a collection file').setInputFiles({
    name: 'unsupported.starframe-collection.json',
    mimeType: 'application/json',
    buffer: Buffer.from(unsupported),
  });
  await expect(input).toHaveValue(unsupported);
  await dialog.getByRole('button', { name: 'Review import' }).click();
  await expect(dialog.getByRole('alert')).toContainText(
    'Unsupported collection format',
  );
  const untrusted = JSON.parse(document);
  untrusted.entries[0].url = 'https://unapproved.invalid/package.zip';
  await input.fill(JSON.stringify(untrusted));
  await dialog.getByRole('button', { name: 'Review import' }).click();
  await expect(dialog.getByRole('alert')).toContainText('unknown field');
  await page.keyboard.press('Escape');
  await expect(
    page.getByRole('heading', { name: 'Shared setup', exact: true }),
  ).toHaveCount(0);
});

test('import review remains usable with enlarged text and forced colors', async ({
  page,
}) => {
  await page.goto('/?fixture&mods');
  await page.setViewportSize({ width: 1024, height: 720 });
  await page.addStyleTag({ content: ':root {font-size:200%;}' });
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  await page.getByRole('button', { name: 'Collections', exact: true }).click();
  await page
    .getByRole('button', { name: 'Import collection', exact: true })
    .click();
  const dialog = page.getByRole('dialog');
  await dialog
    .getByRole('textbox', { name: 'Or paste collection JSON' })
    .fill(document);
  await dialog.getByRole('button', { name: 'Review import' }).click();
  expect(
    await dialog.evaluate(
      (element) => element.scrollWidth <= element.clientWidth,
    ),
  ).toBe(true);
  await dialog
    .getByRole('button', { name: 'Accept import' })
    .scrollIntoViewIfNeeded();
  await page.screenshot({
    path: 'test-results/0.4.0/sharing-review-200-percent.png',
  });
  await page.keyboard.press('Escape');
  await expect(
    page.getByRole('button', { name: 'Import collection', exact: true }),
  ).toBeFocused();
});
