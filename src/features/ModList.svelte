<script lang="ts">
  import { tick } from 'svelte';
  import LocalImport from './LocalImport.svelte';
  import {
    compatibility,
    confirmedFinding,
    key,
    localKey,
    referenceRelease,
    type Management,
    type LibraryEntry,
  } from '../lib/management';
  import type { GameView } from '../lib/generated/game';
  let {
    manager,
    game,
    unavailable,
    onsource,
  }: {
    manager: Management;
    game: GameView | undefined;
    unavailable: boolean;
    onsource: (id: string) => void;
  } = $props();
  let search = $state('');
  let selected = $state<string[]>([]);
  let offset = $state(0);
  let detail = $state<string | null>(null);
  let detailHeading = $state<HTMLHeadingElement>();
  let trigger: HTMLButtonElement | undefined;
  let savedScroll = 0;
  let list: HTMLDivElement;
  let confirmation: HTMLDialogElement;
  let removing = $state<LibraryEntry | null>(null);
  let removeRevision = $state('');
  let removeCollections = $state<string[]>([]);
  type Row = {
    id: string;
    name: string;
    author: string;
    version: string;
    installed: LibraryEntry;
  };
  const rows = $derived.by((): Row[] => {
    const data = $manager.data;
    if (!data) return [];
    return data.library.map((installed) => ({
      id: key(installed.reference),
      name: installed.name,
      author: installed.author,
      version: installed.version,
      installed,
    }));
  });
  const filtered = $derived(
    rows.filter((r) =>
      `${r.name} ${r.author} ${r.version}`
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    ),
  );
  const visible = $derived(
    filtered.slice(
      Math.min(
        offset,
        Math.max(0, Math.floor((filtered.length - 1) / 100) * 100),
      ),
      offset + 100,
    ),
  );
  const chosen = $derived(rows.filter((r) => selected.includes(r.id)));
  const opened = $derived(rows.find((r) => r.id === detail));
  const local = $derived(
    $manager.data?.localSources.find(
      (source) =>
        opened?.installed &&
        localKey(source.reference) === key(opened.installed.reference),
    ),
  );
  let copyStatus = $state('');
  const watch = (row: Row) =>
    $manager.data?.localWatches.find(
      (w) =>
        row.installed &&
        localKey(w.source.reference) === key(row.installed.reference),
    );
  const busy = $derived(unavailable || $manager.pending.includes('membership'));
  const hash = (row: Row) => row.installed.reference.reference.sha256;
  const blocked = (row: Row) => confirmedFinding($manager.data, hash(row));
  const blockReason = (row: Row) => $manager.data?.blocked[hash(row) ?? ''];
  const selectedBlocked = $derived(chosen.some(blocked));
  const enabled = (row: Row) =>
    !!row.installed &&
    !!$manager.data?.enabled.some(
      (r) => key(r) === key(row.installed!.reference),
    );
  async function show(row: Row, event: MouseEvent) {
    savedScroll = list.closest('section')?.scrollTop ?? 0;
    copyStatus = '';
    detail = row.id;
    trigger = event.currentTarget as HTMLButtonElement;
    await tick();
    detailHeading?.focus();
  }
  async function back() {
    detail = null;
    await tick();
    const page = list.closest('section');
    if (page) page.scrollTop = savedScroll;
    trigger?.focus({ preventScroll: true });
  }
  function select(id: string, checked: boolean) {
    selected = checked ? [...selected, id] : selected.filter((s) => s !== id);
  }
  function confirm(row: Row) {
    if (!row.installed || !$manager.data) return;
    removing = row.installed;
    removeRevision = $manager.data.revision;
    removeCollections = $manager.data.collections
      .filter((c) =>
        c.entries.some((r) => key(r) === key(row.installed!.reference)),
      )
      .map((c) => c.name);
    confirmation.showModal();
  }
  async function uninstall() {
    if (removing && (await manager.uninstall(removing, removeRevision))) {
      confirmation.close();
      detail = null;
      await tick();
      list.focus();
    }
  }
</script>

<div class="mod-workspace" class:details-open={!!opened}>
  <div class="mod-browse" bind:this={list} tabindex="-1">
    <LocalImport {manager} {unavailable} />
    <label class="field-label" for="mods-search">Search installed mods</label>
    <input
      id="mods-search"
      type="search"
      bind:value={search}
      oninput={() => (offset = 0)}
    />
    <div class="bulk-actions">
      <label class="checkbox-label"
        ><input
          type="checkbox"
          aria-label="Select visible mods"
          checked={visible.length > 0 &&
            visible.every((r) => selected.includes(r.id))}
          onchange={(e) => {
            selected = e.currentTarget.checked
              ? [...new Set([...selected, ...visible.map((r) => r.id)])]
              : selected.filter((id) => !visible.some((r) => r.id === id));
          }}
        />{chosen.length} selected</label
      >
      <button
        disabled={busy || !chosen.length || selectedBlocked}
        aria-describedby={selectedBlocked
          ? 'mods-selected-security'
          : undefined}
        onclick={() =>
          manager.membership(
            chosen.flatMap((r) => (r.installed ? [r.installed] : [])),
            true,
          )}>Enable selected</button
      >
      <button
        disabled={busy || !chosen.length}
        onclick={() =>
          manager.membership(
            chosen.flatMap((r) => (r.installed ? [r.installed] : [])),
            false,
          )}>Remove from collection</button
      >
      {#if chosen.length}<button onclick={() => (selected = [])}
          >Clear selection</button
        >{:else}<span class="muted">Select rows to use bulk actions.</span>{/if}
    </div>
    {#if selectedBlocked}<p class="error" id="mods-selected-security">
        Some selected mods have retained registry security blocks. Review their
        details before changing the selection. You can still remove them from
        the collection.
      </p>{/if}
    {#if unavailable}<p class="muted">
        Management actions require a desktop connection.
      </p>{:else if busy}<p role="status">Saving collection changes…</p>{/if}
    {#if $manager.loading}<p role="status">Loading the saved library…</p>
    {:else if !rows.length}<div class="empty-state">
        <h2>No installed mods</h2>
        <p>
          Import a local mod or install a release from Mods, then enable it
          here.
        </p>
      </div>
    {:else if !filtered.length}<p class="empty-state">
        No matches. Change or clear your search.
      </p>
    {:else}
      <p class="result-count">
        {filtered.length}
        installed mods
      </p>
      <ul class="mod-list">
        {#each visible as row (row.id)}
          <li class:selected={selected.includes(row.id)}>
            <input
              type="checkbox"
              aria-label={`Select ${row.name} ${row.version}`}
              checked={selected.includes(row.id)}
              onchange={(e) => select(row.id, e.currentTarget.checked)}
            />
            <div class="mod-summary">
              <button class="mod-name" onclick={(e) => show(row, e)}
                >{row.name}</button
              >
              <p>
                {row.author} · {row.version} · {row.installed?.reference
                  .kind === 'local'
                  ? 'Local import'
                  : 'Registry release'}
              </p>
              {#if row.installed?.reference.kind === 'local'}
                <p class="technical">
                  Build {row.installed.reference.reference.sha256.slice(0, 8)} · {watch(
                    row,
                  )
                    ? 'Following source'
                    : 'Saved build'}
                </p>
                {#if watch(row)}<p
                    aria-live="polite"
                    class:error={watch(row)?.state === 'error'}
                  >
                    {watch(row)?.message}
                  </p>{/if}
              {/if}
              {#if row.installed?.reference.kind !== 'local'}<p
                  class="compatibility"
                >
                  {compatibility(
                    row.installed.testedGameBuild,
                    game?.selected?.build,
                  )}
                </p>{/if}
              {#if blockReason(row)}<p
                  class="error"
                  id={`mods-security-${encodeURIComponent(row.id)}`}
                >
                  {blockReason(row)} · Activation blocked
                </p>{/if}
            </div>
            <div class="mod-actions">
              <label class="enable-control"
                ><input
                  type="checkbox"
                  role="switch"
                  aria-label={`Enable ${row.name} ${row.version}`}
                  aria-describedby={blockReason(row)
                    ? `mods-security-${encodeURIComponent(row.id)}`
                    : undefined}
                  checked={enabled(row)}
                  disabled={busy || (!enabled(row) && blocked(row))}
                  onchange={(e) => {
                    const next = e.currentTarget.checked;
                    e.currentTarget.checked = enabled(row);
                    if (row.installed)
                      void manager.membership([row.installed], next);
                  }}
                /><span>Enabled</span></label
              >
              <button
                disabled={busy}
                onclick={() => confirm(row)}
                aria-label={`Uninstall ${row.name} ${row.version}`}
                >Uninstall</button
              >
            </div>
          </li>
        {/each}
      </ul>
      {#if filtered.length > 100}<div class="pagination">
          <button
            disabled={offset === 0}
            onclick={() => (offset = Math.max(0, offset - 100))}
            >Previous</button
          ><span
            >Page {Math.floor(offset / 100) + 1} of {Math.ceil(
              filtered.length / 100,
            )}</span
          ><button
            disabled={offset + 100 >= filtered.length}
            onclick={() => (offset += 100)}>Next</button
          >
        </div>{/if}
    {/if}
  </div>
  {#if opened}<aside class="mod-details" aria-label="Mod details">
      <button onclick={back}>Back to list</button>
      <h2 bind:this={detailHeading} tabindex="-1">{opened.name}</h2>
      <p>{opened.author} · {opened.version}</p>
      {#if blockReason(opened)}
        <h3>Registry security decision</h3>
        <p class="error">{blockReason(opened)}. Activation is blocked.</p>
      {/if}
      {#if opened.installed?.reference.kind === 'local'}
        <h3>Local source</h3>
        <p class="technical">
          {local?.path ??
            'Source metadata is unavailable. Import the exact build again.'}
        </p>
        {#if local}
          <div class="dialog-actions">
            <button
              onclick={async () => {
                try {
                  await navigator.clipboard.writeText(local.path);
                  copyStatus = 'Source path copied.';
                } catch {
                  copyStatus =
                    'Could not copy. Select and copy the path above.';
                }
              }}>Copy source path</button
            >
            <button
              onclick={() =>
                onsource(
                  `local:${local.reference.modId}:${local.reference.hash}`,
                )}>Open source folder</button
            >
          </div>
          {#if copyStatus}<p role="status">{copyStatus}</p>{/if}
        {/if}
        <p>
          Local imports use a managed copy. Starframe does not check them for
          registry updates or game-version compatibility.
        </p>
        <p>
          {watch(opened)?.message ??
            'This is a saved build. Import it again to follow this source.'}
        </p>
        <p>
          Only the latest explicitly imported source for each mod is watched.
          Rebuilds advance the matching active local collection entry; shared
          and inactive collections keep their exact builds. Older copies remain
          available.
        </p>
        <h3>Required builds</h3>
        <p>
          {local?.manifest.requires
            .map((r) => `${r.modId} (${r.releaseId ?? r.hash})`)
            .join(', ') || 'None declared'}
        </p>
      {:else}
        <p>Kind: {opened.installed.kind}</p>
        <button
          onclick={() => {
            const reference = opened.installed.reference;
            if (reference.kind === 'registry')
              onsource(`registry:${reference.reference.modId}`);
          }}>View author/source</button
        >
        <h3>Compatibility</h3>
        <p>
          {compatibility(
            opened.installed.testedGameBuild,
            game?.selected?.build,
          )}
        </p>
        <p>Selected game: {game?.selected?.build ?? 'Not selected'}</p>
        <p>
          Tested build: {opened.installed.testedGameBuild ??
            'No test evidence recorded'}
        </p>
        <p>
          Compatibility warnings allow you to enable and try the mod. Mods run
          at your own risk.
        </p>
        <p>
          View release and dependency information in Mods. Install required
          releases before enabling this mod.
        </p>
      {/if}
      <h3>Installation</h3>
      <p>
        {opened.installed
          ? enabled(opened)
            ? 'Enabled in the saved collection. Game readiness is shown below.'
            : 'Installed in the library and disabled.'
          : 'Not installed.'}
      </p>
      {#if opened.installed?.reference.kind !== 'local'}<p>
          Approval applies to the reviewed archive bytes. It does not guarantee
          that a mod is free of malware.
        </p>{/if}
      <details>
        <summary>Package details</summary>
        <p class="technical">
          Release: {referenceRelease(opened.installed.reference)}<br />SHA-256: {opened
            .installed.reference.reference.sha256}
        </p>
      </details>
    </aside>{/if}
</div>
<dialog
  bind:this={confirmation}
  class="uninstall-dialog"
  aria-labelledby="mods-uninstall-title"
>
  <h2 id="mods-uninstall-title">Uninstall {removing?.name}?</h2>
  <p>
    Remove version {removing?.version} from your library. Settings are kept. Game
    files change only after the game closes.
  </p>
  {#if removing?.reference.kind === 'local'}<p>
      Your source DLL, source folder and local metadata are kept.
    </p>{/if}
  <p>
    Affected collections: {removeCollections.join(', ') || 'None'}. Other
    collections retain an unresolved reference.
  </p>
  {#if $manager.error}<p class="error" role="alert">{$manager.error}</p>{/if}
  <div class="dialog-actions">
    <button onclick={() => confirmation.close()}>Cancel</button><button
      disabled={busy}
      onclick={uninstall}>Confirm uninstall</button
    >
  </div>
</dialog>
