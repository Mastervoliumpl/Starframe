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
    catalog: {
      revision: '1',
      releaseCount: 0,
      checking: false,
      lastChecked: null,
      lastSuccess: null,
      error: null,
    },
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
  };
  const catalogCase = new URLSearchParams(location.search).get('catalog');
  if (catalogCase === 'offline') {
    snapshot.catalog.lastChecked = '1788820000';
    snapshot.catalog.lastSuccess = '1788819700';
    snapshot.catalog.error =
      'Fixture connection failed. Starframe will retry automatically.';
  } else if (catalogCase === 'update') {
    snapshot.catalog.checking = true;
  }
  let receiver: ((snapshot: Snapshot) => void) | undefined;
  let work: ReturnType<typeof setInterval>;
  const publish = () => receiver?.(structuredClone(snapshot));
  return {
    async watch(receive) {
      receiver = receive;
      publish();
      const heartbeat = setInterval(publish, 2000);
      const catalogUpdate =
        catalogCase === 'update'
          ? setTimeout(() => {
              snapshot.catalog.revision = '2';
              snapshot.catalog.checking = false;
              snapshot.revision = String(Number(snapshot.revision) + 1);
              publish();
            }, 1500)
          : undefined;
      return () => {
        clearInterval(heartbeat);
        clearTimeout(catalogUpdate);
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
    async game(action) {
      snapshot.game.busy = true;
      snapshot.revision = String(BigInt(snapshot.revision) + 1n);
      publish();
      await new Promise((resolve) => setTimeout(resolve, 200));
      snapshot.game.error = '';
      if (action.kind === 'discover') {
        snapshot.game.candidates = [
          {
            id: 'fixture-game',
            path: 'C:\\Fixture library\\Sanctuary',
            executable: 'C:\\Fixture library\\Sanctuary\\engine\\Sanctuary.exe',
            edition: 'Playtest fixture',
            build: 'Steam 123 · Unity fixture',
          },
        ];
        snapshot.game.message = 'Choose an installation to save its location.';
      } else if (action.kind === 'select') {
        const selected = snapshot.game.candidates.find(
          (item) => item.id === action.id,
        );
        if (selected) {
          snapshot.game.selected = selected;
          snapshot.game.selectedPath = selected.path;
          snapshot.game.running = 'stopped';
          snapshot.game.message = 'Fixture location selected.';
        }
      } else if (action.kind === 'setup') {
        snapshot.game.launch = {
          phase: 'ready',
          message: 'Fixture runtime prepared.',
          details: [],
        };
      } else if (action.kind === 'launch') {
        snapshot.game.launch = {
          phase: 'launch_requested',
          message:
            'Windows accepted the launch request. Waiting for the game process…',
          details: [],
        };
      } else if (action.kind === 'remove_runtime') {
        snapshot.game.launch = {
          phase: 'setup_required',
          message: 'Fixture runtime removed.',
          details: [],
        };
      } else {
        snapshot.game.error =
          'The native folder picker requires the desktop app.';
      }
      snapshot.game.busy = false;
      snapshot.revision = String(BigInt(snapshot.revision) + 1n);
      publish();
    },
  };
}
