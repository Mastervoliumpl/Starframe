import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  writeFile,
} from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';

const resources = process.env.STARFRAME_INTEGRATION_DIR;
if (!resources)
  throw new Error(
    'Set STARFRAME_INTEGRATION_DIR to staged integration resources.',
  );
const fixtureRoot = resolve('test-results/0.7.0-headless');
await mkdir(fixtureRoot, { recursive: true });
const root = await mkdtemp(join(fixtureRoot, 'workflow-'));
const game = join(root, 'game');
const engine = join(game, 'engine');
const data = join(root, 'data');
await mkdir(join(engine, 'Sanctuary_Data/Managed'), { recursive: true });
await mkdir(join(engine, 'MonoBleedingEdge'), { recursive: true });
const executable = join(engine, 'Sanctuary.exe');
await copyFile(join(process.env.SystemRoot, 'System32/ping.exe'), executable);
await copyFile(executable, join(engine, 'UnityPlayer.dll'));
await writeFile(
  join(engine, 'Sanctuary_Data/app.info'),
  'Enhearten Media PTY\nSanctuary\n',
);
await writeFile(
  join(engine, 'Sanctuary_Data/boot.config'),
  'build-guid=0123456789abcdef0123456789abcdef\n',
);
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
const original = digest(await readFile(executable));
const cli = resolve('src-tauri/target/debug/starframe_headless.exe');
const run = (name, ...args) => {
  const output = execFileSync(cli, [data, name, ...args], {
    encoding: 'utf8',
    windowsHide: true,
    timeout: 30000,
  });
  const result = output
    .trim()
    .split(/\r?\n/)
    .map((line) => {
      try {
        return JSON.parse(line);
      } catch {
        return null;
      }
    })
    .find((item) => item?.ok === true);
  assert.ok(result, `${name} did not return a successful JSON result`);
  return result.data;
};

assert.ok(run('select-game', game).path.endsWith(game));
assert.equal(run('readiness', game).activation, null);
const prepared = run('setup', game, resolve(resources));
assert.equal(prepared.activation.deploymentRevision, '1');
assert.ok(run('readiness', game).activation);
assert.equal(
  run('launch', game, resolve(resources)).activation.deploymentRevision,
  '1',
);
assert.equal(digest(await readFile(executable)), original);
console.log(
  'Headless fake-game selection, setup, readiness and launch passed; game executable unchanged.',
);
