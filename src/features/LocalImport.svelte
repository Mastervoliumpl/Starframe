<script lang="ts">
  import type { Management } from '../lib/management';
  let { manager, unavailable }: { manager: Management; unavailable: boolean } =
    $props();
  let dialog: HTMLDialogElement;
  let trigger: HTMLButtonElement;
  let path = $state('');
  let accepted = $state(false);
  const busy = $derived(
    $manager.pending.includes('local-import') ||
      $manager.pending.includes('local-picker'),
  );
  async function choose(folder: boolean) {
    const selected = await manager.pickLocalSource(folder);
    if (selected) path = selected;
  }
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (await manager.importLocal(path)) {
      accepted = true;
      dialog.close();
    }
  }
</script>

<button
  bind:this={trigger}
  disabled={unavailable || !$manager.data}
  onclick={() => {
    manager.dismissError();
    accepted = false;
    dialog.showModal();
  }}>Import local mod</button
>
{#if accepted}<p role="status">
    Import started. Downloads shows progress and the result. Completed imports
    appear in My mods.
  </p>{/if}
<dialog
  bind:this={dialog}
  class="uninstall-dialog"
  aria-labelledby="local-import-title"
  onclose={() => trigger.focus()}
>
  <form onsubmit={submit}>
    <h2 id="local-import-title">Import local mod</h2>
    <p>
      Choose a managed DLL or a build output folder. Starframe copies its
      contents into your library. Your source files stay in place.
    </p>
    <p id="local-manifest-help">
      A folder needs starframe.local.json. A DLL needs a matching
      .starframe.json file beside it, such as Mod.starframe.json for Mod.dll.
      These files describe the mod and its entry point.
    </p>
    <div class="dialog-actions">
      <button
        type="button"
        disabled={busy || unavailable}
        onclick={() => choose(false)}>Choose DLL</button
      >
      <button
        type="button"
        disabled={busy || unavailable}
        onclick={() => choose(true)}>Choose folder</button
      >
    </div>
    <label class="field-label" for="local-source-path">Source path</label>
    <input
      id="local-source-path"
      type="text"
      bind:value={path}
      required
      maxlength="32768"
      aria-describedby="local-manifest-help"
    />
    <p>
      Starframe watches this source while open. Verified rebuilds update the
      active local collection; game files change after the game closes. Shared
      collections keep their exact builds. DLL hot reload is not supported.
    </p>
    {#if $manager.error}<p class="error" role="alert">{$manager.error}</p>{/if}
    {#if busy}<p role="status">
        {$manager.pending.includes('local-picker')
          ? 'Waiting for source selection…'
          : 'Starting the import…'}
      </p>{/if}
    <div class="dialog-actions">
      <button type="button" onclick={() => dialog.close()}>Cancel</button>
      <button
        class="primary-action"
        type="submit"
        disabled={busy || unavailable || !path.trim()}>Import copy</button
      >
    </div>
  </form>
</dialog>
