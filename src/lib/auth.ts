import { writable } from 'svelte/store';
import { errorMessage } from './state';

export type AuthSession = {
  accountId: string;
  profile: { displayName: string; avatarUrl: string | null };
  context: 'manager';
  authenticatedAt: string;
  expiresAt: string;
  capabilities: string[];
  isOwner: boolean;
};

export type AuthChallenge = {
  displayCode: string;
  verificationUri: string;
  expiresAt: string;
  intervalSeconds: number;
};

export interface AuthTransport {
  authRestore(): Promise<AuthSession | null>;
  authInspect(): Promise<AuthSession | null>;
  authStart(): Promise<AuthChallenge>;
  authPoll(): Promise<'waiting' | 'signed_in'>;
  authSignOut(): Promise<{ serverRevoked: boolean }>;
  authCancel(): Promise<void>;
}

export type AuthView = {
  status: 'signed_out' | 'checking' | 'waiting' | 'signed_in';
  session: AuthSession | null;
  challenge: AuthChallenge | null;
  busy: boolean;
  message: string;
};

export function createAuth(transport: AuthTransport | null) {
  let view: AuthView = {
    status: transport ? 'checking' : 'signed_out',
    session: null,
    challenge: null,
    busy: false,
    message: transport ? '' : 'Sign-in requires the desktop app.',
  };
  const store = writable(view);
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const update = (patch: Partial<AuthView>) => {
    view = { ...view, ...patch };
    store.set(view);
  };
  const clearTimer = () => {
    clearTimeout(timer);
    timer = undefined;
  };

  async function restore() {
    if (!transport) return;
    const current = ++generation;
    update({ status: 'checking', message: '' });
    try {
      const session = await transport.authRestore();
      if (current !== generation) return;
      update({
        status: session ? 'signed_in' : 'signed_out',
        session,
        message: '',
      });
    } catch (error) {
      if (current !== generation) return;
      update({ status: 'signed_out', message: errorMessage(error) });
    }
  }

  async function inspect() {
    if (!transport || view.busy) return;
    const current = ++generation;
    update({ busy: true, message: '' });
    try {
      const session = await transport.authInspect();
      if (current !== generation) return;
      update({
        status: session ? 'signed_in' : 'signed_out',
        session,
        challenge: null,
        busy: false,
        message: session
          ? ''
          : 'The manager session ended. Sign in again for online mods.',
      });
    } catch (error) {
      if (current !== generation) return;
      update({ busy: false, message: errorMessage(error) });
    }
  }

  function schedulePoll(current: number, seconds: number) {
    timer = setTimeout(() => void poll(current, seconds), seconds * 1000);
  }

  async function poll(current: number, seconds: number) {
    if (!transport || current !== generation) return;
    try {
      const result = await transport.authPoll();
      if (current !== generation) return;
      if (result === 'waiting') {
        schedulePoll(current, seconds);
        return;
      }
      const session = await transport.authInspect();
      if (current !== generation) return;
      update({
        status: session ? 'signed_in' : 'signed_out',
        session,
        challenge: null,
        message: session
          ? 'Signed in for online mods.'
          : 'The manager session could not be confirmed. Sign in again.',
      });
    } catch (error) {
      if (current !== generation) return;
      update({
        status: 'signed_out',
        challenge: null,
        message: errorMessage(error),
      });
    }
  }

  async function start() {
    if (!transport || view.busy) return;
    clearTimer();
    const current = ++generation;
    update({ busy: true, message: '' });
    try {
      const challenge = await transport.authStart();
      if (current !== generation) return;
      update({ status: 'waiting', challenge, busy: false });
      schedulePoll(current, challenge.intervalSeconds);
    } catch (error) {
      if (current !== generation) return;
      update({ busy: false, message: errorMessage(error) });
    }
  }

  async function cancel() {
    if (!transport) return;
    clearTimer();
    ++generation;
    update({ status: 'signed_out', challenge: null, busy: false, message: '' });
    try {
      await transport.authCancel();
    } catch (error) {
      update({ message: errorMessage(error) });
    }
  }

  async function signOut() {
    if (!transport || view.busy) return;
    clearTimer();
    const current = ++generation;
    update({ busy: true, message: '' });
    try {
      const result = await transport.authSignOut();
      if (current !== generation) return;
      update({
        status: 'signed_out',
        session: null,
        challenge: null,
        busy: false,
        message: result.serverRevoked
          ? 'Signed out.'
          : 'Signed out on this PC. The website could not confirm revocation.',
      });
    } catch (error) {
      if (current !== generation) return;
      update({ busy: false, message: errorMessage(error) });
    }
  }

  return {
    subscribe: store.subscribe,
    restore,
    inspect,
    start,
    cancel,
    signOut,
    stop: clearTimer,
  };
}
