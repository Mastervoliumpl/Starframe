<script lang="ts">
  import { tick } from 'svelte';
  import {
    exact,
    mediaUrl,
    releaseKey,
    sizes,
    type Registry,
  } from '../lib/registry';
  import { bytes, key, transferring, type Management } from '../lib/management';
  import type { Release } from '../lib/generated/registry';
  let {
    registry,
    manager,
    active,
    onsettings,
    ondownloads,
    gameBuild,
    onsource,
  }: {
    registry: Registry;
    manager: Management;
    active: boolean;
    onsettings: () => void;
    ondownloads: () => void;
    gameBuild: string | undefined;
    onsource: (modId: number) => void;
  } = $props();
  let search = $state('');
  let filters = $state(false);
  let list = $state<HTMLDivElement>();
  let detailHeading = $state<HTMLHeadingElement>();
  let trigger: HTMLButtonElement | undefined;
  let savedScroll = 0;
  let gallery: HTMLDialogElement;
  let galleryTrigger: HTMLButtonElement | undefined;
  let preview = $state<string | null>(null);
  let failedImages = $state<string[]>([]);
  let accepted = $state<string | null>(null);
  const account = $derived($registry.account);
  const groups = $derived([
    ...new Set($registry.options?.tags.map((tag) => tag.groupName) ?? []),
  ]);
  const listing = $derived(
    $registry.detail && 'listing' in $registry.detail ? $registry.detail : null,
  );
  const release = $derived(
    $registry.release && 'artifact' in $registry.release
      ? $registry.release
      : null,
  );
  const installed = $derived(
    release
      ? ($manager.data?.library.some(
          (entry) => key(entry.reference) === key(exact(release)),
        ) ?? false)
      : false,
  );
  const installedVersions = $derived(
    $manager.data?.library.filter(
      (entry) =>
        entry.reference.kind === 'registry' &&
        entry.reference.reference.modId === $registry.selected,
    ) ?? [],
  );
  const pending = $derived(
    release
      ? $registry.installing.includes(releaseKey(release)) ||
          $manager.operations.some(
            (op) => op.releaseId === release.releaseId && transferring(op),
          )
      : false,
  );
  const filterCount = $derived(
    $registry.query.includeTags.length +
      $registry.query.excludeTags.length +
      $registry.query.gameBuilds.length +
      Number($registry.query.maintenance !== 'all') +
      Number($registry.query.modType !== 'all'),
  );
  $effect(() => {
    if (active && account) void registry.browse();
  });

  async function open(modId: number, event?: MouseEvent) {
    trigger =
      event?.currentTarget instanceof HTMLButtonElement
        ? event.currentTarget
        : trigger;
    savedScroll = list?.closest('.page')?.scrollTop ?? 0;
    void registry.detail(modId);
    await tick();
    detailHeading?.focus();
  }
  async function back() {
    registry.back();
    await tick();
    const page = list?.closest('.page');
    if (page) page.scrollTop = savedScroll;
    trigger?.focus({ preventScroll: true });
  }
  function cycleTag(id: string) {
    const q = $registry.query;
    const included = q.includeTags.includes(id);
    const excluded = q.excludeTags.includes(id);
    registry.query({
      includeTags:
        included || excluded
          ? q.includeTags.filter((tag) => tag !== id)
          : [...q.includeTags, id],
      excludeTags: included
        ? [...q.excludeTags, id]
        : q.excludeTags.filter((tag) => tag !== id),
    });
  }
  function toggleBuild(build: string) {
    const chosen = $registry.query.gameBuilds;
    registry.query({
      gameBuilds: chosen.includes(build)
        ? chosen.filter((item) => item !== build)
        : [...chosen, build],
    });
  }
  async function install(selected: Release) {
    accepted = null;
    if (await registry.install(selected)) {
      accepted = releaseKey(selected);
      await manager.refresh();
    }
  }
  async function image(id: string, event: MouseEvent) {
    galleryTrigger = event.currentTarget as HTMLButtonElement;
    preview = id;
    await tick();
    gallery.showModal();
  }
  function imageFailed(id: string) {
    if (!failedImages.includes(id)) failedImages = [...failedImages, id];
  }
</script>

{#if !$registry.account}
  <div class="empty-state">
    <h2>Sign in to browse Mods</h2>
    <p>
      Use your Steam account through Starframe’s website. Installed mods, local
      imports and collections remain available offline.
    </p>
    <button onclick={onsettings}>Open sign-in settings</button>
  </div>
{:else}
  <div class:with-detail={$registry.selected !== null} class="browser">
    <div class="browse-list" bind:this={list}>
      <form
        class="browse-toolbar"
        onsubmit={(event) => {
          event.preventDefault();
          registry.query({ query: search });
        }}
      >
        <label class="search"
          >Search mods<input
            type="search"
            bind:value={search}
            maxlength="200"
            placeholder="Name, description or author"
          /></label
        >
        <button type="submit">Search</button>
        <label
          >Sort<select
            value={$registry.query.sort}
            onchange={(event) =>
              registry.query({
                sort: event.currentTarget.value as typeof $registry.query.sort,
              })}
          >
            <option value="updated">Recently updated</option><option
              value="published">Recently published</option
            ><option value="downloads">Most downloaded</option><option
              value="name">Name</option
            >
          </select></label
        >
        <label
          >Period<select
            value={$registry.query.period}
            onchange={(event) =>
              registry.query({
                period: event.currentTarget
                  .value as typeof $registry.query.period,
              })}
          >
            <option value="all">All time</option><option value="24h"
              >24 hours</option
            ><option value="7d">7 days</option><option value="1m"
              >1 month</option
            ><option value="3m">3 months</option><option value="6m"
              >6 months</option
            ><option value="1y">1 year</option>
          </select></label
        >
        <label
          >Layout<select
            value={$registry.layout}
            onchange={(event) =>
              registry.layout(
                event.currentTarget.value as typeof $registry.layout,
              )}
            ><option value="rows">Rows</option><option value="cards"
              >Cards</option
            ><option value="tiles">Tiles</option></select
          ></label
        >
        <button
          type="button"
          aria-expanded={filters}
          aria-controls="mod-filters"
          onclick={() => (filters = !filters)}
          >Filters{filterCount ? ` (${filterCount})` : ''}</button
        >
        <button
          type="button"
          disabled={$registry.loading}
          onclick={() => registry.browse(true)}>Refresh mods</button
        >
      </form>
      <div id="mod-filters" hidden={!filters} class="filter-panel">
        <div class="filter-selects">
          <label
            >Maintenance<select
              value={$registry.query.maintenance}
              onchange={(event) =>
                registry.query({
                  maintenance: event.currentTarget
                    .value as typeof $registry.query.maintenance,
                })}
              ><option value="all">All</option><option value="maintained"
                >Maintained</option
              ><option value="unmaintained">Unmaintained</option></select
            ></label
          >
          <label
            >Mod type<select
              value={$registry.query.modType}
              onchange={(event) =>
                registry.query({
                  modType: event.currentTarget
                    .value as typeof $registry.query.modType,
                })}
              ><option value="all">All</option><option value="code">Code</option
              ><option value="map">Map</option><option value="ai">AI</option
              ><option value="unclassified">Unclassified</option></select
            ></label
          >
          <button
            onclick={() =>
              registry.query({
                includeTags: [],
                excludeTags: [],
                gameBuilds: [],
                maintenance: 'all',
                modType: 'all',
              })}>Reset filters</button
          >
        </div>
        {#each groups as group (group)}
          <fieldset>
            <legend>{group}</legend>
            <div class="tag-buttons">
              {#each $registry.options?.tags.filter((tag) => tag.groupName === group) ?? [] as tag (tag.id)}
                {@const state = $registry.query.includeTags.includes(tag.id)
                  ? 'Include'
                  : $registry.query.excludeTags.includes(tag.id)
                    ? 'Exclude'
                    : 'Any'}
                <button
                  class:chosen={state !== 'Any'}
                  aria-label={`${tag.label}: ${state}. Click to ${state === 'Any' ? 'include' : state === 'Include' ? 'exclude' : 'clear'}.`}
                  onclick={() => cycleTag(tag.id)}
                  >{tag.label}{state !== 'Any' ? ` · ${state}` : ''}</button
                >
              {/each}
            </div>
          </fieldset>
        {/each}
        <p class="muted">
          Include every selected tag; exclude any selected tag. Game builds
          match any selected build.
        </p>
        {#if $registry.options?.gameBuilds.length}<fieldset>
            <legend>Tested game builds</legend>
            <div class="tag-buttons">
              {#each $registry.options.gameBuilds as build (build)}<label
                  class="build-check"
                  ><input
                    type="checkbox"
                    checked={$registry.query.gameBuilds.includes(build)}
                    onchange={() => toggleBuild(build)}
                  />{build}</label
                >{/each}
            </div>
          </fieldset>{/if}
      </div>
      {#if $registry.error}<p class="error" role="alert">
          {$registry.error}{#if $registry.results}
            Previous results remain below.{/if}
        </p>{/if}
      <p
        class="result-status"
        role="status"
        aria-live={active ? 'polite' : 'off'}
      >
        {#if $registry.loading}Loading mods…{#if $registry.results}
            Previous results remain below.{/if}
        {:else if $registry.results}{$registry.results.pagination.totalItems} mods
          · Checked {new Date(
            $registry.results.pagination.asOf,
          ).toLocaleString()}{/if}
      </p>
      {#if $registry.results}
        <ul
          class="results"
          class:cards={$registry.layout === 'cards'}
          class:tiles={$registry.layout === 'tiles'}
          aria-label="Mod results"
          aria-busy={$registry.loading}
        >
          {#each $registry.results.items as mod (mod.modId)}
            <li class:selected={$registry.selected === mod.modId}>
              <button
                class="mod-open"
                aria-label={`Details for ${mod.name}`}
                aria-pressed={$registry.selected === mod.modId}
                onclick={(event) => open(mod.modId, event)}
              >
                {#if mod.iconId && !failedImages.includes(mod.iconId)}<img
                    class="mod-icon"
                    src={mediaUrl(mod.iconId, 'thumbnail')}
                    alt=""
                    loading="lazy"
                    onerror={() => imageFailed(mod.iconId!)}
                  />{:else}<span
                    class="mod-icon icon-placeholder"
                    aria-hidden="true"
                    >{(mod.modType ?? 'Mod').toUpperCase()}</span
                  >{/if}
                <span class="mod-copy"
                  ><strong>{mod.name}</strong><span
                    >{mod.owner?.displayName ?? 'Author unavailable'} · {mod.modType ??
                      'Unclassified'}</span
                  ><span class="summary">{mod.summary}</span><span
                    class="mod-meta"
                    >{mod.downloads.toLocaleString()} downloads · {mod.likes.toLocaleString()}
                    likes{mod.archiveBytes !== null
                      ? ` · ${bytes(mod.archiveBytes)}`
                      : ''}{!mod.maintained
                      ? ' · Unmaintained'
                      : ''}{mod.latestAvailability &&
                    mod.latestAvailability !== 'available'
                      ? ` · ${mod.latestAvailability}`
                      : ''}</span
                  ></span
                >
              </button>
            </li>
          {/each}
        </ul>
        {#if !$registry.results.items.length}<div class="empty-state">
            <h2>No matching mods</h2>
            <p>Change the search or filters to try again.</p>
          </div>{/if}
        <div class="paging" aria-label="Mod pages">
          <button
            disabled={$registry.loading ||
              $registry.results.pagination.page <= 1}
            onclick={() =>
              registry.query(
                { page: $registry.results!.pagination.page - 1 },
                true,
              )}>Previous page</button
          >
          <span
            >Page {$registry.results.pagination.page} of {Math.max(
              1,
              $registry.results.pagination.totalPages,
            )}</span
          >
          <button
            disabled={$registry.loading ||
              $registry.results.pagination.page >=
                $registry.results.pagination.totalPages}
            onclick={() =>
              registry.query(
                { page: $registry.results!.pagination.page + 1 },
                true,
              )}>Next page</button
          >
          <label
            >Per page<select
              value={$registry.query.pageSize}
              onchange={(event) =>
                registry.query({ pageSize: Number(event.currentTarget.value) })}
              >{#each sizes[$registry.layout] as size (size)}<option
                  value={size}>{size}</option
                >{/each}</select
            ></label
          >
        </div>
      {/if}
      <p class="provenance muted">
        Approved releases come from authors through Starframe’s registry.
        Approval does not guarantee that a binary is free of malware.
      </p>
    </div>
    {#if $registry.selected !== null}
      <section
        class="mod-detail"
        aria-label="Mod details"
        aria-busy={$registry.detailLoading}
      >
        <button onclick={back}>Back to mods</button>
        <h2 bind:this={detailHeading} tabindex="-1">
          {listing?.listing.name ?? `Mod ${$registry.selected}`}
        </h2>
        {#if $registry.detailLoading}<p role="status">
            Loading release details…
          </p>{/if}
        {#if $registry.detailError}<p class="error" role="alert">
            {$registry.detailError}
          </p>
          <button onclick={() => registry.detail($registry.selected!)}
            >Retry details</button
          >{/if}
        {#if listing}
          <p>
            {listing.listing.owner?.displayName ?? 'Author unavailable'} · {listing
              .listing.modType ?? 'Unclassified'}
          </p>
          {#if !listing.listing.maintained}<p>
              Unmaintained{#if listing.listing.replacementModId}
                · <button
                  class="text-action"
                  onclick={() => open(listing.listing.replacementModId!)}
                  >View replacement mod</button
                >{/if}
            </p>{/if}
          <p class="description">{listing.description}</p>
          {#if listing.sourceRepository}<p>
              <button
                class="text-action"
                onclick={() => onsource(listing.listing.modId)}
                >Author’s source repository</button
              >
            </p>{/if}
          {#if listing.media.screenshotIds.length}<div
              class="screenshots"
              aria-label="Mod screenshots"
            >
              {#each listing.media.screenshotIds as id, index (id)}<button
                  disabled={failedImages.includes(id)}
                  aria-haspopup="dialog"
                  aria-label={`Preview screenshot ${index + 1}`}
                  onclick={(event) => image(id, event)}
                  >{#if failedImages.includes(id)}Screenshot unavailable{:else}<img
                      src={mediaUrl(id, 'thumbnail')}
                      alt={`Screenshot ${index + 1}`}
                      loading="lazy"
                      onerror={() => imageFailed(id)}
                    />{/if}</button
                >{/each}
            </div>{/if}
        {/if}
        {#if release}
          <h3>Release {release.versionLabel}</h3>
          <p>
            Approved · {release.availability} · {bytes(release.artifact.bytes)}
          </p>
          <p>Published {new Date(release.publishedAt).toLocaleDateString()}</p>
          <p>Tested game build: {release.metadata.testedGameBuild}</p>
          <p>
            {gameBuild === release.metadata.testedGameBuild
              ? 'Tested with this game build.'
              : gameBuild
                ? `Your game build: ${gameBuild}. This release was tested with a different build.`
                : 'Choose a game in Settings to compare builds.'}
          </p>
          {#if release.metadata.installation?.kind === 'map' || release.metadata.installation?.kind === 'ai'}<p
              class="muted"
            >
              Uses the author’s declared {release.metadata.installation.kind ===
              'map'
                ? 'map'
                : 'AI'} folder. Placement is supported; gameplay compatibility needs
              game acceptance.
            </p>{/if}
          {#if release.security.status === 'blocked'}<p class="error">
              {release.security.reason ??
                'This release is blocked by a confirmed security decision.'}
            </p>{/if}
          {#if installedVersions.length}<p>
              Installed: {installedVersions
                .map((entry) => entry.version)
                .join(', ')}. Collections keep their selected releases.
            </p>{/if}
          <button
            class="install"
            disabled={installed ||
              pending ||
              $registry.detailLoading ||
              !!$registry.detailError ||
              release.availability !== 'available' ||
              release.security.status === 'blocked' ||
              !release.metadata.installation}
            onclick={() => install(release)}
            >{installed
              ? 'Installed'
              : pending
                ? 'Preparing install…'
                : 'Install this release'}</button
          >
          {#if !release.metadata.installation}<p>
              Installation information is unavailable for this release.
            </p>{/if}
          {#if $registry.installError}<p class="error" role="alert">
              {$registry.installError}
            </p>{/if}
          {#if accepted === releaseKey(release)}<p role="status">
              Install queued. Enable the installed release in My mods when it is
              ready.
            </p>
            <button onclick={ondownloads}>View Downloads</button>{/if}
          <h3>Release notes</h3>
          <p class="description">
            {release.metadata.releaseNotes || 'No release notes.'}
          </p>
          {#if release.metadata.dependencies.length}<h3>Dependencies</h3>
            <ul class="dependencies">
              {#each release.metadata.dependencies as dependency (dependency.modId)}<li
                >
                  <button onclick={() => open(dependency.modId)}
                    >Mod {dependency.modId}</button
                  >
                  <p>
                    {dependency.kind === 'exact'
                      ? `Exact release ${dependency.releaseId}`
                      : `${dependency.minimum ? `From ${dependency.minimum}` : 'Any version'}${dependency.before ? `, before ${dependency.before}` : ''}${dependency.includePrerelease ? ', prereleases allowed' : ', stable releases only'}`}
                  </p>
                </li>{/each}
            </ul>
            <p>
              Choose and install required releases explicitly. Enabling checks
              the active collection’s exact selections.
            </p>{/if}
          {#each release.metadata.dependencyProblems as problem (problem.dependency.modId)}<p
              class="error"
            >
              Dependency {problem.dependency.modId}: {problem.code.replaceAll(
                '_',
                ' ',
              )}.
            </p>{/each}
        {:else if $registry.release}<p class="error">
            Release {$registry.release.availability}. It is unavailable for
            installation.
          </p>{/if}
        {#if $registry.history}
          <h3>Release history</h3>
          <ul class="history">
            {#each $registry.history.items as item (item.releaseId)}<li>
                {#if 'artifact' in item}<button
                    aria-pressed={release?.releaseId === item.releaseId}
                    disabled={$registry.detailLoading}
                    onclick={() => registry.release(item.releaseId)}
                    >{item.versionLabel}</button
                  ><span
                    >{item.availability} · {new Date(
                      item.publishedAt,
                    ).toLocaleDateString()}</span
                  >{:else}<span>Unavailable release · {item.availability}</span
                  >{/if}
              </li>{/each}
          </ul>
          <div class="paging">
            <button
              disabled={$registry.detailLoading ||
                $registry.history.pagination.page <= 1}
              onclick={() =>
                registry.detail(
                  $registry.selected!,
                  $registry.history!.pagination.page - 1,
                )}>Previous releases</button
            ><span
              >Page {$registry.history.pagination.page} of {Math.max(
                1,
                $registry.history.pagination.totalPages,
              )}</span
            ><button
              disabled={$registry.detailLoading ||
                $registry.history.pagination.page >=
                  $registry.history.pagination.totalPages}
              onclick={() =>
                registry.detail(
                  $registry.selected!,
                  $registry.history!.pagination.page + 1,
                )}>Next releases</button
            >
          </div>
        {/if}
      </section>
    {/if}
  </div>
{/if}
<dialog
  bind:this={gallery}
  aria-label="Screenshot preview"
  onclose={() => {
    preview = null;
    galleryTrigger?.focus();
  }}
>
  <button onclick={() => gallery.close()}>Close screenshot</button>
  {#if preview}{#if failedImages.includes(preview)}<p role="status">
        Screenshot unavailable.
      </p>{:else}<img
        src={mediaUrl(preview, 'display')}
        alt="Mod screenshot preview"
        onerror={() => imageFailed(preview!)}
      />{/if}{/if}
</dialog>

<style>
  .browser {
    min-width: 0;
  }
  .browse-toolbar,
  .filter-selects,
  .tag-buttons,
  .paging {
    display: flex;
    align-items: end;
    flex-wrap: wrap;
    gap: 12px;
  }
  .browse-toolbar {
    margin-bottom: 16px;
  }
  label {
    display: grid;
    gap: 4px;
    max-width: 100%;
  }
  .search {
    flex: 1 1 240px;
  }
  .filter-panel {
    padding: 16px;
    margin-bottom: 16px;
    background: var(--surface);
  }
  fieldset {
    border: 1px solid var(--selected);
    padding: 12px;
    margin: 12px 0;
    min-width: 0;
  }
  .build-check {
    display: flex;
    align-items: center;
  }
  .chosen {
    border-color: var(--accent);
  }
  .result-status {
    min-height: 24px;
    margin: 12px 0;
  }
  .results,
  .dependencies,
  .history {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .results {
    display: grid;
    gap: 8px;
  }
  .results li {
    min-width: 0;
    border-left: 3px solid transparent;
  }
  .results li.selected {
    border-color: var(--accent);
  }
  .mod-open {
    display: flex;
    align-items: start;
    width: 100%;
    gap: 16px;
    text-align: left;
    padding: 16px;
    border: 1px solid var(--selected);
    border-radius: 0;
  }
  .mod-copy {
    min-width: 0;
    display: grid;
    gap: 4px;
    overflow-wrap: anywhere;
  }
  .mod-copy strong {
    font-size: 1.142857rem;
  }
  .mod-icon {
    width: 56px;
    height: 56px;
    object-fit: contain;
    flex-shrink: 0;
  }
  .icon-placeholder {
    display: grid;
    place-items: center;
    color: var(--outline);
    background: var(--canvas);
    font-size: 0.857143rem;
  }
  .mod-meta {
    color: var(--outline);
    font-size: 0.857143rem;
  }
  .cards {
    grid-template-columns: repeat(auto-fill, minmax(min(280px, 100%), 1fr));
  }
  .tiles {
    grid-template-columns: repeat(auto-fill, minmax(min(190px, 100%), 1fr));
  }
  .cards .mod-open,
  .tiles .mod-open {
    height: 100%;
    flex-direction: column;
  }
  .tiles .summary {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .paging {
    align-items: center;
    margin-top: 16px;
  }
  .paging label {
    display: flex;
    align-items: center;
  }
  .paging select {
    width: auto;
  }
  .provenance {
    margin-top: 24px;
  }
  .mod-detail {
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: start;
    gap: 12px;
    background: var(--surface);
    padding: 20px;
  }
  .mod-detail h2 {
    margin: 0;
  }
  .mod-detail h3 {
    margin: 8px 0 0;
    color: var(--text);
  }
  .mod-detail p {
    max-width: 100%;
  }
  .description {
    white-space: pre-wrap;
  }
  .install {
    background: var(--accent);
    color: var(--canvas);
    font-weight: 600;
  }
  .install:disabled {
    background: var(--selected);
    color: var(--body);
  }
  .text-action {
    padding: 0;
    min-height: 36px;
    background: transparent;
    border: 0;
    text-decoration: underline;
    color: #fed7aa;
  }
  .screenshots {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    width: 100%;
  }
  .screenshots button {
    padding: 4px;
  }
  .screenshots img {
    width: 100%;
    height: 90px;
    object-fit: contain;
  }
  .dependencies li,
  .history li {
    margin-bottom: 12px;
    overflow-wrap: anywhere;
  }
  .history li {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  dialog {
    max-width: min(1000px, 90vw);
    max-height: 90vh;
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--outline);
  }
  dialog::backdrop {
    background: rgb(15 23 42 / 0.8);
  }
  dialog img {
    display: block;
    max-width: 100%;
    max-height: 70vh;
    margin-top: 12px;
  }
  @media (min-width: 1280px) {
    .browser.with-detail {
      display: grid;
      grid-template-columns: minmax(0, 1fr) 360px;
      gap: 20px;
      align-items: start;
    }
  }
  @media (max-width: 1279px) {
    .with-detail .browse-list {
      display: none;
    }
  }
</style>
