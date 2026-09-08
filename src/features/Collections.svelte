<script lang="ts">
  import { tick } from 'svelte';
  import type { Management, ModView } from '../lib/management';
  import LoadOrder from './LoadOrder.svelte';
  import CollectionSharing from './CollectionSharing.svelte';
  let sharing: CollectionSharing;
  let {
    manager,
    unavailable,
    onmods,
  }: { manager: Management; unavailable: boolean; onmods: () => void } =
    $props();
  let dialog = $state<HTMLDialogElement>(null!);
  let nameInput = $state<HTMLInputElement>(null!);
  let newButton: HTMLButtonElement;
  let trigger: HTMLElement | null = null;
  let mode = $state<'create' | 'rename' | 'delete'>('create');
  let editing = $state<ModView['collections'][number] | null>(null);
  let name = $state('');
  let formError = $state('');
  let expected = '';
  let notice = $state('');
  const busy = $derived(
    unavailable || !$manager.data || $manager.pending.includes('membership'),
  );
  const active = $derived(
    $manager.data?.collections.find(
      (c) => c.id === $manager.data?.activeCollection,
    ),
  );
  async function open(next: typeof mode, collection: typeof editing = null) {
    mode = next;
    editing = collection;
    name = collection?.name ?? '';
    expected = $manager.data?.revision ?? '';
    formError = '';
    manager.dismissError();
    trigger =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    dialog.showModal();
    await tick();
    if (mode !== 'delete') nameInput.focus();
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    if (mode !== 'delete' && !name.trim()) {
      formError = 'Enter a collection name.';
      nameInput.focus();
      return;
    }
    formError = '';
    const action =
      mode === 'create'
        ? { kind: 'create_collection' as const, name }
        : mode === 'rename'
          ? { kind: 'rename_collection' as const, id: editing!.id, name }
          : { kind: 'delete_collection' as const, id: editing!.id };
    if (await manager.collection(action, expected)) {
      notice =
        mode === 'delete'
          ? `Deleted ${editing?.name}. Mod files and settings are kept.`
          : mode === 'create'
            ? `Created ${name.trim()}. Choose Use collection to enable it.`
            : `Renamed collection to ${name.trim()}.`;
      dialog.close();
    }
  }
</script>

<div class="collections-heading">
  <div>
    <h2>Your collections</h2>
    <p class="muted">
      Each collection keeps a name and mod order. Mod files and settings are
      shared.
    </p>
  </div>
  <div class="collection-actions">
    <CollectionSharing bind:this={sharing} {manager} {unavailable} />
    <button
      bind:this={newButton}
      class="primary-action"
      disabled={busy}
      onclick={() => open('create')}>New collection</button
    >
  </div>
</div>
<p role="status">{notice}</p>
{#if $manager.error && !dialog?.open}<p class="error" role="alert">
    {$manager.error}<button onclick={() => manager.dismissError()}
      >Dismiss</button
    >
  </p>{/if}
{#if $manager.loading}<p role="status">Loading collections…</p>
{:else if !$manager.data}<p>Collection data is unavailable.</p>
{:else if !$manager.data.collections.length}<p>
    No collections yet. Create one, or enable a mod in My mods to start a
    default collection.
  </p>
{:else}
  <ul class="collection-grid" aria-label="Saved collections">
    {#each $manager.data.collections as collection (collection.id)}
      {@const imported = $manager.data.imports.find(
        (i) => i.collectionId === collection.id,
      )}
      <li>
        <h3>{collection.name}</h3>
        <p class="muted">
          {collection.entries.length}
          {collection.entries.length === 1 ? 'mod' : 'mods'}
        </p>
        <p class="collection-summary">
          {collection.entries
            .slice(0, 3)
            .map(
              (r) =>
                $manager.data?.library.find(
                  (e) =>
                    e.reference.modId === r.modId &&
                    e.reference.hash === r.hash,
                )?.name ?? `${r.modId} (unavailable)`,
            )
            .join(', ') || 'Empty collection'}{collection.entries.length > 3
            ? '…'
            : ''}
        </p>
        <button
          class:active-label={collection.id === active?.id}
          aria-disabled={busy || collection.id === active?.id}
          aria-pressed={collection.id === active?.id}
          aria-label={`Use collection ${collection.name}`}
          onclick={() => {
            if (!busy && collection.id !== active?.id)
              void manager.collection({
                kind: 'select_collection',
                id: collection.id,
              });
          }}
        >
          {#if collection.id === active?.id}<span aria-hidden="true"
              >✓
            </span>Active{:else}Use collection{/if}
        </button>
        <div class="collection-actions">
          <button
            disabled={unavailable || $manager.pending.includes('sharing')}
            aria-label={`Export collection ${collection.name}`}
            onclick={() => sharing.exportCollection(collection.id)}
            >Export</button
          >
          <button
            disabled={busy}
            aria-label={`Rename collection ${collection.name}`}
            onclick={() => open('rename', collection)}>Rename</button
          >
          <button
            disabled={busy}
            aria-label={`Delete collection ${collection.name}`}
            onclick={() => open('delete', collection)}>Delete</button
          >
        </div>
        {#if imported}
          <p role="status">
            {imported.entries.filter((e) => e.status === 'ready').length} of {imported
              .entries.length} exact packages ready
          </p>
          <details>
            <summary>Import details</summary>
            <ol>
              {#each imported.entries as entry (entry.reference.modId)}
                <li>
                  <strong>{entry.reference.modId}</strong>
                  <p class:error={entry.status === 'unresolved'}>
                    {entry.message}
                  </p>
                </li>
              {/each}
            </ol>
          </details>
          {#if !imported.entries.some((e) => e.status === 'pending' || e.status === 'preparing') && imported.entries.some((e) => e.status === 'unresolved')}
            <button
              disabled={unavailable || $manager.pending.includes('sharing')}
              onclick={() =>
                manager.sharing({ kind: 'retry', id: collection.id })}
              >Retry import</button
            >
          {/if}
        {/if}
      </li>
    {/each}
  </ul>
{/if}
{#if active}
  <div class="active-order">
    <button class="text-button" onclick={onmods}
      >Add or remove mods in My mods</button
    >
    {#key active.id}<LoadOrder
        {manager}
        name={active.name}
        {unavailable}
      />{/key}
  </div>
{:else if $manager.data}<button onclick={onmods}>Open My mods</button>{/if}

<dialog
  class="collection-dialog"
  bind:this={dialog}
  aria-labelledby="collection-dialog-title"
  onclose={() => {
    if (!trigger?.isConnected) newButton?.focus();
  }}
>
  <form onsubmit={save}>
    <h2 id="collection-dialog-title">
      {mode === 'create'
        ? 'New collection'
        : mode === 'rename'
          ? 'Rename collection'
          : `Delete ${editing?.name}?`}
    </h2>
    {#if mode === 'delete'}
      <p>
        Delete this saved name and mod order. Mod files and settings are kept.
      </p>
      {#if editing?.id === active?.id}<p>
          This is the active collection. Deleting it disables its mods after the
          game closes.
        </p>{/if}
    {:else}
      <label for="collection-name">Collection name</label>
      <input
        bind:this={nameInput}
        id="collection-name"
        type="text"
        bind:value={name}
        required
        maxlength="200"
        aria-invalid={!!formError}
        aria-describedby="collection-name-error"
        disabled={busy}
      />
      <p id="collection-name-error" class="error">{formError}</p>
    {/if}
    {#if $manager.error}<p class="error" role="alert">
        {$manager.error} Close this dialog and retry with the current collection.
      </p>{/if}
    <div class="dialog-actions">
      <button type="button" onclick={() => dialog.close()}>Cancel</button>
      <button class="primary-action" type="submit" disabled={busy}
        >{$manager.pending.includes('membership')
          ? 'Saving…'
          : mode === 'delete'
            ? 'Delete collection'
            : mode === 'create'
              ? 'Create collection'
              : 'Save name'}</button
      >
    </div>
  </form>
</dialog>

<style>
  .collections-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .collection-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 260px), 1fr));
    gap: 16px;
    padding: 0;
    list-style: none;
  }
  .collection-grid > li {
    border: 1px solid var(--selected);
    border-radius: 8px;
    padding: 20px;
    overflow-wrap: anywhere;
  }
  .collection-grid h3 {
    margin-top: 0;
  }
  .collection-summary {
    min-height: 2.5em;
  }
  .active-label {
    border-color: transparent;
    background: transparent;
    cursor: default;
  }
  .collection-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 12px;
  }
  .active-order {
    margin-top: 32px;
  }
  dialog {
    width: min(480px, calc(100% - 32px));
  }
  input {
    width: 100%;
    box-sizing: border-box;
    margin-top: 8px;
  }
</style>
