import { writable } from 'svelte/store';
import { confirmed, errorMessage } from './state';

import type {
  CollectionReference as Reference,
  InstalledEntry as LibraryEntry,
  ModView,
  PackageOperation,
  ModAction,
  PackageAction,
  SharingAction,
  SharingReply,
} from './generated/management';
export type {
  CollectionReference as Reference,
  InstalledEntry as LibraryEntry,
  ModView,
  PackageOperation,
  ModAction,
  PackageAction,
  ImportEntry,
  SharingAction,
  SharingReply,
} from './generated/management';
export type CollectionEdit =
  | { kind: 'create_collection'; name: string }
  | { kind: 'rename_collection'; id: string; name: string }
  | { kind: 'delete_collection' | 'select_collection'; id: string };
export interface ManagementTransport {
  pickLocalSource(folder: boolean): Promise<string | null>;
  saveCollection(text: string): Promise<boolean>;
  sharing(action: SharingAction): Promise<SharingReply>;
  mods(action: ModAction): Promise<ModView>;
  packages(action: PackageAction): Promise<PackageOperation[]>;
  retryRegistryReceipts(): Promise<number>;
}
export const key = (reference: Reference) =>
  JSON.stringify([
    reference.kind,
    reference.reference.modId,
    reference.kind === 'registry' ? reference.reference.releaseId : null,
    reference.reference.sha256,
  ]);
export const runtimeId = (reference: Reference) =>
  reference.kind === 'registry'
    ? `registry.${reference.reference.modId}`
    : reference.reference.modId;
export const referenceLabel = (reference: Reference) =>
  reference.kind === 'registry'
    ? `Mod ${reference.reference.modId}`
    : reference.reference.modId;
export const referenceRelease = (reference: Reference) =>
  reference.kind === 'registry'
    ? reference.reference.releaseId
    : 'Local import';
export const localKey = (
  reference: import('./generated/management').Reference,
) =>
  key({
    kind: 'local',
    reference: { modId: reference.modId, sha256: reference.hash },
  });
export const transferring = (op: PackageOperation) =>
  op.status === 'preparing' || op.status === 'cancelling';
export const confirmedFinding = (
  data: ModView | null,
  hash: string | undefined,
) => !!(hash && data?.blocked[hash]);
export const bytes = (value: number) =>
  `${(value / 1024 / 1024).toLocaleString(undefined, { maximumFractionDigits: 1 })} MiB`;
export function compatibility(
  build: string | null,
  selected: string | undefined,
) {
  if (!build) return 'Compatibility metadata unavailable';
  if (!selected) return 'Choose a game to check compatibility';
  return build === selected
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
    async pickLocalSource(folder: boolean) {
      let path: string | null = null;
      await run('local-picker', async () => {
        path = await transport!.pickLocalSource(folder);
      });
      return path as string | null;
    },
    importLocal(path: string) {
      return run('local-import', async () => {
        const operations = await confirmed(
          transport!.packages({
            kind: 'import_local',
            path,
            requestId: crypto.randomUUID(),
          }),
        );
        if (!stopped) update({ operations });
      });
    },
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
              reference: entry.reference,
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
            reference: entry.reference,
            expectedRevision,
            confirmReferences: true,
          }),
        );
        if (!stopped) update({ data });
      });
    },
    retryReceipts() {
      return run('receipts', async () => {
        await confirmed(transport!.retryRegistryReceipts());
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
