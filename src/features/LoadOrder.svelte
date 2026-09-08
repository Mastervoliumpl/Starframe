<script lang="ts">
  import { tick } from 'svelte';
  import type { Management } from '../lib/management';
  let {
    manager,
    name,
    unavailable,
  }: { manager: Management; name: string; unavailable: boolean } = $props();
  let dragged = $state<string | null>(null);
  let picked = $state<string | null>(null);
  let message = $state('');
  const order = $derived($manager.data?.order);
  const entries = $derived(order?.effective ?? $manager.data?.enabled ?? []);
  const busy = $derived(
    unavailable || $manager.pending.includes('membership') || !order,
  );
  const label = (id: string) =>
    $manager.data?.library.find((e) => e.reference.modId === id)?.name ?? id;
  async function move(id: string, target: number) {
    if (busy || target < 0 || target >= entries.length) return;
    const ids = entries.map((e) => e.modId);
    const from = ids.indexOf(id);
    if (from < 0 || from === target) return;
    picked = null;
    ids.splice(from, 1);
    ids.splice(target, 0, id);
    const focused = document.activeElement;
    message = 'Saving load priority…';
    const saved = await manager.reorder(ids);
    await tick();
    if (
      focused instanceof HTMLElement &&
      focused.isConnected &&
      !focused.closest('[hidden]') &&
      document.activeElement === document.body
    ) {
      focused.focus({ preventScroll: true });
    }
    message = saved
      ? `Priority saved for ${label(id)}. The list shows the effective load order.`
      : 'Load priority was not saved.';
  }
  function pick(id: string, index: number) {
    if (busy) return;
    if (picked && picked !== id) void move(picked, index);
    else {
      picked = picked === id ? null : id;
      message = picked
        ? `${label(id)} selected. Choose another handle to place it there. Escape cancels.`
        : 'Reordering cancelled.';
    }
  }
</script>

<h2>{name}: load order</h2>
<p id="order-help">
  {#if order}
    Mods run in the numbered order below. Drag a handle to reorder. Required
    constraints can adjust your priority.
  {:else}
    The list retains your requested priority. Resolve the reported problem to
    determine the effective load order.
  {/if}
</p>
<p id="order-keyboard" class="muted order-keyboard">
  On a handle, use the arrow keys to move. You can also select a handle, then
  select another to place it there. Escape cancels the selection.
</p>
<p class="muted">
  Changes apply automatically when the game is closed. Lua overlays use the
  later mod's file when paths collide. Conventional BepInEx plugins and maps are
  unsupported.
</p>
{#if $manager.error}<p class="error" role="alert">
    {$manager.error}<button onclick={() => manager.dismissError()}
      >Dismiss</button
    >
  </p>{/if}
{#if $manager.data?.orderError}<p class="error" role="alert">
    {$manager.data.orderError}
  </p>{/if}
{#if unavailable}<p class="muted">
    Reordering is unavailable until the desktop connects.
  </p>{/if}
<p role="status">
  {$manager.pending.includes('membership') ? 'Saving collection…' : message}
</p>
{#if $manager.loading}
  <p role="status">Loading the active collection…</p>
{:else if !$manager.data}
  <p>Collection data is unavailable.</p>
{:else if !entries.length}
  <div class="empty-state">
    <h3>No enabled mods</h3>
    <p>Enable a mod in My mods to add it to this collection.</p>
  </div>
{:else}
  <ol
    class="load-order"
    aria-label={order
      ? 'Effective load order'
      : 'Requested priority (unresolved)'}
    aria-describedby="order-help"
    aria-busy={$manager.pending.includes('membership')}
  >
    {#each entries as entry, index (entry.modId)}
      <li class:picked={picked === entry.modId}>
        <div class="order-row">
          <button
            class="drag-handle"
            draggable={!busy}
            aria-disabled={busy}
            aria-label={`Reorder ${label(entry.modId)}`}
            aria-describedby="order-keyboard"
            aria-pressed={picked === entry.modId}
            title="Drag to reorder; arrow keys move; select to pick up or place"
            ondragstart={(event) => {
              if (busy) {
                event.preventDefault();
                return;
              }
              dragged = entry.modId;
              picked = null;
              event.dataTransfer?.setData('text/plain', entry.modId);
            }}
            ondragend={() => (dragged = null)}
            ondragover={(event) => {
              if (dragged && !busy) event.preventDefault();
            }}
            ondrop={(event) => {
              event.preventDefault();
              if (dragged) void move(dragged, index);
              dragged = null;
            }}
            onclick={() => pick(entry.modId, index)}
            onkeydown={(event) => {
              if (event.key === 'Escape') {
                picked = null;
                message = 'Reordering cancelled.';
              } else if (
                ['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)
              ) {
                event.preventDefault();
                const target =
                  event.key === 'Home'
                    ? 0
                    : event.key === 'End'
                      ? entries.length - 1
                      : index + (event.key === 'ArrowUp' ? -1 : 1);
                void move(entry.modId, target);
              }
            }}><span aria-hidden="true">⠿</span></button
          >
          <div class="order-description">
            <strong>{label(entry.modId)}</strong>
            <p class="muted">
              Requested position {($manager.data?.enabled.findIndex(
                (e) => e.modId === entry.modId,
              ) ?? index) + 1}
            </p>
            {#each order?.adjustments.filter((note) => note.before === entry.modId || note.after === entry.modId) ?? [] as note (note.message)}<p
              >
                {note.message}
              </p>{/each}
          </div>
          <span class="order-number" aria-hidden="true">{index + 1}</span>
        </div>
      </li>
    {/each}
  </ol>
{/if}

{#if $manager.data?.collisions.length}
  <h3>Lua file collisions</h3>
  <p>
    Winners below follow the effective order. A runtime failure can prevent an
    overlay from loading; check the launch status after the game starts.
  </p>
  <ul>
    {#each $manager.data.collisions as collision (collision.path)}
      <li>
        <code>{collision.path}</code>: {collision.mods.map(label).join(', ')}.
        Winner: <strong>{label(collision.winner)}</strong>.
      </li>
    {/each}
  </ul>
{/if}

<style>
  .load-order {
    padding-left: 0;
    list-style: none;
  }
  li {
    padding: 12px 0;
    border-bottom: 1px solid var(--selected);
  }
  .order-number {
    font-variant-numeric: tabular-nums;
    font-size: 1.25rem;
    color: var(--body);
    text-align: right;
    min-width: 2ch;
  }
  .order-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
  }
  .order-description {
    overflow-wrap: anywhere;
  }
  .order-description p {
    margin: 4px 0;
  }
  .drag-handle {
    cursor: grab;
    background: transparent;
    border-color: transparent;
  }
  .drag-handle:hover,
  .picked .drag-handle {
    border-color: var(--body);
  }
  .picked {
    background: var(--selected);
  }
  .order-keyboard {
    display: none;
  }
  :global(.page:focus-within) .order-keyboard {
    display: block;
  }
  [aria-disabled='true'] {
    cursor: default;
  }
</style>
