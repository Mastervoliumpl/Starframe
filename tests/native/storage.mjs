import { expect } from '@playwright/test';
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  unlink,
  writeFile,
} from 'node:fs/promises';
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

for (const schema of [1, 2, 3]) {
  const converted = await mkdtemp(join(tmpdir(), 'starframe-converted-'));
  await cp(
    join(
      'src-tauri',
      'tests',
      'fixtures',
      'turso-0.7.2',
      `${schema}-populated`,
    ),
    converted,
    { recursive: true },
  );
  const db = await readFile(join(converted, 'state.db'));
  const wal = await readFile(join(converted, 'state.db-wal'));
  for (let run = 0; run < 2; run++) {
    await withDesktop(converted, async (page) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(
        page.getByText('Saved locally: 2 library entries and 2 collections.'),
      ).toBeVisible();
      await page
        .getByRole('button', { name: 'Help & logs', exact: true })
        .click();
      await page
        .getByRole('button', { name: 'Run responsiveness check' })
        .click();
      await page
        .getByRole('button', { name: 'Downloads', exact: true })
        .click();
      await expect(page.getByRole('progressbar')).toHaveCount(3);
    });
  }
  expect(await readFile(join(converted, 'state.db'))).toEqual(db);
  expect(await readFile(join(converted, 'state.db-wal'))).toEqual(wal);
}
console.log(
  'Native conversion checks passed: legacy schemas 1-3, restart, retained originals and usable diagnostics.',
);

const delayed = await mkdtemp(join(tmpdir(), 'starframe-delayed-conversion-'));
await cp(
  join('src-tauri', 'tests', 'fixtures', 'turso-0.7.2', '3-populated'),
  delayed,
  { recursive: true },
);
await writeFile(join(delayed, 'hold-storage-startup'), '');
await withDesktop(delayed, async (page) => {
  await page.getByRole('button', { name: 'Help & logs', exact: true }).click();
  await page.getByRole('button', { name: 'Run responsiveness check' }).click();
  await page.getByRole('button', { name: 'Downloads', exact: true }).click();
  await expect(page.getByRole('progressbar')).toHaveCount(3);
  await unlink(join(delayed, 'hold-storage-startup'));
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(
    page.getByText('Saved locally: 2 library entries and 2 collections.'),
  ).toBeVisible();
});
console.log(
  'Navigation and diagnostics remained usable while storage startup was held on its worker.',
);
