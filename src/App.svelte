<script lang="ts">
  import { onMount, tick } from 'svelte';
  import mark from '../docs/design/starframe-mark.svg';
  import { version } from '../package.json';
  import DiagnosticList from './features/DiagnosticList.svelte';
  import GameSettings from './features/GameSettings.svelte';
  import LaunchBar from './features/LaunchBar.svelte';
  import ModList from './features/ModList.svelte';
  import Collections from './features/Collections.svelte';
  import CollectionSelect from './features/CollectionSelect.svelte';
  import PackageDownloads from './features/PackageDownloads.svelte';
  import { createManagement } from './lib/management';
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
  let manager = $state(createManagement(null));
  let drawer: HTMLDialogElement;
  let menu: HTMLButtonElement;
  let heading: HTMLHeadingElement;
  let fail = $state(false);
  const operations = $derived($desktop.snapshot?.operations ?? []);
  const catalog = $derived($desktop.snapshot?.catalog);
  const managementError = $derived(
    $desktop.snapshot?.savedData.status === 'unavailable' ? '' : $manager.error,
  );
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
    let stopManagement: (() => void) | undefined;
    void getTransport().then((transport) => {
      if (disposed) return;
      desktop = createDesktop(transport);
      manager = createManagement(transport);
      stopManagement = manager.start();
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
      stopManagement?.();
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
      <img src={mark} width="36" height="36" alt="" /><span>STARFRAME</span>
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
          <CollectionSelect
            {manager}
            unavailable={$desktop.connection !== 'connected'}
          />
          <p class="muted">Switches change the active collection.</p>
        </div>
        {#if managementError}<p class="error" role="alert">
            {managementError}<button onclick={() => manager.dismissError()}
              >Dismiss</button
            >
          </p>{/if}
        <ModList
          mode="mods"
          {manager}
          game={$desktop.snapshot?.game}
          unavailable={$desktop.connection !== 'connected'}
          onsource={(id) => desktop.open(`mod:${id}`)}
        />
      </section>
      <section class="page" hidden={page !== 'catalog'} aria-label="Catalog">
        <div class="catalog-status">
          <h2>Approved release catalog</h2>
          <p role="status" aria-live={page === 'catalog' ? 'polite' : 'off'}>
            {#if catalog?.checking}
              Checking for catalog changes…
            {:else if catalog?.revision}
              Catalog revision {catalog.revision}. {catalog.releaseCount} approved
              releases.
            {:else}
              No catalog is cached yet.
            {/if}
          </p>
          {#if catalog?.error}
            <p class="error" role="alert">{catalog.error}</p>
            {#if catalog.revision}<p>
                Cached revision {catalog.revision} remains available.
              </p>{/if}
          {/if}
          {#if catalog?.lastSuccess}
            <p class="muted">
              Last successful check: {new Date(
                Number(catalog.lastSuccess) * 1000,
              ).toLocaleString()}
            </p>
          {/if}
          <p>
            Downloads come from authors. Curation does not guarantee that a
            binary is free of malware.
          </p>
          <button onclick={() => desktop.open('repository')}
            >View Starframe on GitHub</button
          >
        </div>
        {#if managementError}<p class="error" role="alert">
            {managementError}<button onclick={() => manager.dismissError()}
              >Dismiss</button
            >
          </p>{/if}
        <ModList
          mode="catalog"
          {manager}
          game={$desktop.snapshot?.game}
          unavailable={$desktop.connection !== 'connected'}
          onsource={(id) => desktop.open(`mod:${id}`)}
        />
      </section>
      <section
        class="page"
        hidden={page !== 'collections'}
        aria-label="Collections"
      >
        <Collections
          {manager}
          onmods={() => navigate('mods')}
          unavailable={$desktop.connection !== 'connected'}
        />
      </section>
      <section
        class="page"
        hidden={page !== 'downloads'}
        aria-label="Downloads"
      >
        {#if managementError}<p class="error" role="alert">
            {managementError}<button onclick={() => manager.dismissError()}
              >Dismiss</button
            >
          </p>{/if}
        <PackageDownloads
          {manager}
          unavailable={$desktop.connection !== 'connected'}
        />
        <div class="toolbar">
          <p>Responsiveness diagnostics</p>
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
          <details>
            <summary>Share or repair a collection</summary>
            <p>
              In Collections, choose Export and save the collection file. On
              another installation, choose Import collection, select the file or
              paste its JSON, then review and accept once.
            </p>
            <p>
              Starframe verifies matching files and prepares missing approved
              releases. Import details keep unavailable references visible.
              Choose Use collection to enable the complete valid setup after the
              game closes.
            </p>
            <p>
              If an import stops, use Retry import. For a withdrawn release,
              changed identity or missing dependency, ask the sender for a
              repaired collection. You can also export the saved list, edit its
              exact references, and import it as a new collection. Newer
              releases are never selected automatically.
            </p>
          </details>
          <p>Log export is not available yet.</p>
          <details>
            <summary>Artwork credit</summary>
            <p>
              Sanctuary: Shattered Sun launch artwork belongs to Enhearten Media
              and its artists. Used with the permission supplied by the project
              owner: no claim of ownership or use for profit. This artwork is
              separate from Starframe’s AGPL code license.
            </p>
          </details>
          <button onclick={() => desktop.open('repository')}
            >Open Starframe on GitHub</button
          >
        </div>
      </section>
    </main>
    <LaunchBar
      game={$desktop.snapshot?.game}
      unavailable={$desktop.connection !== 'connected' || $desktop.gameRequest}
      collection={activeCollection}
      onsetup={() => navigate('settings')}
      onlaunch={() => desktop.game({ kind: 'launch' })}
    />
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
