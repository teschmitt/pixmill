<script lang="ts">
  import { queue } from "$lib/stores/queue.svelte";
  import { preview } from "$lib/stores/preview.svelte";
  import { formatBytes, formatDimensions } from "$lib/format";
  import type { QueueItem } from "$lib/types";

  let { item }: { item: QueueItem } = $props();

  function remove(event: MouseEvent) {
    event.stopPropagation();
    queue.remove(item.id);
  }

  function openPreview() {
    preview.open(item);
  }

  function handleKey(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      openPreview();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
<article
  class="card"
  class:error={item.status === "error"}
  title={item.path}
  role="button"
  tabindex="0"
  onclick={openPreview}
  onkeydown={handleKey}
>
  <div class="thumb">
    {#if item.thumbnailDataUrl}
      <img src={item.thumbnailDataUrl} alt={item.filename} />
    {:else if item.status === "error"}
      <div class="placeholder muted">!</div>
    {:else}
      <div class="placeholder muted">…</div>
    {/if}
    <button class="remove" onclick={remove} aria-label="Remove">×</button>
    {#if item.status === "done"}
      <span class="badge done">done</span>
    {:else if item.status === "processing"}
      <span class="badge processing">…</span>
    {/if}
  </div>
  <div class="meta">
    <div class="name" title={item.filename}>{item.filename}</div>
    <div class="sub">
      <span>{formatDimensions(item.width, item.height)}</span>
      <span>·</span>
      <span>{formatBytes(item.sizeBytes)}</span>
      {#if item.format}
        <span>·</span>
        <span class="fmt">{item.format}</span>
      {/if}
    </div>
    {#if item.error}
      <div class="err" title={item.error}>{item.error}</div>
    {/if}
  </div>
</article>

<style>
  .card {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    transition: border-color 0.15s;
    cursor: pointer;
    text-align: left;
  }
  .card:hover {
    border-color: var(--accent);
  }
  .card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .card.error {
    border-color: #c0392b;
  }
  .thumb {
    position: relative;
    aspect-ratio: 1;
    background: rgba(127, 127, 127, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .placeholder {
    font-size: 24px;
  }
  .remove {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 0;
    background: rgba(0, 0, 0, 0.6);
    color: white;
    cursor: pointer;
    display: none;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    line-height: 1;
  }
  .card:hover .remove {
    display: flex;
  }
  .badge {
    position: absolute;
    bottom: 4px;
    left: 4px;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    color: white;
  }
  .badge.done {
    background: #2e7d32;
  }
  .badge.processing {
    background: var(--accent);
  }
  .meta {
    padding: 8px 10px;
    font-size: 12px;
    min-width: 0;
  }
  .name {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    color: var(--muted);
    font-size: 11px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 2px;
  }
  .fmt {
    text-transform: uppercase;
  }
  .err {
    color: #c0392b;
    font-size: 11px;
    margin-top: 4px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .muted {
    color: var(--muted);
  }
</style>
