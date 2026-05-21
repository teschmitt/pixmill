<script lang="ts">
  import { autoBurstCoalescer } from "$lib/autoBurstCoalescer";
  import { platform } from "$lib/platform";
  import { toQueueItem } from "$lib/queueItem";
  import { queue } from "$lib/stores/queue.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { watchFolders, type WatchedFolderUi } from "$lib/stores/watchFolders.svelte";
  import { requestPendingThumbnails } from "$lib/thumbnails";

  let { folder }: { folder: WatchedFolderUi } = $props();

  let scanning = $state(false);
  let configError = $state<string | null>(null);
  let retryCooldown = $state(false);

  let autoProcessDisabled = $derived(settings.outputDir === null);

  async function scanNow() {
    scanning = true;
    try {
      const items = await platform.ingest([folder.path], folder.recursive);
      queue.add(items.map(toQueueItem));
      void requestPendingThumbnails();
      // Decision #11: autoProcess means "convert this folder", not "convert
      // only future files". A "Scan now" click on an auto-process folder
      // pushes the scanned paths into the coalescer so the backlog gets
      // processed without a separate "Run batch" click.
      if (folder.autoProcess) {
        for (const item of items) {
          if (!item.error) autoBurstCoalescer.push(folder.path, item.path);
        }
      }
    } catch (e) {
      configError = String(e);
    } finally {
      scanning = false;
    }
  }

  async function onRemove() {
    try {
      await platform.removeWatchedFolder(folder.path);
      watchFolders.remove(folder.path);
    } catch (e) {
      configError = String(e);
    }
  }

  async function setRecursive(value: boolean) {
    configError = null;
    try {
      await platform.setWatchedFolderConfig(folder.path, value, folder.autoProcess);
      watchFolders.setConfig(folder.path, value, folder.autoProcess);
    } catch (e) {
      configError = String(e);
    }
  }

  async function setAutoProcess(value: boolean) {
    configError = null;
    try {
      await platform.setWatchedFolderConfig(folder.path, folder.recursive, value);
      watchFolders.setConfig(folder.path, folder.recursive, value);
    } catch (e) {
      configError = String(e);
    }
  }

  async function onRetry() {
    if (retryCooldown) return;
    configError = null;
    retryCooldown = true;
    try {
      await platform.retryWatchedFolder(folder.path);
    } catch (e) {
      configError = String(e);
    } finally {
      // 2s cooldown so an impatient user can't flood the backend with retries
      // while notify is busy attaching the watch.
      setTimeout(() => (retryCooldown = false), 2000);
    }
  }
</script>

<div class="row">
  <div class="main">
    <div class="path" title={folder.path}>{folder.path}</div>
    <div class="controls">
      <label class="check">
        <input
          type="checkbox"
          checked={folder.recursive}
          onchange={(e) => setRecursive(e.currentTarget.checked)}
        />
        recursive
      </label>
      <label class="check" title={autoProcessDisabled ? "Set an output folder first" : ""}>
        <input
          type="checkbox"
          checked={folder.autoProcess}
          disabled={autoProcessDisabled}
          onchange={(e) => setAutoProcess(e.currentTarget.checked)}
        />
        auto-process
      </label>
      <span class="badge badge-{folder.status.kind}">
        {folder.status.kind === "error" ? folder.status.message : folder.status.kind}
      </span>
      {#if folder.batchInFlight}
        <span class="processing">
          Processing {folder.batchInFlight.completed}/{folder.batchInFlight.total}{#if folder.batchInFlight.errors > 0}
            · {folder.batchInFlight.errors} error(s)
          {/if}
        </span>
      {:else}
        <span class="counter">{folder.filesAdded} added</span>
      {/if}
    </div>
    {#if folder.batchInFlight}
      <div class="progress">
        <div
          class="bar"
          style="width: {folder.batchInFlight.total > 0
            ? (folder.batchInFlight.completed / folder.batchInFlight.total) * 100
            : 0}%"
        ></div>
      </div>
    {/if}
  </div>
  <div class="actions">
    {#if folder.status.kind === "error"}
      <button onclick={onRetry} disabled={retryCooldown}>
        {retryCooldown ? "Retrying…" : "Retry now"}
      </button>
    {/if}
    <button onclick={scanNow} disabled={scanning}>
      {scanning ? "Scanning…" : "Scan now"}
    </button>
    <button class="ghost" onclick={onRemove}>Remove</button>
  </div>
  {#if configError}
    <div class="error">{configError}</div>
  {/if}
</div>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: flex-start;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
  }
  .main {
    flex: 1;
    min-width: 200px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .path {
    font-size: 13px;
    word-break: break-all;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    font-size: 12px;
    color: var(--muted);
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .badge {
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 11px;
    border: 1px solid var(--border);
  }
  .badge-watching {
    color: #2a8c4a;
    border-color: #2a8c4a;
  }
  .badge-paused {
    color: var(--muted);
  }
  .badge-error {
    color: #b03a2e;
    border-color: #b03a2e;
  }
  .counter {
    font-size: 11px;
  }
  .processing {
    font-size: 11px;
    color: var(--accent);
  }
  .progress {
    height: 4px;
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
    margin-top: 4px;
  }
  .bar {
    height: 100%;
    background: var(--accent);
    transition: width 0.15s;
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .actions button {
    padding: 4px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    font-size: 12px;
  }
  .actions button:hover:not([disabled]) {
    border-color: var(--accent);
  }
  .actions button.ghost {
    color: var(--muted);
  }
  .actions button[disabled] {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error {
    flex-basis: 100%;
    font-size: 12px;
    color: #b03a2e;
  }
</style>
