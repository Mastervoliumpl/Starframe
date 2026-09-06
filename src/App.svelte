<script lang="ts">
  import { onMount, tick } from 'svelte';
  import mark from '../docs/design/starframe-mark.svg';
  import { version } from '../package.json';
  import DiagnosticList from './features/DiagnosticList.svelte';
  import GameSettings from './features/GameSettings.svelte';
  import { createDesktop } from './lib/state';
  import { getTransport } from './lib/native';

  const pages = [
    { id: 'mods', label: 'My mods' },
    { id: 'catalog', label: 'Catalog' },
    { id: 'collections', label: 'Collections' },
    { id: 'downloads', label: 'Downloads' },
    { id: 'settings', label: 'Settings' },
    { id: 'help', label: 'Help & logs' },
  ] as const;
  type Page = (typeof pages)[number]['id'];
  let page = $state<Page>('mods');
  let desktop = $state(createDesktop(null));
  let drawer: HTMLDialogElement;
  let menu: HTMLButtonElement;
  let heading: HTMLHeadingElement;
  let fail = $state(false);
  const operations = $derived($desktop.snapshot?.operations ?? []);
  const activeCollection = $derived(
    $desktop.snapshot?.savedData.status === 'ready'
      ? ($desktop.snapshot.savedData.activeCollectionName ?? 'None')
      : 'Unavailable',
  );
  const active = $derived(
    operations.some(
      (op) => op.status === 'running' || op.status === 'cancelling',
    ),
  );
  const finished = $derived(operations.length > 0 && !active);

  async function navigate(destination: Page) {
    page = destination;
    drawer?.close();
    await tick();
    heading.focus();
  }

  function keepDrawerFocus(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    const buttons = drawer.querySelectorAll<HTMLButtonElement>('button');
    const first = buttons[0];
    const last = buttons[buttons.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  onMount(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void getTransport().then((transport) => {
      if (disposed) return;
      desktop = createDesktop(transport);
      stop = desktop.startWatching();
    });
    const wide = matchMedia('(min-width: 900px)');
    const closeDrawer = () => {
      if (wide.matches && drawer.open) {
        drawer.close();
        heading.focus();
      }
    };
    wide.addEventListener('change', closeDrawer);
    return () => {
      disposed = true;
      stop?.();
      wide.removeEventListener('change', closeDrawer);
    };
  });
</script>

{#snippet navigation()}
  <nav aria-label="Main navigation">
    {#each pages as destination (destination.id)}
      <button
        aria-current={page === destination.id ? 'page' : undefined}
        onclick={() => navigate(destination.id)}>{destination.label}</button
      >
    {/each}
  </nav>
{/snippet}

<a class="skip-link" href="#workspace">Skip to content</a>
<div class="app-shell">
  <aside class="sidebar" aria-label="Starframe">
    <div class="brand">
      <img src={mark} width="36" height="36" alt="" /><span>Starframe</span>
    </div>
    {@render navigation()}
    <p class="build-version">Development build<br />{version}</p>
  </aside>
  <div class="work-area">
    <header class="page-header">
      <button
        class="menu-button"
        bind:this={menu}
        onclick={() => drawer.showModal()}
        aria-haspopup="dialog">Menu</button
      >
      <div>
        <p class="eyebrow">Sanctuary: Shattered Sun</p>
        <h1 bind:this={heading} tabindex="-1">
          {pages.find((entry) => entry.id === page)?.label}
        </h1>
      </div>
      <p class="connection" role="status">
        {$desktop.connection === 'connected'
          ? 'Desktop connected'
          : $desktop.connection === 'preview'
            ? 'Browser preview · native actions unavailable'
            : $desktop.connection === 'connecting'
              ? 'Connecting…'
              : 'Reconnecting…'}
      </p>
    </header>
    <main id="workspace" tabindex="-1">
      {#if $desktop.snapshot?.savedData.status === 'unavailable'}
        <p class="error" role="alert">
          Saved data is unavailable. {$desktop.snapshot.savedData.message} No empty
          library has replaced it. Close Starframe before attempting recovery.
        </p>
      {/if}
      {#if $desktop.error}<p class="error" role="alert">
          {$desktop.error}
        </p>{/if}
      <section class="page" hidden={page !== 'mods'} aria-label="My mods">
        <div class="toolbar">
          <p>Active collection: <strong>{activeCollection}</strong></p>
          <p class="muted">Library management coming later</p>
        </div>
        <div class="empty-state">
          <h2>Saved library</h2>
          <p>Mod management is not available in this build.</p>
          <button onclick={() => navigate('catalog')}>Browse catalog</button>
        </div>
      </section>
      <section class="page" hidden={page !== 'catalog'} aria-label="Catalog">
        <div class="empty-state">
          <h2>The catalog is not connected yet</h2>
          <p>
            Approved releases will appear here. Downloads will come from their
            authors. Curation does not guarantee that a binary is free of
            malware.
          </p>
          <button onclick={() => desktop.open('repository')}
            >View Starframe on GitHub</button
          >
        </div>
      </section>
      <section
        class="page"
        hidden={page !== 'collections'}
        aria-label="Collections"
      >
        <div class="empty-state">
          <h2>Saved collections</h2>
          <p>
            A collection will hold a name and an ordered list of mods.
            Collection editing is not available in this build.
          </p>
        </div>
      </section>
      <section
        class="page"
        hidden={page !== 'downloads'}
        aria-label="Downloads"
      >
        <div class="toolbar">
          <p>Operations</p>
          <p class="muted">
            {$desktop.connection !== 'connected'
              ? 'Actions wait for the desktop connection.'
              : 'Progress is confirmed by the desktop.'}
          </p>
        </div>
        {#if operations.length}
          <ul class="operations">
            {#each operations as operation (operation.id)}
              <li>
                <div class="operation-heading">
                  <h2>{operation.label}</h2>
                  <span>{operation.status}</span>
                </div>
                <p>{operation.message}</p>
                <div class="progress-row">
                  <progress
                    aria-label={operation.label}
                    max="100"
                    value={operation.progress}
                  ></progress><span>{operation.progress}%</span>
                </div>
                {#if operation.status === 'running' || operation.status === 'cancelling'}
                  <button
                    disabled={operation.status === 'cancelling' ||
                      $desktop.cancelling.includes(operation.id) ||
                      $desktop.connection !== 'connected'}
                    onclick={() => desktop.cancel(operation.id)}
                    >{operation.status === 'cancelling' ||
                    $desktop.cancelling.includes(operation.id)
                      ? 'Cancelling…'
                      : 'Cancel'}</button
                  >
                {/if}
              </li>
            {/each}
          </ul>
        {:else}
          <div class="empty-state">
            <h2>No operations</h2>
            <p>
              Run the responsiveness check in Help & logs to inspect progress
              and cancellation.
            </p>
            <button onclick={() => navigate('help')}>Open Help & logs</button>
          </div>
        {/if}
      </section>
      <section class="page" hidden={page !== 'settings'} aria-label="Settings">
        <div class="settings-section">
          <h2>Saved data</h2>
          {#if $desktop.snapshot?.savedData.status === 'ready'}
            <p>
              Saved locally: {$desktop.snapshot.savedData.libraryCount} library entries
              and {$desktop.snapshot.savedData.collectionCount} collections.
            </p>
          {:else if $desktop.snapshot?.savedData.status === 'unavailable'}
            <p>
              Saved data could not be opened. The recovery message identifies
              the data folder.
            </p>
          {:else}<p>
              {$desktop.connection === 'preview'
                ? 'Saved data requires the desktop app.'
                : 'Opening saved data…'}
            </p>{/if}
        </div>
        <GameSettings
          game={$desktop.snapshot?.game}
          unavailable={$desktop.connection !== 'connected' ||
            $desktop.gameRequest}
          onaction={(action) => desktop.game(action)}
        />
        <div class="settings-section">
          <h2>Starframe {version}</h2>
          <p>Automatic update checks are not available yet.</p>
          <button onclick={() => desktop.open('releases')}
            >View releases in browser</button
          >
        </div>
      </section>
      <section class="page" hidden={page !== 'help'} aria-label="Help and logs">
        <div class="settings-section">
          <h2>Responsiveness check</h2>
          <p>
            Run three simulated transfers while a background worker hashes
            memory. This check does not download mods or change game files.
          </p>
          <div class="diagnostic-controls">
            <label class="checkbox-label"
              ><input
                type="checkbox"
                bind:checked={fail}
                disabled={active || $desktop.starting}
              />Simulate a failure</label
            ><button
              disabled={active ||
                $desktop.starting ||
                $desktop.connection !== 'connected'}
              onclick={() => desktop.start(fail)}
              >{$desktop.starting
                ? 'Starting…'
                : 'Run responsiveness check'}</button
            ><button onclick={() => navigate('downloads')}>View progress</button
            >
          </div>
          {#if active}<p class="muted">
              A check is running. Cancel individual workers in Downloads.
            </p>{:else if $desktop.connection !== 'connected'}<p class="muted">
              The check requires a desktop connection.
            </p>{/if}
          <details>
            <summary>Connection details</summary>
            <p class="technical">
              Session: {$desktop.snapshot?.sessionId ?? 'Unavailable'}<br
              />Revision: {$desktop.snapshot?.revision ?? 'Unavailable'}
            </p>
            <button
              disabled={$desktop.connection === 'preview'}
              onclick={() => desktop.reconnect()}
              >Reconnect state subscription</button
            >
          </details>
        </div>
        <DiagnosticList />
        <div class="settings-section">
          <h2>Project help</h2>
          <p>Log export is not available yet.</p>
          <button onclick={() => desktop.open('repository')}
            >Open Starframe on GitHub</button
          >
        </div>
      </section>
    </main>
    <footer class="launch-footer">
      <div>
        <strong>Active collection: {activeCollection}</strong>
        <p id="launch-reason">
          {$desktop.snapshot?.game.running === 'running'
            ? 'Game running. '
            : $desktop.snapshot?.game.selectedPath &&
                $desktop.snapshot.game.running === 'unknown'
              ? 'Game state unknown. '
              : ''}Setup and launch are not available in this build.
        </p>
        {#if active}<button
            class="text-button"
            onclick={() => navigate('downloads')}
            >Diagnostic running · View progress</button
          >{/if}
      </div>
      <button class="launch-button" disabled aria-describedby="launch-reason"
        >Launch Sanctuary Shattered Sun</button
      >
    </footer>
  </div>
</div>
<p class="sr-only" role="status">
  {finished ? 'Responsiveness check finished. See Downloads for results.' : ''}
</p>
<dialog
  class="navigation-drawer"
  bind:this={drawer}
  aria-label="Navigation"
  onkeydown={keepDrawerFocus}
>
  <button class="close-menu" onclick={() => drawer.close()}>Close menu</button>
  {@render navigation()}
</dialog>
