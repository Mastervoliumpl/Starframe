import { expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import {
  mkdtemp,
  mkdir,
  readFile,
  realpath,
  writeFile,
  stat,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { withDesktop } from './session.mjs';

const root = await realpath(
  await mkdtemp(join(tmpdir(), 'starframe-local-import-')),
);
const data = join(root, 'data');
const source = join(root, 'source');
const content = join(source, 'LJ', 'lua', 'fixture.lua');
await mkdir(join(source, 'LJ', 'lua'), { recursive: true });
await writeFile(content, "return 'native fixture'\n");
await writeFile(
  join(source, 'starframe.local.json'),
  JSON.stringify({
    schemaVersion: 1,
    modId: 'fixture.local.native',
    name: 'Native local import fixture',
    author: 'Test fixture',
    version: 'dev.1',
    layout: { kind: 'starframe_lua_zip' },
  }),
);
const original = await readFile(content);
const offline = {
  HTTPS_PROXY: 'http://127.0.0.1:1',
  HTTP_PROXY: 'http://127.0.0.1:1',
  NO_PROXY: '',
  STARFRAME_TEST_STEAM_ROOT: join(root, 'no-steam'),
};
let reference;
await withDesktop(
  data,
  async (page, pid) => {
    const trigger = page.getByRole('button', {
      name: 'Import local mod',
      exact: true,
    });
    await expect(trigger).toBeEnabled();
    await trigger.click();
    const dialog = page.getByRole('dialog', {
      name: 'Import local mod',
      exact: true,
    });
    await dialog.getByRole('button', { name: 'Choose folder' }).click();
    const picker = spawn(
      'powershell.exe',
      [
        '-NoProfile',
        '-ExecutionPolicy',
        'Bypass',
        '-File',
        resolve('tests/native/choose-folder.ps1'),
        '-AppProcessId',
        String(pid),
        '-Folder',
        source,
        '-Title',
        'Import local mod',
      ],
      { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
    );
    let pickerOutput = '';
    picker.stdout.on('data', (chunk) => (pickerOutput += chunk));
    picker.stderr.on('data', (chunk) => (pickerOutput += chunk));
    await expect
      .poll(() => picker.exitCode, {
        timeout: 30000,
        message: 'native source picker completes',
      })
      .not.toBeNull();
    if (picker.exitCode !== 0) throw new Error(pickerOutput);
    await expect(
      dialog.getByRole('textbox', { name: 'Source path' }),
    ).toHaveValue(source);
    await dialog
      .getByRole('button', { name: 'Import copy', exact: true })
      .click();
    await expect(trigger).toBeFocused();
    const enabled = page.getByRole('switch', {
      name: 'Enable Native local import fixture dev.1',
    });
    await expect(enabled).toBeVisible();
    await enabled.click();
    await expect(enabled).toBeChecked();
    const view = await page.evaluate(() =>
      window.__TAURI_INTERNALS__.invoke('mod_action', {
        action: { kind: 'list' },
      }),
    );
    expect(view.catalog).toBeNull();
    expect(view.orderError).toBeNull();
    expect(view.localSources[0].path).toBe(source);
    reference = view.library[0].reference;
    await page
      .getByRole('button', { name: 'Native local import fixture', exact: true })
      .click();
    const details = page.getByRole('complementary', { name: 'Mod details' });
    await expect(details).toContainText(source);
    await expect(
      details.getByRole('heading', { name: 'Compatibility', exact: true }),
    ).toHaveCount(0);
    await mkdir('test-results/native', { recursive: true });
    await page.screenshot({
      path: 'test-results/native/local-import-details.png',
    });
  },
  offline,
);
await writeFile(content, 'author next build\n');
await withDesktop(
  data,
  async (page) => {
    const enabled = page.getByRole('switch', {
      name: 'Enable Native local import fixture dev.1',
    });
    await expect(enabled).toBeChecked();
    expect(
      await readFile(
        join(data, 'artifacts', reference.hash, 'LJ', 'lua', 'fixture.lua'),
      ),
    ).toEqual(original);
    await page
      .getByRole('button', {
        name: 'Uninstall Native local import fixture dev.1',
      })
      .click();
    await expect(page.getByRole('dialog')).toContainText(
      'source folder and local metadata are kept',
    );
    await page
      .getByRole('button', { name: 'Confirm uninstall', exact: true })
      .click();
    await expect(enabled).toHaveCount(0);
    expect(await readFile(content, 'utf8')).toBe('author next build\n');
    await stat(join(source, 'starframe.local.json'));
    await expect
      .poll(async () =>
        stat(join(data, 'artifacts', reference.hash)).then(
          () => true,
          () => false,
        ),
      )
      .toBe(false);
  },
  offline,
);
console.log(
  'Native local folder picker, offline import, normal controls, restart and source retention passed.',
);
