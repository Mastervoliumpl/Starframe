import { createHash, randomUUID } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

export const runtimeId = (reference) =>
  reference.kind === 'registry'
    ? `registry.${reference.reference.modId}`
    : reference.reference.modId;

// Seeds previously installed inert content. Signed network acceptance is tested in Rust.
export async function installedRegistry(
  data,
  modId,
  name,
  files,
  installation,
  dependencies = [],
) {
  const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
  const inventory = Object.entries(files).map(([path, bytes]) => ({
    path,
    sha256: hash(bytes),
    sizeBytes: bytes.length,
  }));
  const sha256 = hash(JSON.stringify(inventory));
  const reference = { modId, releaseId: randomUUID(), sha256 };
  const directory = join(data, 'artifacts', sha256);
  for (const [path, bytes] of Object.entries(files)) {
    const destination = join(directory, ...path.split('/'));
    await mkdir(join(destination, '..'), { recursive: true });
    await writeFile(destination, bytes);
  }
  const db = new DatabaseSync(join(data, 'sqlite/state.db'));
  try {
    db.prepare('INSERT INTO prepared_artifacts(hash,record) VALUES (?,?)').run(
      sha256,
      JSON.stringify({ hash: sha256, files: inventory }),
    );
    db.prepare(
      'INSERT INTO registry_library(mod_id,release_id,sha256,installation_record) VALUES (?,?,?,?)',
    ).run(
      modId,
      reference.releaseId,
      sha256,
      JSON.stringify({
        reference,
        display: { name, author: 'Test fixture' },
        versionLabel: '1',
        testedGameBuild: 'Steam 111 · Unity 0123456789abcdef0123456789abcdef',
        metadataRevision: 1,
        installation,
        dependencies,
      }),
    );
  } finally {
    db.close();
  }
  return { kind: 'registry', reference };
}
