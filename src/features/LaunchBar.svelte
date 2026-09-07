<script lang="ts">
  import artwork from '../assets/sanctuary-sphere.jpg';
  import type { GameView } from '../lib/generated/game';
  let {
    game,
    unavailable,
    collection,
    onsetup,
    onlaunch,
  }: {
    game: GameView | undefined;
    unavailable: boolean;
    collection: string;
    onsetup: () => void;
    onlaunch: () => void;
  } = $props();
  const needsSetup = $derived(
    game?.launch.phase === 'setup_required' || game?.launch.phase === 'failed',
  );
  const pending = $derived(
    game?.busy || game?.launch.phase === 'launch_requested',
  );
  const reason = $derived(
    game?.launch.phase === 'preparing' ||
      game?.launch.phase === 'launch_requested'
      ? game.launch.message
      : game?.selectedPath && game.running === 'unknown'
        ? 'Game state unknown. Setup and launch wait until it can be checked.'
        : (game?.launch.message ?? 'Launch requires the desktop app.'),
  );
</script>

<footer class="launch-footer">
  <div>
    <strong>Active collection: {collection}</strong>
    <p id="launch-reason" role="status">{reason}</p>
    {#if game?.launch.phase === 'failed' || game?.launch.phase === 'runtime_failed' || game?.launch.details.length}
      <button class="text-button" onclick={onsetup}>View issues</button>
    {/if}
  </div>
  <button
    class="launch-button"
    disabled={unavailable ||
      pending ||
      (!needsSetup &&
        (game?.launch.phase !== 'ready' || game.running !== 'stopped'))}
    aria-describedby="launch-reason"
    onclick={needsSetup ? onsetup : onlaunch}
  >
    <img
      class="launch-art"
      src={artwork}
      alt=""
      decoding="async"
      fetchpriority="low"
    />
    <span class="launch-tint" aria-hidden="true"></span>
    <span class="launch-label"
      >{needsSetup ? 'Finish setup' : 'Launch Sanctuary Shattered Sun'}</span
    >
  </button>
</footer>
