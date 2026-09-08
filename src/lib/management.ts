import { writable } from 'svelte/store';
import { confirmed, errorMessage } from './state';

export type Reference = {
  modId: string;
  hash: string;
  origin: 'catalog' | 'local_import';
  releaseId: string | null;
};
export type LibraryEntry = {
  reference: Reference;
  name: string;
  author: string;
  version: string;
};
export type Release = {
  id: string;
  version: string;
  withdrawn: boolean;
  withdrawalReason: string | null;
  compatibilityProblems: {
    gameBuild: string;
    note: string;
    sourceUrl: string;
  }[];
  testedGameBuilds: string[];
  requires: string[];
  artifact: {
    url: string;
    sha256: string;
    sizeBytes: number;
    layout:
      | { kind: 'starframe_lua_zip' }
      | {
          kind: 'starframe_managed_zip';
          root: string;
          entryAssembly: string;
          entryType: string;
        };
  };
};
export type CatalogMod = {
  id: string;
  name: string;
  author: string;
  sourceUrl: string;
  description: string;
  unmaintained: boolean;
  releases: Release[];
};
export type ModView = {
  imports: { collectionId: string; entries: ImportEntry[] }[];
  revision: string;
  activeCollection: string | null;
  catalog: {
    schemaVersion: number;
    catalogRevision: string;
    mods: CatalogMod[];
  } | null;
  library: LibraryEntry[];
  enabled: Reference[];
  order: {
    effective: Reference[];
    adjustments: { before: string; after: string; message: string }[];
  } | null;
  orderError: string | null;
  collisions: { path: string; mods: string[]; winner: string }[];
  collections: {
    id: string;
    name: string;
    revision: number;
    entries: Reference[];
  }[];
  cleanupErrors: string[];
};
export type PackageOperation = {
  id: string;
  requestId: string;
  releaseId: string;
  hash: string;
  status: 'preparing' | 'cancelling' | 'cancelled' | 'completed' | 'failed';
  message: string;
  receivedBytes: number;
  totalBytes: number;
};
export type ModAction =
  | { kind: 'list' | 'retry_cleanup' }
  | { kind: 'create_collection'; name: string; expectedRevision: string }
  | {
      kind: 'rename_collection';
      id: string;
      name: string;
      expectedRevision: string;
    }
  | {
      kind: 'delete_collection' | 'select_collection';
      id: string;
      expectedRevision: string;
    }
  | { kind: 'reorder'; modIds: string[]; expectedRevision: string }
  | {
      kind: 'set_enabled';
      modId: string;
      hash: string;
      enabled: boolean;
      expectedRevision: string;
    }
  | {
      kind: 'uninstall';
      modId: string;
      hash: string;
      expectedRevision: string;
      confirmReferences: boolean;
    };
export type PackageAction =
  | { kind: 'list' }
  | { kind: 'prepare'; requestId: string; releaseId: string }
  | { kind: 'cancel'; operationId: string };
export type CollectionEdit =
  | { kind: 'create_collection'; name: string }
  | { kind: 'rename_collection'; id: string; name: string }
  | { kind: 'delete_collection' | 'select_collection'; id: string };
export type ImportEntry = {
  reference: Reference;
  status: 'pending' | 'preparing' | 'ready' | 'unresolved';
  message: string;
  operationId: string | null;
};
export type SharingAction =
  | { kind: 'review'; text: string }
  | {
      kind: 'accept';
      text: string;
      requestId: string;
      expectedRevision: string;
    }
  | { kind: 'retry' | 'export'; id: string };
export type SharingReply = {
  text: string | null;
  name: string;
  entries: ImportEntry[];
  collectionId: string | null;
  orderError: string | null;
};
export interface ManagementTransport {
  saveCollection(text: string): Promise<boolean>;
  sharing(action: SharingAction): Promise<SharingReply>;
  mods(action: ModAction): Promise<ModView>;
  packages(action: PackageAction): Promise<PackageOperation[]>;
}
export const key = (reference: Reference) =>
  `${reference.modId}:${reference.hash}`;
export const transferring = (op: PackageOperation) =>
  op.status === 'preparing' || op.status === 'cancelling';
export const bytes = (value: number) =>
  `${(value / 1024 / 1024).toLocaleString(undefined, { maximumFractionDigits: 1 })} MiB`;
export function compatibility(
  release: Release | undefined,
  build: string | undefined,
) {
  if (!release) return 'Compatibility metadata unavailable';
  if (!build) return 'Choose a game to check compatibility';
  if (release.compatibilityProblems.some((p) => p.gameBuild === build))
    return 'Known compatibility problem';
  return release.testedGameBuilds.includes(build)
    ? 'Tested with this version'
    : 'Not tested with this version';
}

export function createManagement(transport: ManagementTransport | null) {
  let view = {
    data: null as ModView | null,
    operations: [] as PackageOperation[],
    pending: [] as string[],
    error: '',
    loading: !!transport,
  };
  const store = writable(view);
  const update = (patch: Partial<typeof view>) => {
    view = { ...view, ...patch };
    store.set(view);
  };
  let stopped = false;
  let reading = false;
  let generation = 0;
  async function refresh() {
    if (!transport || stopped || reading || view.pending.length) return;
    reading = true;
    const current = generation;
    try {
      const [data, operations] = await confirmed(
        Promise.all([
          transport.mods({ kind: 'list' }),
          transport.packages({ kind: 'list' }),
        ]),
      );
      if (!stopped && current === generation)
        update({ data, operations, loading: false });
    } catch (error) {
      if (!stopped && current === generation)
        update({ error: errorMessage(error), loading: false });
    } finally {
      reading = false;
    }
  }
  async function run(id: string, work: () => Promise<void>) {
    if (!transport || stopped || view.pending.includes(id)) return false;
    generation++;
    update({ pending: [...view.pending, id], error: '' });
    try {
      await work();
      return true;
    } catch (error) {
      if (!stopped) update({ error: errorMessage(error) });
      return false;
    } finally {
      generation++;
      if (!stopped) update({ pending: view.pending.filter((p) => p !== id) });
      void refresh();
    }
  }
  return {
    subscribe: store.subscribe,
    start() {
      void refresh();
      const timer = setInterval(() => void refresh(), 1000);
      return () => {
        stopped = true;
        generation++;
        clearInterval(timer);
      };
    },
    refresh,
    dismissError: () => update({ error: '' }),
    async saveCollection(text: string) {
      let saved = false;
      await run('sharing', async () => {
        saved = await transport!.saveCollection(text);
      });
      return saved;
    },
    async sharing(action: SharingAction) {
      let reply: SharingReply | null = null;
      await run('sharing', async () => {
        reply = await confirmed(transport!.sharing(action));
      });
      return reply as SharingReply | null;
    },
    collection(action: CollectionEdit, expectedRevision?: string) {
      return run('membership', async () => {
        if (!view.data) throw new Error('Collection data is unavailable.');
        const data = await confirmed(
          transport!.mods({
            ...action,
            expectedRevision: expectedRevision ?? view.data.revision,
          }),
        );
        if (!stopped) update({ data });
      });
    },
    reorder(modIds: string[]) {
      return run('membership', async () => {
        if (!view.data) return;
        const data = await confirmed(
          transport!.mods({
            kind: 'reorder',
            modIds,
            expectedRevision: view.data.revision,
          }),
        );
        if (!stopped) update({ data });
      });
    },
    async membership(entries: LibraryEntry[], enabled: boolean) {
      if (view.pending.includes('membership')) return false;
      return run('membership', async () => {
        for (const entry of entries) {
          if (stopped || !view.data) break;
          const data = await confirmed(
            transport!.mods({
              kind: 'set_enabled',
              modId: entry.reference.modId,
              hash: entry.reference.hash,
              enabled,
              expectedRevision: view.data.revision,
            }),
          );
          if (!stopped) update({ data });
        }
      });
    },
    uninstall(entry: LibraryEntry, expectedRevision: string) {
      return run('membership', async () => {
        const data = await confirmed(
          transport!.mods({
            kind: 'uninstall',
            modId: entry.reference.modId,
            hash: entry.reference.hash,
            expectedRevision,
            confirmReferences: true,
          }),
        );
        if (!stopped) update({ data });
      });
    },
    install(releaseId: string) {
      return run(`install:${releaseId}`, async () => {
        const operations = await confirmed(
          transport!.packages({
            kind: 'prepare',
            releaseId,
            requestId: crypto.randomUUID(),
          }),
        );
        if (!stopped) update({ operations });
      });
    },
    cancel(operationId: string) {
      return run(`cancel:${operationId}`, async () => {
        const operations = await confirmed(
          transport!.packages({
            kind: 'cancel',
            operationId,
          }),
        );
        if (!stopped) update({ operations });
      });
    },
    cleanup() {
      return run('cleanup', async () => {
        const data = await confirmed(
          transport!.mods({ kind: 'retry_cleanup' }),
        );
        if (!stopped) update({ data });
      });
    },
  };
}
export type Management = ReturnType<typeof createManagement>;
