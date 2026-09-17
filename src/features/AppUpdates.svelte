<script lang="ts">
  import type {
    UpdateAction,
    UpdateChannel,
    UpdateView,
  } from '../lib/generated/updates';
  let {
    view,
    version,
    unavailable,
    onaction,
  }: {
    view?: UpdateView;
    version: string;
    unavailable: boolean;
    onaction: (action: UpdateAction) => Promise<void>;
  } = $props();
  let pending = $state(false);
  const busy = $derived(view?.phase !== 'idle');
  async function act(action: UpdateAction) {
    if (pending) return;
    pending = true;
    try {
      await onaction(action);
    } finally {
      pending = false;
    }
  }
</script>

<div class="settings-section">
  <h2>Starframe {version}</h2>
  <label for="update-channel">Update channel</label>
  <select
    id="update-channel"
    value={view?.channel ?? 'stable'}
    disabled={unavailable || pending || (busy && view?.phase !== 'checking')}
    onchange={(event) =>
      act({
        kind: 'channel',
        channel: event.currentTarget.value as UpdateChannel,
      })}
  >
    <option value="stable">Stable releases</option>
    <option value="preview">Preview and stable releases</option>
  </select>
  <p class="muted">
    Preview releases may be unfinished. Changing channels never downgrades the
    installed app.
  </p>
  <p role="status">{view?.message || 'Updates require the desktop app.'}</p>
  {#if view?.lastSuccess}
    <p class="muted">
      Last successful check: {new Date(
        Number(view.lastSuccess) * 1000,
      ).toLocaleString()}
    </p>
  {/if}
  {#if view?.error}<p class="error" role="alert">{view.error}</p>{/if}
  <button
    disabled={unavailable || pending || busy}
    onclick={() => act({ kind: 'check' })}
  >
    {view?.phase === 'checking' ? 'Checking…' : 'Check for updates'}
  </button>
  {#if view?.release}
    {@const release = view.release}
    <h3>Update available: {release.version}</h3>
    <p>
      Update closes Starframe and reopens it after installation. Your library,
      collections and settings are kept. Installation waits for the game and
      active file work to finish.
    </p>
    <details>
      <summary>Release notes</summary>
      <div class="release-notes">
        {release.notes || 'No release notes were supplied.'}
      </div>
    </details>
    {#if view.phase === 'downloading'}<p>
        Downloaded {Math.round(Number(view.received || '0') / 1024 / 1024)} MB
      </p>{/if}
    <div class="actions">
      <button
        class="primary-action"
        disabled={unavailable || pending || busy}
        onclick={() => act({ kind: 'install', version: release.version })}
        >Update</button
      >
      <button
        disabled={unavailable || pending}
        onclick={() => act({ kind: 'view_release' })}>View release</button
      >
      {#if view.phase === 'downloading' || view.phase === 'waiting'}
        <button
          disabled={unavailable || pending}
          onclick={() => act({ kind: 'cancel' })}>Cancel update</button
        >
      {:else}
        <button
          disabled={unavailable || pending || busy || view.dismissed}
          onclick={() => act({ kind: 'later', version: release.version })}
          >Later</button
        >
      {/if}
    </div>
  {/if}
</div>

<style>
  select {
    display: block;
    margin-top: 8px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-top: 16px;
  }
  .release-notes {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    margin-block: 12px;
  }
  details {
    margin-block: 16px;
  }
</style>
