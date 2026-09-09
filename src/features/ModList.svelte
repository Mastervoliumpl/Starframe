<script lang="ts">
  import { tick } from 'svelte';
  import LocalImport from './LocalImport.svelte';
  import {
    bytes,
    compatibility,
    findings,
    confirmedFinding,
    key,
    transferring,
    type Management,
    type LibraryEntry,
    type CatalogMod,
    type Release,
  } from '../lib/management';
  import type { GameView } from '../lib/generated/game';
  let {
    mode,
    manager,
    game,
    unavailable,
    catalogFresh,
    onsource,
  }: {
    mode: 'mods' | 'catalog';
    manager: Management;
    game: GameView | undefined;
    unavailable: boolean;
    catalogFresh: boolean;
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
    mod?: CatalogMod;
    release?: Release;
    installed?: LibraryEntry;
  };
  const rows = $derived.by((): Row[] => {
    const data = $manager.data;
    if (!data) return [];
    if (mode === 'catalog')
      return (data.catalog?.mods ?? []).flatMap((mod) =>
        mod.releases.map((release) => ({
          id: release.id,
          name: mod.name,
          author: mod.author,
          version: release.version,
          mod,
          release,
          installed: data.library.find(
            (e) =>
              e.reference.modId === mod.id &&
              e.reference.hash === release.artifact.sha256 &&
              e.reference.origin === 'catalog' &&
              e.reference.releaseId === release.id,
          ),
        })),
      );
    return data.library.map((installed) => {
      const mod =
        installed.reference.origin === 'catalog'
          ? data.catalog?.mods.find((m) => m.id === installed.reference.modId)
          : undefined;
      return {
        id: key(installed.reference),
        name: installed.name,
        author: installed.author,
        version: installed.version,
        installed,
        mod,
        release: mod?.releases.find(
          (r) => r.id === installed.reference.releaseId,
        ),
      };
    });
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
        key(source.reference) === key(opened.installed.reference),
    ),
  );
  let copyStatus = $state('');
  const watch = (row: Row) =>
    $manager.data?.localWatches.find(
      (w) =>
        row.installed &&
        key(w.source.reference) === key(row.installed.reference),
    );
  const busy = $derived(unavailable || $manager.pending.includes('membership'));
  const hash = (row: Row) =>
    row.installed?.reference.hash ?? row.release?.artifact.sha256;
  const security = (row: Row) => findings($manager.data, hash(row));
  const blocked = (row: Row) => confirmedFinding($manager.data, hash(row));
  const suspected = (row: Row) =>
    security(row).some(
      (advisory) => advisory.history.at(-1)?.state === 'suspected',
    );
  const selectedBlocked = $derived(chosen.some(blocked));
  const enabled = (row: Row) =>
    !!row.installed &&
    !!$manager.data?.enabled.some(
      (r) => key(r) === key(row.installed!.reference),
    );
  const operation = (row: Row) =>
    $manager.operations.find(
      (o) => o.releaseId === row.release?.id && transferring(o),
    );
  function reason(row: Row) {
    if (unavailable) return 'Waiting for the desktop connection.';
    if (blocked(row))
      return 'Blocked by a confirmed security finding. Review the mod details.';
    if (row.installed)
      return 'Installed in your library. Enable it in My mods.';
    if (row.release?.withdrawn)
      return `Withdrawn: ${row.release.withdrawalReason ?? 'The catalog does not provide a reason.'}`;
    if (!row.release) return 'Release metadata is unavailable.';
    if (
      !catalogFresh &&
      !$manager.data?.library.some(
        (entry) => entry.reference.hash === hash(row),
      )
    )
      return 'Catalog refresh required before downloading.';
    if (
      operation(row) ||
      $manager.pending.includes(`install:${row.release.id}`)
    )
      return 'Preparing the package. See Downloads for progress.';
    return '';
  }
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
    {#if mode === 'mods'}<LocalImport {manager} {unavailable} />{/if}
    <label class="field-label" for={`${mode}-search`}
      >Search {mode === 'mods' ? 'installed mods' : 'catalog releases'}</label
    >
    <input
      id={`${mode}-search`}
      type="search"
      bind:value={search}
      oninput={() => (offset = 0)}
    />
    <div class="bulk-actions">
      <label class="checkbox-label"
        ><input
          type="checkbox"
          aria-label={`Select visible ${mode === 'mods' ? 'mods' : 'releases'}`}
          checked={visible.length > 0 &&
            visible.every((r) => selected.includes(r.id))}
          onchange={(e) => {
            selected = e.currentTarget.checked
              ? [...new Set([...selected, ...visible.map((r) => r.id)])]
              : selected.filter((id) => !visible.some((r) => r.id === id));
          }}
        />{chosen.length} selected</label
      >
      {#if mode === 'mods'}
        <button
          disabled={busy || !chosen.length || selectedBlocked}
          aria-describedby={selectedBlocked
            ? `${mode}-selected-security`
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
      {:else}
        <button
          disabled={unavailable || !chosen.some((r) => !reason(r))}
          onclick={async () => {
            for (const row of chosen)
              if (!reason(row) && row.release)
                if (!(await manager.install(row.release.id))) break;
          }}>Install selected</button
        >
      {/if}
      {#if chosen.length}<button onclick={() => (selected = [])}
          >Clear selection</button
        >{:else}<span class="muted">Select rows to use bulk actions.</span>{/if}
    </div>
    {#if mode === 'mods' && selectedBlocked}<p
        class="error"
        id={`${mode}-selected-security`}
      >
        Some selected mods have confirmed security findings. Review their
        details before changing the selection. You can still remove them from
        the collection.
      </p>{/if}
    {#if unavailable}<p class="muted">
        Management actions require a desktop connection.
      </p>{:else if busy}<p role="status">Saving collection changes…</p>{/if}
    {#if $manager.loading}<p role="status">
        Loading the saved library and catalog…
      </p>
    {:else if !rows.length}<div class="empty-state">
        <h2>
          {mode === 'mods' ? 'No installed mods' : 'No approved releases yet'}
        </h2>
        <p>
          {mode === 'mods'
            ? 'Import a local mod or install a release from Catalog, then enable it here.'
            : 'Approved releases appear after the maintainer updates the catalog. Starframe checks automatically while open.'}
        </p>
      </div>
    {:else if !filtered.length}<p class="empty-state">
        No matches. Change or clear your search.
      </p>
    {:else}
      <p class="result-count">
        {filtered.length}
        {mode === 'mods' ? 'installed mods' : 'releases'}
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
                  .origin === 'local_import'
                  ? 'Local import'
                  : 'Catalog release'}
              </p>
              {#if row.installed?.reference.origin === 'local_import'}
                <p class="technical">
                  Build {row.installed.reference.hash.slice(0, 8)} · {watch(row)
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
              {#if row.installed?.reference.origin !== 'local_import'}<p
                  class="compatibility"
                >
                  {compatibility(row.release, game?.selected?.build)}
                </p>{/if}
              {#if row.mod?.unmaintained}<p>Unmaintained</p>{/if}
              {#if security(row).length}<p
                  class:error={blocked(row)}
                  id={`${mode}-security-${encodeURIComponent(row.id)}`}
                >
                  {blocked(row)
                    ? 'Confirmed security finding · activation blocked'
                    : suspected(row)
                      ? 'Unconfirmed security finding · review details'
                      : 'Previous security finding cleared'}
                </p>{/if}
              {#if row.release?.withdrawn}<p>
                  Withdrawn · {row.release.withdrawalReason ??
                    'Reason not supplied in the catalog.'}{row.installed
                    ? blocked(row)
                      ? ' A security finding blocks this copy.'
                      : ' Installed copy remains usable.'
                    : ''}
                </p>{/if}
              {#if operation(row)}<p>
                  {operation(row)?.message}
                  {bytes(operation(row)!.receivedBytes)} / {bytes(
                    operation(row)!.totalBytes,
                  )}
                </p>{/if}
            </div>
            <div class="mod-actions">
              {#if mode === 'mods'}
                <label class="enable-control"
                  ><input
                    type="checkbox"
                    role="switch"
                    aria-label={`Enable ${row.name} ${row.version}`}
                    aria-describedby={security(row).length
                      ? `${mode}-security-${encodeURIComponent(row.id)}`
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
              {:else}
                <button
                  disabled={!!reason(row)}
                  title={reason(row) ||
                    'Download and verify this exact release into your library.'}
                  onclick={() => row.release && manager.install(row.release.id)}
                  >{row.installed
                    ? 'Installed'
                    : operation(row)
                      ? 'Preparing…'
                      : 'Install'}</button
                >
                {#if reason(row) && !row.release?.withdrawn}<small
                    >{reason(row)}</small
                  >{/if}
              {/if}
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
      {#if security(opened).length}
        <h3>Security findings</h3>
        {#each security(opened) as advisory (advisory.id)}
          {@const current = advisory.history.at(-1)!}
          <h4>{advisory.title}</h4>
          <p class:error={current.state === 'confirmed'}>
            {current.state === 'confirmed'
              ? 'Confirmed · downloads and activation blocked'
              : current.state === 'suspected'
                ? 'Unconfirmed · use is permitted'
                : 'Cleared · this finding no longer blocks use'}
          </p>
          <p>{current.explanation}</p>
          <p>{current.recommendedAction}</p>
          <p>Library files and settings are retained.</p>
          <details>
            <summary>Evidence and correction history</summary>
            {#each advisory.history as finding, historyIndex (finding.recordedAt)}
              <p>
                {new Date(finding.recordedAt * 1000).toLocaleString()} · {finding.state}
              </p>
              <p>{finding.explanation}</p>
              <p>{finding.recommendedAction}</p>
              {#each finding.evidence as url, evidenceIndex (url)}
                <button
                  onclick={() =>
                    onsource(
                      `advisory:${advisory.id}:${historyIndex}:${evidenceIndex}`,
                    )}>Open evidence {evidenceIndex + 1}</button
                >
                <p class="technical">{url}</p>
              {/each}
            {/each}
          </details>
        {/each}
      {/if}
      {#if opened.installed?.reference.origin === 'local_import'}
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
          catalog updates or game-version compatibility.
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
        <p>{opened.mod?.description || 'No description supplied.'}</p>
        {#if opened.mod}<button
            onclick={() => onsource(`mod:${opened.mod!.id}`)}
            >View author/source</button
          >{/if}
        <h3>Compatibility</h3>
        <p>{compatibility(opened.release, game?.selected?.build)}</p>
        <p>Selected game: {game?.selected?.build ?? 'Not selected'}</p>
        <p>
          Tested builds: {opened.release?.testedGameBuilds.join(', ') ||
            'No test evidence recorded'}
        </p>
        {#each opened.release?.compatibilityProblems ?? [] as problem, index (index)}<p
          >
            {problem.gameBuild}: {problem.note}
          </p>
          <p class="technical">Evidence: {problem.sourceUrl}</p>{/each}
        <p>
          Compatibility warnings allow you to enable and try the mod. Mods run
          at your own risk.
        </p>
        {#if opened.mod?.unmaintained}<p>
            Unmaintained. Installed copies remain usable.
          </p>{/if}
        {#if opened.release?.withdrawn}<p>
            Withdrawn: {opened.release.withdrawalReason ??
              'Reason not supplied in the catalog.'} Installed copies and settings
            are retained.
          </p>{/if}
        <h3>Required releases</h3>
        <p>{opened.release?.requires.join(', ') || 'None declared'}</p>
        <p>Install required releases from Catalog before enabling this mod.</p>
      {/if}
      <h3>Installation</h3>
      <p>
        {opened.installed
          ? enabled(opened)
            ? 'Enabled in the saved collection. Game readiness is shown below.'
            : 'Installed in the library and disabled.'
          : 'Not installed.'}
      </p>
      {#if mode === 'catalog' && opened.release}<button
          class="primary-action"
          disabled={!!reason(opened)}
          onclick={() => opened.release && manager.install(opened.release.id)}
          >{opened.installed ? 'Installed' : 'Install release'}</button
        >
        <p>{reason(opened)}</p>{/if}
      {#if opened.installed?.reference.origin !== 'local_import'}<p>
          Approval applies to the reviewed archive bytes. It does not guarantee
          that a mod is free of malware.
        </p>{/if}
      <details>
        <summary>Package details</summary>
        <p class="technical">
          Release: {opened.release?.id ??
            opened.installed?.reference.releaseId ??
            'Local import'}<br />SHA-256: {opened.release?.artifact.sha256 ??
            opened.installed?.reference.hash}
        </p>
        {#if opened.release}<p>
            Download: {bytes(opened.release?.artifact.sizeBytes ?? 0)}. Space
            checks also budget up to 2 GiB for extraction per active transfer
            and a 64 MiB margin. Deployment checks space for recovery copies.
          </p>{/if}
      </details>
    </aside>{/if}
</div>
<dialog
  bind:this={confirmation}
  class="uninstall-dialog"
  aria-labelledby={`${mode}-uninstall-title`}
>
  <h2 id={`${mode}-uninstall-title`}>Uninstall {removing?.name}?</h2>
  <p>
    Remove version {removing?.version} from your library. Settings are kept. Game
    files change only after the game closes.
  </p>
  {#if removing?.reference.origin === 'local_import'}<p>
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
