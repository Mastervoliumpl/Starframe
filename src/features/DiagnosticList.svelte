<script lang="ts">
  const rows = Array.from({ length: 1000 }, (_, index) => ({
    id: index,
    name: `Fixture mod ${String(index + 1).padStart(4, '0')}`,
  }));
  let show = $state(false);
  let search = $state('');
  let selected = $state<number[]>([]);
  let notes = $state('');
  const filtered = $derived(
    rows.filter((row) => row.name.toLowerCase().includes(search.toLowerCase())),
  );
</script>

<div class="settings-section">
  <h2>Library interaction fixture</h2>
  <p>
    Use 1,000 sample rows to check search, selection and scrolling during a
    workload. These rows are not installed mods. Notes last only while this
    window is open.
  </p>
  <label class="checkbox-label"
    ><input type="checkbox" bind:checked={show} />Show fixture rows</label
  >
  <div hidden={!show}>
    <label class="field-label" for="fixture-search">Search fixture rows</label>
    <div class="search-row">
      <input id="fixture-search" type="search" bind:value={search} /><button
        onclick={() => (search = '')}>Clear search</button
      >
    </div>
    <p class="fixture-count">
      {filtered.length} rows · {selected.length} selected
    </p>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (the scroll region needs keyboard access) -->
    <div
      class="fixture-list"
      role="region"
      aria-label="Fixture rows"
      tabindex="0"
    >
      {#each filtered as row (row.id)}
        <label class:selected={selected.includes(row.id)}
          ><input type="checkbox" value={row.id} bind:group={selected} /><span
            >{row.name}<small>Diagnostic fixture</small></span
          ></label
        >
      {/each}
    </div>
    <label class="field-label" for="fixture-notes"
      >Check notes (not saved)</label
    >
    <textarea id="fixture-notes" rows="3" bind:value={notes}></textarea>
  </div>
</div>
