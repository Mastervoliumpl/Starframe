import { expect } from '@playwright/test';
import { mkdir, mkdtemp, readFile, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { withDesktop } from './session.mjs';

const root = await mkdtemp(join(tmpdir(), 'starframe-storage-'));
for (let run = 0; run < 2; run++) {
  await withDesktop(root, async (page) => {
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(
      page.getByText('Saved locally: 0 library entries and 0 collections.'),
    ).toBeVisible();
  });
}
const original = await readFile(join(root, 'sqlite', 'state.db'));
for (const mode of ['corrupt', 'newer']) {
  const invalid = await mkdtemp(join(tmpdir(), 'starframe-storage-'));
  const bytes =
    mode === 'corrupt' ? Buffer.alloc(8192, 0x5a) : Buffer.from(original);
  if (mode === 'newer') bytes.writeUInt32BE(99, 60);
  await mkdir(join(invalid, 'sqlite'));
  await writeFile(join(invalid, 'sqlite', 'complete'), '');
  await writeFile(join(invalid, 'sqlite', 'state.db'), bytes);
  await withDesktop(invalid, async (page) => {
    await expect(page.getByRole('alert')).toContainText(
      mode === 'corrupt' ? 'header is invalid' : 'newer Starframe version',
    );
    await page
      .getByRole('button', { name: 'Help & logs', exact: true })
      .click();
    await page
      .getByRole('button', { name: 'Run responsiveness check' })
      .click();
    await page.getByRole('button', { name: 'Downloads', exact: true }).click();
    await expect(page.getByRole('progressbar')).toHaveCount(3);
  });
  expect(await readFile(join(invalid, 'sqlite', 'state.db'))).toEqual(bytes);
}
console.log(
  'Native saved-data checks passed: fresh startup, restart, corrupt/newer retention and usable navigation. All databases were temporary.',
);

const obsolete = await mkdtemp(join(tmpdir(), 'starframe-obsolete-storage-'));
await writeFile(join(obsolete, 'state.db'), 'obsolete database');
await writeFile(join(obsolete, 'state.db-wal'), 'retained wal');
await withDesktop(obsolete, async (page) => {
  await expect(page.getByRole('alert')).toContainText(
    'Obsolete saved-data formats are unsupported',
  );
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByRole('button', { name: 'Run responsiveness check' }).click();
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await expect(page.getByRole('progressbar')).toHaveCount(3);
});
expect(await readFile(join(obsolete, 'state.db'), 'utf8')).toBe(
  'obsolete database',
);
expect(await readFile(join(obsolete, 'state.db-wal'), 'utf8')).toBe(
  'retained wal',
);

const delayed = await mkdtemp(join(tmpdir(), 'starframe-delayed-storage-'));
await writeFile(join(delayed, 'hold-storage-startup'), '');
await withDesktop(delayed, async (page) => {
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByRole('button', { name: 'Run responsiveness check' }).click();
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await expect(page.getByRole('progressbar')).toHaveCount(3);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByText('Opening saved data…')).toBeVisible();
  await unlink(join(delayed, 'hold-storage-startup'));
  await expect(
    page.getByText('Saved locally: 0 library entries and 0 collections.'),
  ).toBeVisible({ timeout: 30000 });
});
console.log(
  'Navigation and diagnostics remained usable while storage startup was held on its worker.',
);
