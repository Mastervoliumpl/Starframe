import { writable } from 'svelte/store';
import { errorMessage } from './state';
import type {
  CollectionReference,
  PackageOperation,
} from './generated/management';
import type {
  ListQuery,
  ModList,
  ModResult,
  Options,
  Release,
  ReleaseHistory,
  ReleaseResult,
} from './generated/registry';

export interface RegistryTransport {
  registryList(query: ListQuery): Promise<ModList>;
  registryOptions(): Promise<Options>;
  registryDetail(modId: number): Promise<ModResult>;
  registryHistory(modId: number, page: number): Promise<ReleaseHistory>;
  registryRelease(releaseId: string): Promise<ReleaseResult>;
  registryInstall(
    requestId: string,
    reference: Extract<CollectionReference, { kind: 'registry' }>['reference'],
  ): Promise<PackageOperation[]>;
}

export type Layout = 'rows' | 'cards' | 'tiles';
export const sizes: Record<Layout, number[]> = {
  rows: [10, 20, 50],
  cards: [6, 12, 24],
  tiles: [12, 24, 48],
};
export const initialQuery: ListQuery = {
  page: 1,
  pageSize: 10,
  query: '',
  includeTags: [],
  excludeTags: [],
  sort: 'updated',
  period: 'all',
  maintenance: 'all',
  modType: 'all',
  gameBuilds: [],
};
export const exact = (
  release: Release,
): Extract<CollectionReference, { kind: 'registry' }> => ({
  kind: 'registry',
  reference: {
    modId: release.modId,
    releaseId: release.releaseId,
    sha256: release.artifact.sha256,
  },
});
export const releaseKey = (release: Release) =>
  `${release.modId}:${release.releaseId}:${release.artifact.sha256}`;
export const mediaUrl = (id: string, variant: 'thumbnail' | 'display') =>
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(
    id,
  )
    ? `https://api.starframemanager.com/v1/media/${id}/${variant}`
    : '';

type View = {
  account: string | null;
  query: ListQuery;
  layout: Layout;
  options: Options | null;
  results: ModList | null;
  loading: boolean;
  error: string;
  selected: number | null;
  detail: ModResult | null;
  history: ReleaseHistory | null;
  release: ReleaseResult | null;
  detailLoading: boolean;
  detailError: string;
  installing: string[];
  installError: string;
};

export function createRegistry(transport: RegistryTransport | null) {
  let view: View = {
    account: null,
    query: structuredClone(initialQuery),
    layout: 'rows',
    options: null,
    results: null,
    loading: false,
    error: '',
    selected: null,
    detail: null,
    history: null,
    release: null,
    detailLoading: false,
    detailError: '',
    installing: [],
    installError: '',
  };
  const store = writable(view);
  const update = (patch: Partial<View>) => {
    view = { ...view, ...patch };
    store.set(view);
  };
  let generation = 0;
  let detailGeneration = 0;
  let stopped = false;
  let savedQuery = '';
  let pendingQuery = '';
  let timer: ReturnType<typeof setInterval> | undefined;

  async function browse(force = false) {
    if (!transport || !view.account || stopped) return;
    const query = structuredClone(view.query);
    const serialized = JSON.stringify(query);
    if (!force && view.loading && pendingQuery === serialized) return;
    if (!force && savedQuery === serialized && view.results) return;
    const current = ++generation;
    pendingQuery = serialized;
    update({ loading: true, error: '' });
    try {
      const [results, options] = await Promise.all([
        transport.registryList(query),
        view.options && !force
          ? Promise.resolve(view.options)
          : transport.registryOptions(),
      ]);
      if (stopped || current !== generation) return;
      savedQuery = serialized;
      update({
        results,
        options,
        loading: false,
        query: { ...view.query, page: results.pagination.page },
      });
    } catch (error) {
      if (!stopped && current === generation)
        update({ loading: false, error: errorMessage(error) });
    }
  }

  async function detail(modId: number, page = 1) {
    if (!transport || !view.account || stopped) return;
    const current = ++detailGeneration;
    const changed = view.selected !== modId;
    update({
      selected: modId,
      detailLoading: true,
      detailError: '',
      ...(changed
        ? { detail: null, release: null, history: null, installError: '' }
        : {}),
    });
    try {
      const [detail, history] = await Promise.all([
        transport.registryDetail(modId),
        transport.registryHistory(modId, page),
      ]);
      if (stopped || current !== detailGeneration) return;
      update({
        detail,
        history,
        release:
          changed || !view.release
            ? 'listing' in detail
              ? detail.latestRelease
              : detail
            : view.release,
        detailLoading: false,
      });
    } catch (error) {
      if (!stopped && current === detailGeneration)
        update({ detailLoading: false, detailError: errorMessage(error) });
    }
  }

  return {
    subscribe: store.subscribe,
    start() {
      timer = setInterval(() => {
        if (!view.loading) void browse(true);
      }, 300_000);
    },
    account(account: string | null) {
      if (view.account === account) return false;
      generation++;
      detailGeneration++;
      savedQuery = '';
      update({
        account,
        results: null,
        options: null,
        loading: false,
        selected: null,
        detail: null,
        history: null,
        release: null,
        detailLoading: false,
        error: '',
        detailError: '',
        installError: '',
      });
      return true;
    },
    browse,
    query(patch: Partial<ListQuery>, paging = false) {
      update({
        query: { ...view.query, ...patch, ...(paging ? {} : { page: 1 }) },
      });
      void browse(true);
    },
    layout(layout: Layout) {
      if (!(layout in sizes)) return;
      update({
        layout,
        query: { ...view.query, page: 1, pageSize: sizes[layout][0] },
      });
      void browse(true);
    },
    detail,
    back() {
      detailGeneration++;
      update({
        selected: null,
        detailLoading: false,
        detailError: '',
        installError: '',
      });
    },
    async release(releaseId: string) {
      if (!transport || !view.account || stopped) return;
      const current = ++detailGeneration;
      update({ detailLoading: true, detailError: '', installError: '' });
      try {
        const release = await transport.registryRelease(releaseId);
        if (!stopped && current === detailGeneration)
          update({ release, detailLoading: false });
      } catch (error) {
        if (!stopped && current === detailGeneration)
          update({ detailError: errorMessage(error), detailLoading: false });
      }
    },
    async install(release: Release) {
      if (!transport || !view.account || stopped) return false;
      const id = releaseKey(release);
      if (view.installing.includes(id)) return false;
      const account = view.account;
      update({ installing: [...view.installing, id], installError: '' });
      try {
        await transport.registryInstall(
          crypto.randomUUID(),
          exact(release).reference,
        );
        return !stopped && view.account === account;
      } catch (error) {
        if (!stopped && view.account === account)
          update({ installError: errorMessage(error) });
        return false;
      } finally {
        if (!stopped)
          update({
            installing: view.installing.filter((pending) => pending !== id),
          });
      }
    },
    stop() {
      clearInterval(timer);
      stopped = true;
      generation++;
      detailGeneration++;
    },
  };
}
export type Registry = ReturnType<typeof createRegistry>;
