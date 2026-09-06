import { expect } from '@playwright/test';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
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
const original = await readFile(join(root, 'state.db'));
for (const mode of ['corrupt', 'newer']) {
  const invalid = await mkdtemp(join(tmpdir(), 'starframe-storage-'));
  const bytes =
    mode === 'corrupt' ? Buffer.alloc(8192, 0x5a) : Buffer.from(original);
  if (mode === 'newer') bytes.writeUInt32BE(99, 60);
  await writeFile(join(invalid, 'state.db'), bytes);
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
  expect(await readFile(join(invalid, 'state.db'))).toEqual(bytes);
}
console.log(
  'Native saved-data checks passed: fresh startup, restart, corrupt/newer retention and usable navigation. All databases were temporary.',
);
