<script lang="ts">
  import { tick } from 'svelte';
  import type { Management, SharingReply } from '../lib/management';
  let { manager, unavailable }: { manager: Management; unavailable: boolean } =
    $props();
  let dialog: HTMLDialogElement;
  let input = $state<HTMLTextAreaElement>(null!);
  let text = $state('');
  let error = $state('');
  let reply = $state<SharingReply | null>(null);
  let mode = $state<'import' | 'export'>('import');
  let requestId = '';
  let notice = $state('');
  let reading = $state(false);
  const busy = $derived(
    unavailable || reading || $manager.pending.includes('sharing'),
  );
  async function open() {
    mode = 'import';
    text = '';
    error = '';
    reply = null;
    requestId = crypto.randomUUID();
    manager.dismissError();
    dialog.showModal();
    await tick();
    input.focus();
  }
  export async function exportCollection(id: string) {
    mode = 'export';
    text = '';
    error = '';
    reply = null;
    manager.dismissError();
    dialog.showModal();
    const result = await manager.sharing({ kind: 'export', id });
    if (result && dialog.open) {
      reply = result;
      text = result.text ?? '';
    }
  }
  async function file(event: Event) {
    const target = event.currentTarget as HTMLInputElement;
    const selected = target.files?.[0];
    if (!selected) return;
    error = '';
    reply = null;
    if (selected.size > 1024 * 1024) {
      error = 'Collection files must be at most 1 MiB.';
      return;
    }
    reading = true;
    try {
      text = await selected.text();
    } catch {
      error = 'Could not read this collection file. Choose it again.';
    } finally {
      reading = false;
      target.value = '';
    }
  }
  async function review() {
    error = '';
    reply = await manager.sharing({ kind: 'review', text });
  }
  async function accept() {
    if (!reply || !$manager.data) return;
    const result = await manager.sharing({
      kind: 'accept',
      text,
      requestId,
      expectedRevision: $manager.data.revision,
    });
    if (result) {
      notice = `Imported ${result.name}. Exact packages are being prepared. Choose Use collection when you want to enable it.`;
      dialog.close();
    }
  }
  async function download() {
    if (await manager.saveCollection(text)) {
      notice = 'Collection file saved.';
      dialog.close();
    }
  }
</script>

<button disabled={busy} onclick={open}>Import collection</button>
{#if notice}<p role="status">{notice}</p>{/if}
<dialog
  class="collection-dialog"
  bind:this={dialog}
  aria-labelledby="sharing-title"
>
  <h2 id="sharing-title">
    {mode === 'export'
      ? 'Export collection'
      : reply
        ? `Import ${reply.name}?`
        : 'Import collection'}
  </h2>
  <p>
    Includes the collection name and exact mod order. Mod files, settings and
    local paths are excluded.
  </p>
  {#if mode === 'import' && !reply}
    <label for="collection-file">Choose a collection file</label>
    <input
      id="collection-file"
      type="file"
      accept=".json,application/json"
      onchange={file}
      disabled={busy}
    />
  {/if}
  {#if !reply || mode === 'export'}
    <label for="collection-json"
      >{mode === 'export'
        ? 'Collection JSON'
        : 'Or paste collection JSON'}</label
    >
    <textarea
      bind:this={input}
      id="collection-json"
      bind:value={text}
      readonly={mode === 'export'}
      disabled={busy}
      rows="9"
      maxlength="1048576"></textarea>
  {/if}
  {#if mode === 'import' && reply}
    <p>
      Accept once to create the collection and prepare missing approved
      releases. Unavailable entries stay in the list. No newer release is
      substituted.
    </p>
    <ol class="import-review">
      {#each reply.entries as entry (entry.reference.modId)}
        <li>
          <strong
            >{$manager.data?.catalog?.mods.find(
              (m) => m.id === entry.reference.modId,
            )?.name ?? entry.reference.modId}</strong
          >
          <p class="muted">
            {entry.reference.modId} · {entry.reference.releaseId ??
              'Local-only content'}
          </p>
          <p class:error={entry.status === 'unresolved'}>{entry.message}</p>
        </li>
      {/each}
    </ol>
    {#if !reply.entries.length}<p>This collection is empty.</p>{/if}
    {#if reply.orderError}<p class="error">
        {reply.orderError} The collection cannot apply until this is resolved.
      </p>{/if}
  {/if}
  {#if error || $manager.error}<p class="error" role="alert">
      {error || $manager.error}
    </p>{/if}
  <div class="dialog-actions">
    <button onclick={() => dialog.close()}>Close</button>
    {#if mode === 'export'}
      <button class="primary-action" disabled={busy || !text} onclick={download}
        >Save collection file</button
      >
    {:else if reply}
      <button
        disabled={busy}
        onclick={() => {
          reply = null;
          manager.dismissError();
        }}>Edit JSON</button
      >
      <button class="primary-action" disabled={busy} onclick={accept}
        >{busy ? 'Importing…' : 'Accept import'}</button
      >
    {:else}
      <button
        class="primary-action"
        disabled={busy || !text.trim()}
        onclick={review}>{busy ? 'Reading…' : 'Review import'}</button
      >
    {/if}
  </div>
</dialog>

<style>
  dialog {
    width: min(640px, calc(100% - 32px));
  }
  textarea,
  input {
    display: block;
    width: 100%;
    box-sizing: border-box;
    margin: 8px 0 20px;
  }
  textarea {
    color: var(--text);
    background: var(--canvas);
    border: 1px solid var(--outline);
    padding: 12px;
    resize: vertical;
    font: inherit;
  }
  .import-review {
    padding-left: 24px;
    overflow-wrap: anywhere;
  }
  .import-review li {
    margin-bottom: 20px;
  }
  .import-review p {
    margin: 6px 0;
  }
</style>
