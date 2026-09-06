import type { Snapshot } from '../lib/generated/model';
import type { Transport } from '../lib/state';
import { version } from '../../package.json';

// Browser tests opt into this fixture with ?fixture; production builds omit it.
export function fixtureTransport(): Transport {
  let snapshot: Snapshot = {
    sessionId: 'browser-fixture',
    revision: '0',
    appVersion: version,
    operations: [],
  };
  let receiver: ((snapshot: Snapshot) => void) | undefined;
  let work: ReturnType<typeof setInterval>;
  const publish = () => receiver?.(structuredClone(snapshot));
  return {
    async watch(receive) {
      receiver = receive;
      publish();
      const heartbeat = setInterval(publish, 2000);
      return () => {
        clearInterval(heartbeat);
        if (receiver === receive) receiver = undefined;
      };
    },
    async start(requestId, fail) {
      if (
        snapshot.operations.some(
          (op) => op.status === 'running' || op.status === 'cancelling',
        )
      )
        throw new Error('A diagnostic is already running.');
      snapshot.operations = Array.from({ length: 3 }, (_, index) => ({
        id: `fixture-${index}`,
        requestId,
        label: `Diagnostic worker ${index + 1}`,
        progress: 0,
        status: 'running' as const,
        message: 'Browser fixture; no native work.',
      }));
      snapshot.revision = String(BigInt(snapshot.revision) + 1n);
      publish();
      work = setInterval(() => {
        for (const op of snapshot.operations) {
          if (op.status === 'cancelling') {
            op.status = 'cancelled';
            op.message = 'Diagnostic cancelled.';
          }
          if (op.status !== 'running') continue;
          op.progress++;
          if (fail && op.progress === 50) {
            op.status = 'failed';
            op.message =
              'Requested diagnostic failure. Run another check to retry.';
          } else if (op.progress === 100) {
            op.status = 'completed';
            op.message = 'Diagnostic completed.';
          }
        }
        snapshot = {
          ...snapshot,
          revision: String(BigInt(snapshot.revision) + 1n),
        };
        publish();
        if (
          !snapshot.operations.some(
            (op) => op.status === 'running' || op.status === 'cancelling',
          )
        )
          clearInterval(work);
      }, 100);
      return snapshot.operations.map((op) => op.id);
    },
    async cancel(id) {
      const op = snapshot.operations.find((op) => op.id === id);
      if (op?.status === 'running') op.status = 'cancelling';
    },
    async open() {},
  };
}
