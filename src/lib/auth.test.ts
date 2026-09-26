import { get } from 'svelte/store';
import { afterEach, expect, test, vi } from 'vitest';
import { createAuth, type AuthTransport } from './auth';

afterEach(() => vi.useRealTimers());

const session = {
  accountId: '33333333-3333-4333-8333-333333333333',
  profile: { displayName: 'Fixture user', avatarUrl: null },
  context: 'manager' as const,
  authenticatedAt: '2026-09-24T00:00:00Z',
  expiresAt: '2026-10-24T00:00:00Z',
  capabilities: ['download_mod'],
  isOwner: false,
};

function transport(): AuthTransport {
  return {
    authRestore: vi.fn(async () => null),
    authInspect: vi.fn(async () => session),
    authStart: vi.fn(async () => ({
      displayCode: 'A1B2C3D4',
      verificationUri: 'https://starframemanager.com/sign-in?manager=fixture',
      expiresAt: '2026-10-24T00:00:00Z',
      intervalSeconds: 5,
    })),
    authPoll: vi
      .fn<() => Promise<'waiting' | 'signed_in'>>()
      .mockResolvedValueOnce('waiting')
      .mockResolvedValueOnce('signed_in'),
    authSignOut: vi.fn(async () => ({ serverRevoked: false })),
    authCancel: vi.fn(async () => {}),
  };
}

test('polls at the server interval and handles local-only sign-out', async () => {
  vi.useFakeTimers();
  const api = transport();
  const auth = createAuth(api);
  await auth.start();
  expect(get(auth).challenge?.displayCode).toBe('A1B2C3D4');
  expect(api.authPoll).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(4999);
  expect(api.authPoll).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  expect(api.authPoll).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(5000);
  expect(api.authPoll).toHaveBeenCalledTimes(2);
  expect(get(auth).session?.profile.displayName).toBe('Fixture user');
  await auth.signOut();
  expect(get(auth).session).toBeNull();
  expect(get(auth).message).toContain('could not confirm revocation');
});

test('cancelling a challenge stops future polls', async () => {
  vi.useFakeTimers();
  const api = transport();
  const auth = createAuth(api);
  await auth.start();
  await auth.cancel();
  await vi.advanceTimersByTimeAsync(10000);
  expect(api.authPoll).not.toHaveBeenCalled();
  expect(api.authCancel).toHaveBeenCalledTimes(1);
  expect(get(auth).status).toBe('signed_out');
});
