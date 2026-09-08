<script lang="ts">
  import type { Management } from '../lib/management';
  let { manager, unavailable }: { manager: Management; unavailable: boolean } =
    $props();
</script>

<label class="collection-selector">
  Active collection
  <select
    value={$manager.data?.activeCollection ?? ''}
    disabled={unavailable ||
      !$manager.data ||
      $manager.pending.includes('membership') ||
      !$manager.data.collections.length}
    onchange={(event) => {
      const id = event.currentTarget.value;
      event.currentTarget.value = $manager.data?.activeCollection ?? '';
      void manager.collection({ kind: 'select_collection', id });
    }}
  >
    <option value="" disabled>No active collection</option>
    {#each $manager.data?.collections ?? [] as collection (collection.id)}
      <option value={collection.id}>{collection.name}</option>
    {/each}
  </select>
</label>

<style>
  .collection-selector {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  select {
    max-width: min(100%, 32rem);
  }
</style>
