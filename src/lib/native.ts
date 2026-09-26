import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import type { Snapshot } from './generated/model';
import type { Transport } from './state';
import type { AuthTransport } from './auth';

const native: Transport & AuthTransport = {
  authRestore: () => invoke('auth_restore'),
  authInspect: () => invoke('auth_inspect'),
  authStart: () => invoke('auth_start'),
  authPoll: () => invoke('auth_poll'),
  authSignOut: () => invoke('auth_sign_out'),
  authCancel: () => invoke('auth_cancel'),
  pickLocalSource: (folder) => invoke('pick_local_source', { folder }),
  saveCollection: (text) => invoke('save_collection_file', { text }),
  sharing: (action) => invoke('sharing_action', { action }),
  mods: (action) => invoke('mod_action', { action }),
  packages: (action) => invoke('package_action', { action }),
  retryRegistryReceipts: () => invoke('registry_retry_receipts'),
  async watch(receive) {
    const channel = new Channel<Snapshot>();
    channel.onmessage = receive;
    try {
      await invoke('watch_state', { channel });
    } catch (error) {
      channel.onmessage = () => {};
      throw error;
    }
    return () => {
      channel.onmessage = () => {};
    };
  },
  start: (requestId, fail) => invoke('start_diagnostic', { requestId, fail }),
  cancel: (operationId) => invoke('cancel_operation', { operationId }),
  open: (page) => invoke('open_external', { page }),
  game: (action) => invoke('game_action', { action }),
  update: (action) => invoke('update_action', { action }),
};

export async function getTransport(): Promise<
  (Transport & AuthTransport) | null
> {
  if (isTauri()) return native;
  if (
    import.meta.env.DEV &&
    new URLSearchParams(location.search).has('fixture')
  ) {
    return (await import('../fixtures/transport')).fixtureTransport();
  }
  return null;
}
