<script lang="ts">
  import { tick } from 'svelte';
  import type { createAuth } from '../lib/auth';

  let {
    auth,
    unavailable,
  }: { auth: ReturnType<typeof createAuth>; unavailable: boolean } = $props();

  let signInButton = $state<HTMLButtonElement>();
  let cancelButton = $state<HTMLButtonElement>();

  async function begin() {
    await auth.start();
    await tick();
    cancelButton?.focus();
  }

  async function cancel() {
    await auth.cancel();
    await tick();
    signInButton?.focus();
  }

  async function signOut() {
    await auth.signOut();
    await tick();
    signInButton?.focus();
  }
</script>

<div
  class="settings-section"
  aria-busy={$auth.busy || $auth.status === 'checking'}
>
  <h2>Online mods</h2>
  {#if $auth.status === 'checking'}
    <p>Checking the saved manager session…</p>
  {:else if $auth.status === 'signed_in' && $auth.session}
    <p>Signed in as <strong>{$auth.session.profile.displayName}</strong>.</p>
    {#if !$auth.session.capabilities.includes('download_mod')}
      <p>Online downloads are unavailable for this account.</p>
    {/if}
    <div class="diagnostic-controls">
      <button
        disabled={$auth.busy || unavailable}
        onclick={() => auth.inspect()}>Check session</button
      >
      <button disabled={$auth.busy} onclick={signOut}
        >{$auth.busy ? 'Signing out…' : 'Sign out'}</button
      >
    </div>
  {:else if $auth.status === 'waiting' && $auth.challenge}
    <p>Confirm this code in the browser opened by Starframe:</p>
    <p><strong class="technical">{$auth.challenge.displayCode}</strong></p>
    <p>
      The code expires after ten minutes. Starframe checks for confirmation
      every five seconds.
    </p>
    <button bind:this={cancelButton} onclick={cancel}>Cancel sign-in</button>
  {:else}
    <p>
      Sign in with Steam through the Starframe website to browse and download
      approved mods.
    </p>
    <button
      bind:this={signInButton}
      disabled={unavailable || $auth.busy}
      onclick={begin}
      >{$auth.busy ? 'Opening browser…' : 'Sign in with Steam'}</button
    >
  {/if}
  {#if unavailable}
    <p>
      The website connection requires the desktop app. Installed mods, local
      imports and game launching remain available.
    </p>
  {/if}
  <p role="status" aria-live="polite">{$auth.message}</p>
</div>
