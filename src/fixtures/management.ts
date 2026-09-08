import type {
  CatalogMod,
  ManagementTransport,
  ModView,
  PackageOperation,
  Reference,
  SharingReply,
} from '../lib/management';
import { key } from '../lib/management';
export function fixtureManagement(
  changed: (data: ModView) => void,
): ManagementTransport {
  const populated = new URLSearchParams(location.search).has('mods');
  const count = new URLSearchParams(location.search).has('large') ? 1200 : 4;
  const mods: CatalogMod[] = populated
    ? Array.from({ length: count }, (_, i) => ({
        id: `fixture.mod${i}`,
        name:
          [
            'Terrain tools fixture',
            'Core library fixture',
            'Withdrawn fixture',
            'Failed transfer fixture',
          ][i] ?? `Mod fixture ${i}`,
        author: 'Test fixture',
        sourceUrl: 'https://example.invalid/source',
        description:
          'Inert test data for the management screens. No author mod is downloaded or executed.',
        unmaintained: i === 0,
        releases: [
          {
            id: `fixture.mod${i}.1`,
            version: '1.0',
            withdrawn: i === 2,
            withdrawalReason: i === 2 ? 'Author removed this release.' : null,
            compatibilityProblems:
              i === 1
                ? [
                    {
                      gameBuild: 'Steam 123 · Unity fixture',
                      note: 'Fixture conflict with this build.',
                      sourceUrl: 'https://example.invalid/report',
                    },
                  ]
                : [],
            testedGameBuilds: ['Steam 100 · Unity fixture'],
            requires: [],
            artifact: {
              url: 'https://example.invalid/package.zip',
              sha256: i.toString(16).padStart(64, '0'),
              sizeBytes: 10485760,
              layout: {
                kind: 'starframe_managed_zip',
                root: 'package',
                entryAssembly: 'Mod.dll',
                entryType: 'Fixture.Mod',
              },
            },
          },
        ],
      }))
    : [];
  const data: ModView = {
    localSources: [],
    localWatches: [],
    imports: [],
    revision: '0',
    activeCollection: null,
    catalog: { schemaVersion: 1, catalogRevision: '1', mods },
    library: mods.slice(0, 3).map((m) => ({
      name: m.name,
      author: m.author,
      version: m.releases[0].version,
      reference: {
        modId: m.id,
        hash: m.releases[0].artifact.sha256,
        origin: 'catalog',
        releaseId: m.releases[0].id,
      },
    })),
    enabled: [],
    order: { effective: [], adjustments: [] },
    orderError: null,
    collisions: [],
    collections: [],
    cleanupErrors: [],
  };
  const operations: PackageOperation[] = [];
  const commit = () => {
    const active = data.collections.find((c) => c.id === data.activeCollection);
    if (active) active.entries = [...data.enabled];
    data.order = { effective: [...data.enabled], adjustments: [] };
    data.revision = String(BigInt(data.revision) + 1n);
    changed(data);
  };
  return {
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
            schemaVersion: 1,
            name: collection.name,
            entries: collection.entries,
          }
        : JSON.parse('text' in action ? action.text : '{}');
      if (
        document.format !== 'starframe-collection' ||
        document.schemaVersion !== 1
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
              (key) => !['modId', 'hash', 'origin', 'releaseId'].includes(key),
            )
          )
            throw new Error('Invalid collection reference: unknown field.');
          const release = mods
            .find((m) => m.id === reference.modId)
            ?.releases.find(
              (r) =>
                r.id === reference.releaseId &&
                r.artifact.sha256 === reference.hash,
            );
          const local = data.library.some(
            (e) => JSON.stringify(e.reference) === JSON.stringify(reference),
          );
          const available =
            reference.origin === 'local_import'
              ? local
              : release && !release.withdrawn;
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
          data.library.some(
            (e) =>
              e.reference.modId === entry.reference.modId &&
              e.reference.hash === entry.reference.hash,
          )
        ) {
          setTimeout(() => {
            entry.status = 'ready';
            entry.message = 'Exact package verified and available.';
            commit();
          }, 600);
        } else {
          await this.packages({
            kind: 'prepare',
            requestId: crypto.randomUUID(),
            releaseId: entry.reference.releaseId!,
          });
          const operation = operations.find(
            (o) => o.releaseId === entry.reference.releaseId,
          )!;
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
          const reference = data.enabled.find((r) => r.modId === id);
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
            ? r.modId !== action.reference.modId
            : key(r) !== key(action.reference),
        );
        if (action.kind === 'set_enabled' && action.enabled)
          data.enabled.push(entry.reference);
        if (action.kind === 'uninstall') {
          data.library = data.library.filter((e) => e !== entry);
          data.localSources = data.localSources.filter(
            (s) => key(s.reference) !== key(action.reference),
          );
          data.localWatches = data.localWatches.filter(
            (w) => key(w.source.reference) !== key(action.reference),
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
        const reference: Reference = {
          modId: 'fixture.local',
          hash: 'b'.repeat(64),
          origin: 'local_import',
          releaseId: null,
        };
        operations.unshift({
          id: crypto.randomUUID(),
          requestId: action.requestId,
          releaseId: 'local-import',
          hash: reference.hash,
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
          });
          data.localSources.push({
            reference,
            path: action.path,
            manifest: {
              schemaVersion: 1,
              modId: reference.modId,
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
      if (action.kind === 'prepare') {
        const mod = mods.find((m) =>
          m.releases.some((r) => r.id === action.releaseId),
        );
        const release = mod?.releases[0];
        if (!release || release.withdrawn)
          throw new Error('Release withdrawn. New downloads are blocked.');
        if (
          operations.some(
            (o) =>
              o.releaseId === release.id &&
              (o.status === 'preparing' || o.status === 'cancelling'),
          )
        )
          throw new Error('This release is already being prepared.');
        const op: PackageOperation = {
          id: crypto.randomUUID(),
          requestId: action.requestId,
          releaseId: release.id,
          hash: release.artifact.sha256,
          status: 'preparing',
          message: 'Downloading fixture bytes…',
          receivedBytes: 0,
          totalBytes: release.artifact.sizeBytes,
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
            release.id === 'fixture.mod3.1'
          ) {
            op.status = 'failed';
            op.message =
              'Download unavailable: HTTP 404. Retry or check the author source.';
            clearInterval(timer);
          } else if (op.receivedBytes >= op.totalBytes) {
            op.status = 'completed';
            op.message = 'Package verified in the library.';
            if (!data.library.some((e) => e.reference.hash === op.hash))
              data.library.push({
                name: mod!.name,
                author: mod!.author,
                version: release.version,
                reference: {
                  modId: mod!.id,
                  hash: op.hash,
                  origin: 'catalog',
                  releaseId: release.id,
                },
              });
            commit();
            clearInterval(timer);
          }
        }, 400);
      } else if (action.kind === 'cancel') {
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
