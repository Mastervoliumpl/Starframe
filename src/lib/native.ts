import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import type { Snapshot } from './generated/model';
import type { Transport } from './state';

const native: Transport = {
  mods: (action) => invoke('mod_action', { action }),
  packages: (action) => invoke('package_action', { action }),
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
};

export async function getTransport(): Promise<Transport | null> {
  if (isTauri()) return native;
  if (
    import.meta.env.DEV &&
    new URLSearchParams(location.search).has('fixture')
  ) {
    return (await import('../fixtures/transport')).fixtureTransport();
  }
  return null;
}
