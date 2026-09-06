<script lang="ts">
  import type { GameView } from '../lib/generated/game';
  import type { GameAction } from '../lib/generated/model';
  let {
    game,
    unavailable,
    onaction,
  }: {
    game: GameView | undefined;
    unavailable: boolean;
    onaction: (action: GameAction) => void;
  } = $props();
  const candidates = $derived(
    game?.candidates.filter((item) => item.path !== game.selected?.path) ?? [],
  );
  function displayPath(path: string) {
    return path.startsWith('\\\\?\\UNC\\')
      ? `\\\\${path.slice(8)}`
      : path.replace(/^\\\\\?\\/, '');
  }
</script>

<div class="settings-section game-settings">
  <h2>Game location</h2>
  <p>
    Locate Sanctuary to see its build and running state. Setup and launch are
    not available in this build.
  </p>
  <div class="game-actions">
    <button
      disabled={unavailable || game?.busy}
      onclick={() => onaction({ kind: 'discover' })}>Find in Steam</button
    >
    <button
      disabled={unavailable || game?.busy}
      onclick={() => onaction({ kind: 'choose_folder' })}
      >Choose game folder</button
    >
  </div>
  <p role="status">
    {game?.busy
      ? 'Checking game location…'
      : (game?.message ?? 'Game discovery requires the desktop app.')}
  </p>
  {#if game?.error}<p class="error" role="alert">{game.error}</p>{/if}
  {#if game?.selected}
    <h3>Selected installation</h3>
    <dl>
      <dt>Edition</dt>
      <dd>{game.selected.edition}</dd>
      <dt>Build</dt>
      <dd>{game.selected.build}</dd>
      <dt>Folder</dt>
      <dd>{displayPath(game.selected.path)}</dd>
      <dt>Game state</dt>
      <dd>
        {game.running === 'running'
          ? 'Game running'
          : game.running === 'stopped'
            ? 'Game not running'
            : 'Game state unknown'}
      </dd>
    </dl>
  {:else if game?.selectedPath}
    <h3>Saved location unavailable</h3>
    <p>{displayPath(game.selectedPath)}</p>
    <p>Game state unknown. The saved location is retained.</p>
  {/if}
  {#if candidates.length}
    <h3>Steam search results</h3>
    <ul class="game-candidates">
      {#each candidates as item (item.id)}
        <li>
          <strong>{item.edition}</strong>
          <p>{item.build}</p>
          <p>{displayPath(item.path)}</p>
          <button
            disabled={unavailable || game?.busy}
            aria-label={`Use installation: ${item.edition}, ${displayPath(item.path)}`}
            onclick={() => onaction({ kind: 'select', id: item.id })}
            >Use installation</button
          >
        </li>
      {/each}
    </ul>
  {/if}
</div>
