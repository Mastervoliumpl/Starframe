import type { UpdateView } from '../lib/generated/updates';

export const emptyUpdates = (): UpdateView => ({
  channel: 'preview',
  phase: 'idle',
  release: null,
  dismissed: false,
  lastSuccess: null,
  message: 'Updates have not been checked in this session.',
  error: null,
  received: '0',
});
