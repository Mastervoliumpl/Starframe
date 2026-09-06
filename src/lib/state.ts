import { writable } from 'svelte/store';
import type { Snapshot } from './generated/model';

export interface Transport {
  watch(receive: (snapshot: Snapshot) => void): Promise<() => void>;
  start(requestId: string, fail: boolean): Promise<string[]>;
  cancel(operationId: string): Promise<void>;
  open(page: 'repository' | 'releases'): Promise<void>;
}

export type DesktopView = {
  snapshot: Snapshot | null;
  connection: 'connecting' | 'connected' | 'reconnecting' | 'preview';
  error: string;
  starting: boolean;
  cancelling: string[];
};

class ResponseTimeout extends Error {
  constructor() {
    super(
      'The desktop has not confirmed this request. Check progress, then try again if needed.',
    );
  }
}

async function confirmed<T>(request: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      request,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new ResponseTimeout()), 5000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

export function errorMessage(error: unknown): string {
  if (
    error &&
    typeof error === 'object' &&
    'message' in error &&
    typeof error.message === 'string'
  )
    return error.message;
  return typeof error === 'string' ? error : 'The request failed. Try again.';
}

export function createDesktop(transport: Transport | null) {
  let view: DesktopView = {
    snapshot: null,
    connection: transport ? 'connecting' : 'preview',
    error: '',
    starting: false,
    cancelling: [],
  };
  const store = writable(view);
  const update = (patch: Partial<DesktopView>) => {
    view = { ...view, ...patch };
    store.set(view);
  };
  let generation = 0;
  let dispose: (() => void) | undefined;
  let lastMessage = 0;
  let stopped = false;
  let timer: ReturnType<typeof setInterval> | undefined;
  let pendingStart: { id: string; fail: boolean } | undefined;
  let connectionError = '';

  async function connect() {
    if (!transport || stopped) return;
    const current = ++generation;
    dispose?.();
    dispose = undefined;
    let session: string | null = null;
    let revision = -1n;
    lastMessage = Date.now();
    update({ connection: view.snapshot ? 'reconnecting' : 'connecting' });
    try {
      const close = await transport.watch((snapshot) => {
        if (
          stopped ||
          current !== generation ||
          !/^(0|[1-9][0-9]*)$/.test(snapshot.revision)
        )
          return;
        if (session !== null && snapshot.sessionId !== session) return;
        const next = BigInt(snapshot.revision);
        if (
          view.snapshot?.sessionId === snapshot.sessionId &&
          next < BigInt(view.snapshot.revision)
        )
          return;
        if (next < revision) return;
        session = snapshot.sessionId;
        lastMessage = Date.now();
        if (next > revision || view.connection !== 'connected')
          update({
            snapshot,
            connection: 'connected',
            error: view.error === connectionError ? '' : view.error,
          });
        connectionError = '';
        revision = next;
      });
      if (stopped || current !== generation) close();
      else dispose = close;
    } catch (error) {
      if (current === generation && !stopped) {
        connectionError = errorMessage(error);
        update({ connection: 'reconnecting', error: connectionError });
      }
    }
  }

  return {
    subscribe: store.subscribe,
    startWatching() {
      void connect();
      timer = setInterval(() => {
        if (Date.now() - lastMessage > 6000) void connect();
      }, 1000);
      return () => {
        stopped = true;
        generation++;
        clearInterval(timer);
        dispose?.();
      };
    },
    reconnect: connect,
    async start(fail = false) {
      if (!transport || view.starting || view.connection !== 'connected')
        return;
      update({ starting: true, error: '' });
      pendingStart ??= { id: crypto.randomUUID(), fail };
      try {
        await confirmed(transport.start(pendingStart.id, pendingStart.fail));
        pendingStart = undefined;
      } catch (error) {
        if (!(error instanceof ResponseTimeout)) pendingStart = undefined;
        if (!stopped) update({ error: errorMessage(error) });
      } finally {
        if (!stopped) update({ starting: false });
      }
    },
    async cancel(id: string) {
      if (
        !transport ||
        view.cancelling.includes(id) ||
        view.connection !== 'connected'
      )
        return;
      update({ cancelling: [...view.cancelling, id], error: '' });
      try {
        await confirmed(transport.cancel(id));
      } catch (error) {
        if (!stopped) update({ error: errorMessage(error) });
      } finally {
        if (!stopped)
          update({
            cancelling: view.cancelling.filter((entry) => entry !== id),
          });
      }
    },
    async open(page: 'repository' | 'releases') {
      if (!transport) {
        window.open(
          `https://github.com/Mastervoliumpl/Starframe${page === 'releases' ? '/releases' : ''}`,
          '_blank',
          'noopener,noreferrer',
        );
        return;
      }
      try {
        await confirmed(transport.open(page));
      } catch (error) {
        update({ error: errorMessage(error) });
      }
    },
  };
}
