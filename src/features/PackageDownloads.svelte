<script lang="ts">
  import { bytes, transferring, type Management } from '../lib/management';
  let { manager, unavailable }: { manager: Management; unavailable: boolean } =
    $props();
</script>

<h2 class="download-title">Downloads and imports</h2>
<p>
  Completed downloads and imports are verified in your library. Enable them in
  My mods; game readiness is shown in the launch area.
</p>
{#if !$manager.operations.length}<p class="result-count">
    No downloads or imports yet. Install a release from Catalog or import a
    local mod in My mods.
  </p>{:else}
  <ul class="operations">
    {#each $manager.operations as op (op.id)}<li>
        <div class="operation-heading">
          <h3>
            {op.releaseId === 'local-import' ? 'Local import' : op.releaseId}
          </h3>
          <span
            >{op.status === 'completed'
              ? 'Installed in library'
              : op.status}</span
          >
        </div>
        <p>{op.message}</p>
        <div class="progress-row">
          <progress
            aria-label={op.releaseId === 'local-import'
              ? 'Local import'
              : `Download ${op.releaseId}`}
            max={op.totalBytes || 1}
            value={op.releaseId === 'local-import' && transferring(op)
              ? undefined
              : op.receivedBytes}
          ></progress><span
            >{bytes(op.receivedBytes)}{op.releaseId === 'local-import' &&
            transferring(op)
              ? ' copied'
              : ` / ${bytes(op.totalBytes)}`}</span
          >
        </div>
        {#if transferring(op)}<button
            disabled={unavailable ||
              op.status === 'cancelling' ||
              $manager.pending.includes(`cancel:${op.id}`)}
            onclick={() => manager.cancel(op.id)}
            >{op.status === 'cancelling'
              ? 'Cancelling…'
              : op.releaseId === 'local-import'
                ? 'Cancel import'
                : 'Cancel download'}</button
          >
        {:else if (op.status === 'failed' || op.status === 'cancelled') && op.releaseId !== 'local-import' && op.releaseId !== 'local-verification'}<button
            disabled={unavailable ||
              $manager.pending.includes(`install:${op.releaseId}`)}
            onclick={() => manager.install(op.releaseId)}
            >Retry exact release</button
          >{/if}
        {#if (op.status === 'failed' || op.status === 'cancelled') && op.releaseId === 'local-import'}<p
          >
            Fix the source, then use Import local mod in My mods to retry.
          </p>
        {:else if op.status === 'failed' && op.releaseId !== 'local-verification'}<p
          >
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
