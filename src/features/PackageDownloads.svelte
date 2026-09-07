<script lang="ts">
  import { bytes, transferring, type Management } from '../lib/management';
  let { manager, unavailable }: { manager: Management; unavailable: boolean } =
    $props();
</script>

<h2 class="download-title">Mod downloads</h2>
<p>
  Completed downloads are verified in your library. Enable them in My mods; game
  readiness is shown in the launch area.
</p>
{#if !$manager.operations.length}<p class="result-count">
    No mod downloads yet. Install a release from Catalog.
  </p>{:else}
  <ul class="operations">
    {#each $manager.operations as op (op.id)}<li>
        <div class="operation-heading">
          <h3>{op.releaseId}</h3>
          <span
            >{op.status === 'completed'
              ? 'Installed in library'
              : op.status}</span
          >
        </div>
        <p>{op.message}</p>
        <div class="progress-row">
          <progress
            aria-label={`Download ${op.releaseId}`}
            max={op.totalBytes || 1}
            value={op.receivedBytes}
          ></progress><span
            >{bytes(op.receivedBytes)} / {bytes(op.totalBytes)}</span
          >
        </div>
        {#if transferring(op)}<button
            disabled={unavailable ||
              op.status === 'cancelling' ||
              $manager.pending.includes(`cancel:${op.id}`)}
            onclick={() => manager.cancel(op.id)}
            >{op.status === 'cancelling'
              ? 'Cancelling…'
              : 'Cancel download'}</button
          >
        {:else if op.status === 'failed' || op.status === 'cancelled'}<button
            disabled={unavailable ||
              $manager.pending.includes(`install:${op.releaseId}`)}
            onclick={() => manager.install(op.releaseId)}
            >Retry exact release</button
          >{/if}
        {#if op.status === 'failed'}<p>
            Retry uses the same approved release. A failed request does not mean
            the author withdrew it.
          </p>{/if}
      </li>{/each}
  </ul>{/if}
{#if $manager.data?.cleanupErrors.length}<div class="error">
    <h3>Uninstall cleanup needs attention</h3>
    {#each $manager.data.cleanupErrors as error (error)}<p>
        {error}
      </p>{/each}<button
      disabled={unavailable || $manager.pending.includes('cleanup')}
      onclick={() => manager.cleanup()}>Retry cleanup</button
    >
  </div>{/if}
