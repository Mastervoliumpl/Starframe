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
  const operationPending = $derived(
    game?.busy ||
      game?.launch.phase === 'launch_requested' ||
      game?.launch.phase === 'preparing',
  );
  const canWrite = $derived(
    !unavailable &&
      !operationPending &&
      game?.selected &&
      game.running === 'stopped',
  );
  const candidates = $derived(
    game?.candidates.filter((item) => item.path !== game.selected?.path) ?? [],
  );
  function displayPath(path: string) {
    return path.startsWith('\\\\?\\UNC\\')
      ? `\\\\${path.slice(8)}`
      : path.replace(/^\\\\\?\\/, '');
  }
  let selectedHeading = $state<HTMLHeadingElement>();
  let selectedFromResult = $state<string | null>(null);
  $effect(() => {
    if (
      selectedFromResult &&
      game?.selected?.path === selectedFromResult &&
      !game.busy
    ) {
      selectedFromResult = null;
      if (selectedHeading?.checkVisibility()) selectedHeading.focus();
    }
  });
</script>

<div class="settings-section game-settings">
  <h2>Game location</h2>
  <p>Choose the Sanctuary installation you want Starframe to manage.</p>
  <div class="game-actions">
    <button
      disabled={unavailable || operationPending}
      onclick={() => onaction({ kind: 'discover' })}>Find in Steam</button
    >
    <button
      disabled={unavailable || operationPending}
      onclick={() => onaction({ kind: 'choose_folder' })}
      >Choose game folder</button
    >
  </div>
  <p role="status">
    {game?.busy
      ? 'Working on the game setup…'
      : (game?.message ?? 'Game discovery requires the desktop app.')}
  </p>
  {#if game?.error}<p class="error" role="alert">{game.error}</p>{/if}
  {#if game?.selected}
    <h3 bind:this={selectedHeading} tabindex="-1">Selected installation</h3>
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
            disabled={unavailable || operationPending}
            aria-label={`Use installation: ${item.edition}, ${displayPath(item.path)}`}
            onclick={() => {
              selectedFromResult = item.path;
              onaction({ kind: 'select', id: item.id });
            }}>Use installation</button
          >
        </li>
      {/each}
    </ul>
  {/if}
</div>

<div class="settings-section game-settings">
  <h2>Game runtime</h2>
  <p>
    Starframe installs and updates its game runtime automatically while the game
    is closed. Enable installed releases in My mods; saved changes apply
    automatically too.
  </p>
  <p>{game?.launch.message ?? 'Runtime setup requires the desktop app.'}</p>
  {#if game?.launch.details.length}
    <ul>
      {#each game.launch.details as detail (detail)}<li>{detail}</li>{/each}
    </ul>
  {/if}
  <div class="game-actions">
    <button disabled={!canWrite} onclick={() => onaction({ kind: 'setup' })}
      >Repair or reinstall runtime</button
    >
  </div>
  <p class="muted">
    Close the game before repair. Repair restores missing runtime files and
    keeps your mods and settings. Files changed outside Starframe are retained
    for inspection; any repair error appears here.
  </p>
  <p class="muted">
    Uninstalling Starframe through Windows also removes its game integration.
    The uninstaller lets you choose whether to keep your library, collections
    and app settings. Original mod source folders and unowned game settings are
    kept.
  </p>
</div>
