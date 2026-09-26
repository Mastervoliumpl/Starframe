import { describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  createRegistry,
  exact,
  initialQuery,
  type RegistryTransport,
} from './registry';
import type { ModList, ModResult, ReleaseResult } from './generated/registry';
import {
  fixtureDetail,
  fixtureRelease,
  fixtureReleaseId,
} from '../fixtures/registry';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => (resolve = done));
  return { promise, resolve };
}
const results = (name: string): ModList => ({
  apiVersion: 1,
  items: [{ ...fixtureDetail(1).listing, name }],
  pagination: {
    page: 1,
    pageSize: 10,
    totalItems: 1,
    totalPages: 1,
    asOf: '2026-09-24T00:00:00Z',
  },
});
function transport(): RegistryTransport {
  return {
    registryList: vi.fn(async () => results('First')),
    registryOptions: vi.fn(async () => ({ tags: [], gameBuilds: [] })),
    registryDetail: vi.fn(async (id) => fixtureDetail(id)),
    registryHistory: vi.fn(async () => ({
      apiVersion: 1 as const,
      items: [],
      pagination: {
        page: 1,
        pageSize: 12,
        totalItems: 0,
        totalPages: 0,
        asOf: '2026-09-24T00:00:00Z',
      },
    })),
    registryRelease: vi.fn(async () => fixtureRelease(1)),
    registryInstall: vi.fn(async () => []),
  };
}
describe('native registry browser', () => {
  it('ignores stale filter responses and does not overlap identical reads', async () => {
    const first = deferred<ModList>();
    const second = deferred<ModList>();
    const api = transport();
    api.registryList = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const registry = createRegistry(api);
    registry.account('fixture');
    const reading = registry.browse();
    void registry.browse();
    expect(api.registryList).toHaveBeenCalledTimes(1);
    registry.query({ query: 'new', includeTags: ['lua'] });
    second.resolve(results('New'));
    await vi.waitFor(() => expect(get(registry).loading).toBe(false));
    first.resolve(results('Old'));
    await reading;
    expect(get(registry).results?.items[0].name).toBe('New');
    expect(api.registryList).toHaveBeenLastCalledWith({
      ...initialQuery,
      query: 'new',
      includeTags: ['lua'],
    });
    await registry.browse();
    expect(api.registryList).toHaveBeenCalledTimes(2);
  });
  it('clears account-scoped responses on sign-out while keeping browse choices', async () => {
    const pending = deferred<ModList>();
    const api = transport();
    api.registryList = vi.fn(() => pending.promise);
    const registry = createRegistry(api);
    registry.account('fixture');
    registry.layout('cards');
    registry.account(null);
    pending.resolve(results('Private account result'));
    await Promise.resolve();
    await Promise.resolve();
    expect(get(registry).results).toBeNull();
    expect(get(registry).layout).toBe('cards');
    expect(get(registry).query.pageSize).toBe(6);
    await registry.browse();
    expect(api.registryList).toHaveBeenCalledTimes(1);
  });
  it('retains results on a scoped network failure and retries only on request', async () => {
    const api = transport();
    const registry = createRegistry(api);
    registry.account('fixture');
    await registry.browse();
    api.registryList = vi.fn().mockRejectedValue(new Error('Offline'));
    await registry.browse(true);
    expect(get(registry).error).toBe('Offline');
    expect(get(registry).results?.items[0].name).toBe('First');
    await registry.browse();
    expect(api.registryList).toHaveBeenCalledTimes(1);
  });
  it('ignores stale detail and history while installing the exact chosen older release', async () => {
    const pending = deferred<ModResult>();
    const api = transport();
    api.registryDetail = vi
      .fn()
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValueOnce(fixtureDetail(2));
    const registry = createRegistry(api);
    registry.account('fixture');
    const first = registry.detail(1);
    await registry.detail(2);
    pending.resolve(fixtureDetail(1));
    await first;
    expect(get(registry).selected).toBe(2);
    api.registryRelease = vi.fn(async () => fixtureRelease(2, 1));
    await registry.release(fixtureReleaseId(2, 1));
    expect(await registry.install(fixtureRelease(2, 1))).toBe(true);
    expect(api.registryInstall).toHaveBeenCalledWith(
      expect.stringMatching(/^[0-9a-f-]{36}$/),
      exact(fixtureRelease(2, 1)).reference,
    );
    expect(get(registry).query).toEqual(initialQuery);
    registry.back();
    expect(get(registry).selected).toBeNull();
  });
  it('does not report a previous account’s install result or restore detail after Back', async () => {
    const api = transport();
    const pending = deferred<ReleaseResult>();
    api.registryRelease = vi.fn(() => pending.promise);
    const registry = createRegistry(api);
    registry.account('fixture');
    await registry.detail(1);
    const changing = registry.release(fixtureReleaseId(1, 1));
    registry.back();
    pending.resolve(fixtureRelease(1, 1));
    await changing;
    expect(get(registry).selected).toBeNull();
    expect(get(registry).release).toEqual(fixtureRelease(1));
    const installing = deferred<[]>();
    api.registryInstall = vi.fn(() => installing.promise);
    const requested = registry.install(fixtureRelease(1));
    registry.account(null);
    installing.resolve([]);
    expect(await requested).toBe(false);
    expect(get(registry).installing).toEqual([]);
  });
});
