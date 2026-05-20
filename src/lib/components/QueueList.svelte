<script lang="ts">
  import { queue } from "$lib/stores/queue.svelte";
  import ThumbnailCard from "./ThumbnailCard.svelte";
</script>

<section class="grid-wrap">
  {#if queue.items.length === 0}
    <div class="empty">No files yet. Drop images or use the buttons above.</div>
  {:else}
    <div class="actions">
      <button onclick={() => queue.clear()}>Clear all</button>
    </div>
    <div class="grid">
      {#each queue.items as item (item.id)}
        <ThumbnailCard {item} />
      {/each}
    </div>
  {/if}
</section>

<style>
  .grid-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 12px;
    display: flex;
    flex-direction: column;
  }
  .empty {
    margin: auto;
    color: var(--muted);
    text-align: center;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 8px;
  }
  .actions button {
    padding: 4px 10px;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--muted);
    cursor: pointer;
    font-size: 12px;
  }
  .actions button:hover {
    color: var(--text);
    border-color: var(--accent);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 12px;
  }
</style>
