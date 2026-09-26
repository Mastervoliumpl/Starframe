import { expect, test, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import {
  createManagement,
  compatibility,
  type ModView,
  type LibraryEntry,
  type ManagementTransport,
} from './management';
const entry: LibraryEntry = {
  name: 'Fixture',
  author: 'Fixture',
  version: '1',
  kind: 'code',
  testedGameBuild: '1',
  reference: {
    kind: 'registry',
    reference: {
      modId: 1,
      sha256: 'a'.repeat(64),
      releaseId: '7f7f7f7f-7f7f-4f7f-8f7f-7f7f7f7f7f7f',
    },
  },
};
const data = (revision = '0'): ModView => ({
  blocked: {},
  localSources: [],
  localWatches: [],
  revision,
  activeCollection: null,
  library: [entry],
  enabled: [],
  order: { effective: [], adjustments: [] },
  orderError: null,
  collisions: [],
  collections: [],
  cleanupErrors: [],
  imports: [],
});
afterEach(() => vi.useRealTimers());

test('an old refresh cannot overwrite an acknowledged edit; repeated input is ignored', async () => {
  let resolveRead: (value: ModView) => void = () => {};
  let resolveEdit: (value: ModView) => void = () => {};
  const transport: ManagementTransport = {
    mods: vi
      .fn()
      .mockResolvedValueOnce(data())
      .mockImplementationOnce(
        () => new Promise<ModView>((resolve) => (resolveRead = resolve)),
      )
      .mockImplementationOnce(
        () => new Promise<ModView>((resolve) => (resolveEdit = resolve)),
      )
      .mockResolvedValue(data('1')),
    sharing: vi.fn(),
    saveCollection: vi.fn(),
    pickLocalSource: vi.fn(),
    packages: vi.fn().mockResolvedValue([]),
    retryRegistryReceipts: vi.fn().mockResolvedValue(0),
  };
  const manager = createManagement(transport);
  await manager.refresh();
  const oldRead = manager.refresh();
  const edit = manager.membership([entry], true);
  expect(get(manager).data?.enabled).toEqual([]);
  await manager.membership([entry], false);
  expect(transport.mods).toHaveBeenCalledTimes(3);
  resolveEdit({ ...data('1'), enabled: [entry.reference] });
  await edit;
  resolveRead(data());
  await oldRead;
  expect(get(manager).data?.revision).toBe('1');
  expect(get(manager).data?.enabled).toEqual([entry.reference]);
});

test('bulk edits use each confirmed revision and retain partial success on failure', async () => {
  const transport: ManagementTransport = {
    mods: vi
      .fn()
      .mockResolvedValueOnce(data())
      .mockResolvedValueOnce({ ...data('1'), enabled: [entry.reference] })
      .mockRejectedValueOnce(new Error('Missing required release.'))
      .mockResolvedValue(data('1')),
    sharing: vi.fn(),
    saveCollection: vi.fn(),
    pickLocalSource: vi.fn(),
    packages: vi.fn().mockResolvedValue([]),
    retryRegistryReceipts: vi.fn().mockResolvedValue(0),
  };
  const manager = createManagement(transport);
  await manager.refresh();
  await manager.membership(
    [
      entry,
      {
        ...entry,
        reference: {
          kind: 'local',
          reference: { modId: 'other', sha256: 'b'.repeat(64) },
        },
      },
    ],
    true,
  );
  expect(vi.mocked(transport.mods).mock.calls[2][0]).toMatchObject({
    expectedRevision: '1',
    reference: { kind: 'local', reference: { modId: 'other' } },
  });
  expect(get(manager).error).toContain('Missing required release');
  expect(get(manager).pending).toEqual([]);
});

test('silent operation replies time out without claiming installation', async () => {
  vi.useFakeTimers();
  const manager = createManagement({
    mods: vi.fn().mockResolvedValue(data()),
    sharing: vi.fn(),
    saveCollection: vi.fn(),
    pickLocalSource: vi.fn(),
    packages: vi.fn().mockImplementation(() => new Promise(() => {})),
    retryRegistryReceipts: vi.fn().mockResolvedValue(0),
  });
  const install = manager.importLocal('fixture.dll');
  await vi.advanceTimersByTimeAsync(5000);
  expect(await install).toBe(false);
  expect(get(manager).error).toContain('not confirmed');
  expect(get(manager).operations).toEqual([]);
});

test('game build changes recalculate evidence without an activation ban', () => {
  expect(compatibility('old', 'old')).toBe('Tested with this version');
  expect(compatibility('old', 'new')).toBe('Not tested with this version');
  expect(compatibility('old', undefined)).toBe(
    'Choose a game to check compatibility',
  );
  expect(compatibility(null, 'new')).toBe('Compatibility metadata unavailable');
});
