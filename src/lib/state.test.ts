import { get } from 'svelte/store';
import { afterEach, expect, test, vi } from 'vitest';
import { createDesktop, type Transport } from './state';
import type { Snapshot } from './generated/model';

const snapshot = (revision: string, sessionId = 'session'): Snapshot => ({
  sessionId,
  revision,
  appVersion: '0.1.0-dev.1',
  operations: [],
  game: {
    launch: {
      phase: 'setup_required',
      message: 'Choose a game installation to finish setup.',
      details: [],
    },
    busy: false,
    candidates: [],
    selected: null,
    selectedPath: null,
    running: 'unknown',
    message: 'No game found in this fixture.',
    error: '',
  },
  savedData: {
    status: 'ready',
    revision: '0',
    libraryCount: 0,
    collectionCount: 0,
    activeCollectionName: null,
  },
});
afterEach(() => vi.useRealTimers());

test('a successful reconnect clears its connection error', async () => {
  const transport: Transport = {
    watch: vi
      .fn()
      .mockRejectedValueOnce(new Error('Disconnected'))
      .mockImplementation(async (receive) => {
        receive(snapshot('1'));
        return vi.fn();
      }),
    start: vi.fn(),
    cancel: vi.fn(),
    open: vi.fn(),
    game: vi.fn(),
  };
  const state = createDesktop(transport);
  await state.reconnect();
  expect(get(state).error).toBe('Disconnected');
  await state.reconnect();
  expect(get(state).connection).toBe('connected');
  expect(get(state).error).toBe('');
});

test('stale revisions, sessions and retired subscriptions cannot overwrite current state', async () => {
  const listeners: ((state: Snapshot) => void)[] = [];
  const transport: Transport = {
    watch: vi.fn(async (receive) => {
      listeners.push(receive);
      return vi.fn();
    }),
    start: vi.fn(),
    cancel: vi.fn(),
    open: vi.fn(),
    game: vi.fn(),
  };
  const state = createDesktop(transport);
  const stop = state.startWatching();
  listeners[0](snapshot('9007199254740993'));
  listeners[0](snapshot('9007199254740992'));
  listeners[0](snapshot('9007199254740994', 'old-session'));
  expect(get(state).snapshot?.revision).toBe('9007199254740993');
  await state.reconnect();
  listeners[1](snapshot('2'));
  expect(get(state).snapshot?.revision).toBe('9007199254740993');
  listeners[1](snapshot('1', 'new-session'));
  listeners[0](snapshot('9007199254740995'));
  expect(get(state).snapshot?.sessionId).toBe('new-session');
  stop();
  listeners[1](snapshot('2', 'new-session'));
  expect(get(state).snapshot?.revision).toBe('1');
});

test('a lost acknowledgement can be retried with the same request ID', async () => {
  vi.useFakeTimers();
  const transport: Transport = {
    watch: vi.fn(async (receive) => {
      receive(snapshot('1'));
      return vi.fn();
    }),
    start: vi
      .fn()
      .mockImplementationOnce(() => new Promise(() => {}))
      .mockResolvedValue(['job']),
    cancel: vi.fn(),
    open: vi.fn(),
    game: vi.fn(),
  };
  const state = createDesktop(transport);
  const stop = state.startWatching();
  const first = state.start(true);
  await vi.advanceTimersByTimeAsync(5000);
  await first;
  expect(get(state).starting).toBe(false);
  expect(get(state).error).toContain('not confirmed');
  await state.start(false);
  expect(vi.mocked(transport.start).mock.calls[1]).toEqual(
    vi.mocked(transport.start).mock.calls[0],
  );
  expect(get(state).snapshot?.operations).toEqual([]);
  stop();
});

test('cancellation acknowledgement does not mark an operation cancelled and repeated clicks are ignored', async () => {
  let finish: () => void = () => {};
  const transport: Transport = {
    watch: vi.fn(async (receive) => {
      receive({
        ...snapshot('1'),
        operations: [
          {
            id: 'job',
            requestId: 'request',
            label: 'Test',
            progress: 10,
            status: 'running',
            message: '',
          },
        ],
      });
      return vi.fn();
    }),
    start: vi.fn(),
    cancel: vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    ),
    open: vi.fn(),
    game: vi.fn(),
  };
  const state = createDesktop(transport);
  const stop = state.startWatching();
  const cancel = state.cancel('job');
  void state.cancel('job');
  expect(transport.cancel).toHaveBeenCalledTimes(1);
  expect(get(state).cancelling).toEqual(['job']);
  finish();
  await cancel;
  expect(get(state).snapshot?.operations[0].status).toBe('running');
  expect(get(state).cancelling).toEqual([]);
  stop();
});

test('silent channels reconnect and pending actions do not duplicate or claim success', async () => {
  vi.useFakeTimers();
  let receive: (state: Snapshot) => void = () => {};
  let finish: () => void = () => {};
  const transport: Transport = {
    watch: vi.fn(async (callback) => {
      receive = callback;
      callback(snapshot('1'));
      return vi.fn();
    }),
    start: vi.fn(
      () =>
        new Promise<string[]>((resolve) => {
          finish = () => resolve(['job']);
        }),
    ),
    cancel: vi.fn(),
    open: vi.fn(),
    game: vi.fn(),
  };
  const state = createDesktop(transport);
  const stop = state.startWatching();
  const request = state.start();
  void state.start();
  expect(get(state).starting).toBe(true);
  expect(transport.start).toHaveBeenCalledTimes(1);
  finish();
  await request;
  expect(get(state).snapshot?.operations).toEqual([]);
  receive(snapshot('2'));
  await vi.advanceTimersByTimeAsync(7000);
  expect(transport.watch).toHaveBeenCalledTimes(2);
  stop();
  await vi.advanceTimersByTimeAsync(14000);
  expect(transport.watch).toHaveBeenCalledTimes(2);
});
