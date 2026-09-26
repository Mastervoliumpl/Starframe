import type {
  LibraryEntry,
  ManagementTransport,
  ModView,
  PackageOperation,
  Reference,
  SharingReply,
} from '../lib/management';
import { key, localKey, runtimeId } from '../lib/management';
export function fixtureManagement(
  changed: (data: ModView) => void,
): ManagementTransport {
  const populated = new URLSearchParams(location.search).has('mods');
  const count = new URLSearchParams(location.search).has('large') ? 1200 : 4;
  const mods: LibraryEntry[] = populated
    ? Array.from({ length: count }, (_, i) => ({
        name:
          [
            'Terrain tools fixture',
            'Core library fixture',
            'Withdrawn fixture',
            'Failed transfer fixture',
          ][i] ?? `Mod fixture ${i}`,
        author: 'Test fixture',
        version: '1.0',
        kind: 'code',
        testedGameBuild: 'Steam 100 · Unity fixture',
        reference: {
          kind: 'registry',
          reference: {
            modId: i + 1,
            releaseId: `11111111-1111-4111-8111-${(i + 1).toString(16).padStart(12, '0')}`,
            sha256: i.toString(16).padStart(64, '0'),
          },
        },
      }))
    : [];
  const data: ModView = {
    blocked: {},
    localSources: [],
    localWatches: [],
    imports: [],
    revision: '0',
    activeCollection: null,
    library: mods.slice(0, 3),
    enabled: [],
    order: { effective: [], adjustments: [] },
    orderError: null,
    collisions: [],
    collections: [],
    cleanupErrors: [],
  };
  const operations: PackageOperation[] = [];
  if (new URLSearchParams(location.search).has('registryArchive')) {
    operations.push(
      {
        id: crypto.randomUUID(),
        requestId: crypto.randomUUID(),
        releaseId: '44444444-4444-4444-8444-444444444444',
        hash: 'c'.repeat(64),
        kind: 'registry_verification',
        receiptId: null,
        status: 'failed',
        message: 'The exact managed content changed.',
        receivedBytes: 0,
        totalBytes: 10,
      },
      {
        id: crypto.randomUUID(),
        requestId: crypto.randomUUID(),
        releaseId: '11111111-1111-4111-8111-111111111111',
        hash: 'a'.repeat(64),
        kind: 'registry_archive',
        receiptId: null,
        status: 'completed',
        message: 'Verified registry archive saved.',
        receivedBytes: 10,
        totalBytes: 10,
      },
      {
        id: crypto.randomUUID(),
        requestId: crypto.randomUUID(),
        releaseId: '22222222-2222-4222-8222-222222222222',
        hash: 'b'.repeat(64),
        kind: 'registry_archive',
        receiptId: '33333333-3333-4333-8333-333333333333',
        status: 'failed',
        message: 'The signed release is no longer available.',
        receivedBytes: 0,
        totalBytes: 10,
      },
    );
  }
  const security = new URLSearchParams(location.search).get('security');
  if (populated && ['blocked', 'cleared'].includes(security ?? '')) {
    const hash = data.library[0].reference.reference.sha256;
    if (security === 'blocked')
      data.blocked[hash] = 'Confirmed fixture security block';
    data.enabled = [data.library[0].reference];
  }
  const commit = () => {
    const active = data.collections.find((c) => c.id === data.activeCollection);
    if (active) active.entries = [...data.enabled];
    data.order = { effective: [...data.enabled], adjustments: [] };
    data.revision = String(BigInt(data.revision) + 1n);
    changed(data);
  };
  function transfer(reference: Reference): PackageOperation {
    const mod = mods.find((entry) => key(entry.reference) === key(reference));
    if (
      !mod ||
      reference.kind !== 'registry' ||
      reference.reference.modId === 3
    )
      throw new Error('Exact release unavailable.');
    const op: PackageOperation = {
      id: crypto.randomUUID(),
      requestId: crypto.randomUUID(),
      releaseId: reference.reference.releaseId,
      hash: reference.reference.sha256,
      kind: 'registry_install',
      receiptId: null,
      status: 'preparing',
      message: 'Downloading fixture bytes…',
      receivedBytes: 0,
      totalBytes: 10485760,
    };
    operations.unshift(op);
    const timer = setInterval(() => {
      if (op.status !== 'preparing') {
        clearInterval(timer);
        return;
      }
      op.receivedBytes += 1048576;
      if (
        op.receivedBytes >= op.totalBytes / 2 &&
        reference.reference.modId === 4
      ) {
        op.status = 'failed';
        op.message = 'Download unavailable: HTTP 404.';
        clearInterval(timer);
      } else if (op.receivedBytes >= op.totalBytes) {
        op.status = 'completed';
        op.message = 'Package verified in the library.';
        if (
          !data.library.some((entry) => key(entry.reference) === key(reference))
        ) {
          data.library.push(mod);
        }
        commit();
        clearInterval(timer);
      }
    }, 400);
    return op;
  }
  return {
    async retryRegistryReceipts() {
      return 0;
    },
    async pickLocalSource(folder) {
      return folder ? 'C:\\fixture\\local-build' : 'C:\\fixture\\Local.dll';
    },
    async saveCollection(text) {
      const url = URL.createObjectURL(
        new Blob([text], { type: 'application/json' }),
      );
      const link = document.createElement('a');
      link.href = url;
      link.download = 'collection.starframe-collection.json';
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      return true;
    },
    async sharing(action) {
      const collection =
        'id' in action
          ? data.collections.find((c) => c.id === action.id)
          : null;
      const document: {
        format: string;
        schemaVersion: number;
        name: string;
        entries: Reference[];
      } = collection
        ? {
            format: 'starframe-collection',
            schemaVersion: 2,
            name: collection.name,
            entries: collection.entries,
          }
        : JSON.parse('text' in action ? action.text : '{}');
      if (
        document.format !== 'starframe-collection' ||
        document.schemaVersion !== 2
      )
        throw new Error('Unsupported collection format or version.');
      if (
        Object.keys(document).some(
          (key) =>
            !['format', 'schemaVersion', 'name', 'entries'].includes(key),
        )
      )
        throw new Error('Invalid collection file: unknown field.');
      if (
        !document.name?.trim() ||
        !Array.isArray(document.entries) ||
        document.entries.length > 256
      )
        throw new Error('Invalid collection name or entries.');
      const reply: SharingReply = {
        text: null,
        name: document.name,
        collectionId: null,
        orderError: null,
        entries: document.entries.map((reference) => {
          if (
            Object.keys(reference).some(
              (field) => !['kind', 'reference'].includes(field),
            )
          )
            throw new Error('Invalid collection reference: unknown field.');
          const release = mods.find(
            (mod) => key(mod.reference) === key(reference),
          );
          const local = data.library.some(
            (e) => key(e.reference) === key(reference),
          );
          const available =
            reference.kind === 'local'
              ? local
              : !!release && reference.reference.modId !== 3;
          return {
            reference,
            status: available ? 'pending' : 'unresolved',
            message: !available
              ? 'Exact release unavailable or withdrawn. No substitute was selected.'
              : local
                ? 'Already downloaded; verify local files before reuse.'
                : 'Download and verify this exact approved release.',
            operationId: null,
          };
        }),
      };
      if (action.kind === 'export')
        return { ...reply, text: JSON.stringify(document, null, 2) };
      if (action.kind === 'review') return reply;
      const id = action.kind === 'accept' ? action.requestId : action.id;
      if (!data.collections.some((c) => c.id === id))
        data.collections.push({
          id,
          name: document.name,
          entries: document.entries,
          revision: 1,
        });
      const imported = { collectionId: id, entries: reply.entries };
      data.imports = [
        ...data.imports.filter((i) => i.collectionId !== id),
        imported,
      ];
      commit();
      for (const entry of imported.entries) {
        if (entry.status !== 'pending') continue;
        entry.status = 'preparing';
        if (
          data.library.some((e) => key(e.reference) === key(entry.reference))
        ) {
          setTimeout(() => {
            entry.status = 'ready';
            entry.message = 'Exact package verified and available.';
            commit();
          }, 600);
        } else {
          const operation = transfer(entry.reference);
          entry.operationId = operation.id;
          const timer = setInterval(() => {
            if (
              operation.status === 'preparing' ||
              operation.status === 'cancelling'
            )
              return;
            entry.status =
              operation.status === 'completed' ? 'ready' : 'unresolved';
            entry.message = operation.message;
            clearInterval(timer);
            commit();
          }, 100);
        }
      }
      return { ...reply, collectionId: id };
    },
    async mods(action) {
      if (
        'expectedRevision' in action &&
        action.expectedRevision !== data.revision
      )
        throw new Error(
          'The library changed. Retry with its current revision.',
        );
      if (action.kind === 'create_collection') {
        data.collections.push({
          id: crypto.randomUUID(),
          name: action.name.trim(),
          revision: 1,
          entries: [],
        });
        commit();
      }
      if (
        action.kind === 'rename_collection' ||
        action.kind === 'delete_collection' ||
        action.kind === 'select_collection'
      ) {
        const selected = data.collections.find((c) => c.id === action.id);
        if (!selected) throw new Error('This collection no longer exists.');
        if (action.kind === 'rename_collection') {
          selected.name = action.name.trim();
          selected.revision++;
        }
        if (action.kind === 'select_collection') {
          data.activeCollection = selected.id;
          data.enabled = [...selected.entries];
        }
        if (action.kind === 'delete_collection') {
          data.collections = data.collections.filter(
            (c) => c.id !== selected.id,
          );
          if (data.activeCollection === selected.id) {
            data.activeCollection = null;
            data.enabled = [];
          }
        }
        commit();
      }
      if (action.kind === 'reorder') {
        if (action.expectedRevision !== data.revision)
          throw new Error(
            'The collection changed. Retry with its current revision.',
          );
        if (
          action.modIds.length !== data.enabled.length ||
          new Set(action.modIds).size !== data.enabled.length
        )
          throw new Error('Include each enabled mod once.');
        data.enabled = action.modIds.map((id) => {
          const reference = data.enabled.find((r) => runtimeId(r) === id);
          if (!reference) throw new Error('Unknown enabled mod.');
          return reference;
        });
        commit();
      }
      if (action.kind === 'set_enabled' || action.kind === 'uninstall') {
        if (action.expectedRevision !== data.revision)
          throw new Error(
            'The library changed. Retry with its current revision.',
          );
        const entry = data.library.find(
          (e) => key(e.reference) === key(action.reference),
        );
        if (!entry) throw new Error('Exact release is not installed.');
        data.enabled = data.enabled.filter((r) =>
          action.kind === 'set_enabled' && action.enabled
            ? runtimeId(r) !== runtimeId(action.reference)
            : key(r) !== key(action.reference),
        );
        if (action.kind === 'set_enabled' && action.enabled)
          data.enabled.push(entry.reference);
        if (action.kind === 'uninstall') {
          data.library = data.library.filter((e) => e !== entry);
          data.localSources = data.localSources.filter(
            (s) => localKey(s.reference) !== key(action.reference),
          );
          data.localWatches = data.localWatches.filter(
            (w) => localKey(w.source.reference) !== key(action.reference),
          );
        }
        if (!data.activeCollection) {
          data.activeCollection = crypto.randomUUID();
          data.collections.push({
            id: data.activeCollection,
            name: 'Default',
            revision: 1,
            entries: [...data.enabled],
          });
        }
        commit();
      }
      return structuredClone(data);
    },
    async packages(action) {
      if (action.kind === 'import_local') {
        if (!action.path.includes('fixture'))
          throw new Error(
            'The browser fixture accepts only fixture source paths.',
          );
        const sourceReference = {
          modId: 'fixture.local',
          hash: 'b'.repeat(64),
          origin: 'local_import' as const,
          releaseId: null,
        };
        const reference: Reference = {
          kind: 'local',
          reference: {
            modId: sourceReference.modId,
            sha256: sourceReference.hash,
          },
        };

        operations.unshift({
          id: crypto.randomUUID(),
          requestId: action.requestId,
          releaseId: 'local-import',
          hash: reference.reference.sha256,
          kind: 'package',
          receiptId: null,
          status: 'completed',
          message: 'Local fixture copied into the library.',
          receivedBytes: 100,
          totalBytes: 100,
        });
        if (!data.library.some((e) => key(e.reference) === key(reference))) {
          data.library.push({
            reference,
            name: 'Local build fixture',
            author: 'Test fixture',
            version: 'dev.1',
            kind: 'code',
            testedGameBuild: null,
          });
          data.localSources.push({
            reference: sourceReference,
            path: action.path,
            manifest: {
              schemaVersion: 1,
              modId: reference.reference.modId,
              name: 'Local build fixture',
              author: 'Test fixture',
              version: 'dev.1',
              layout: {
                kind: 'starframe_managed_zip',
                root: '',
                entryAssembly: 'Local.dll',
                entryType: 'Fixture.Local',
              },
              requires: [],
              loadBefore: [],
              loadAfter: [],
              preferBefore: [],
              preferAfter: [],
            },
          });
          data.localWatches.push({
            source: data.localSources.at(-1)!,
            state: 'watching',
            message:
              'Watching the source while Starframe is open. The verified copy is current.',
          });
          commit();
        }
      }
      if (action.kind === 'cancel') {
        const op = operations.find((o) => o.id === action.operationId);
        if (op?.status === 'preparing') {
          op.status = 'cancelled';
          op.message = 'Cancelled. No game files changed.';
        }
      }
      return structuredClone(operations);
    },
  };
}
