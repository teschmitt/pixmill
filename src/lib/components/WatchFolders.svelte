<script lang="ts">
  import WatchFolderRow from "$lib/components/WatchFolderRow.svelte";
  import { platform } from "$lib/platform";
  import { toQueueItem } from "$lib/queueItem";
  import { queue } from "$lib/stores/queue.svelte";
  import { watchFolders } from "$lib/stores/watchFolders.svelte";
  import { requestPendingThumbnails } from "$lib/thumbnails";

  let addError = $state<string | null>(null);
  let busy = $state(false);

  async function onAddFolder() {
    addError = null;
    const path = await platform.pickFolder();
    if (!path) return;
    try {
      const folder = { path, recursive: true, autoProcess: false };
      await platform.addWatchedFolder(folder);
      watchFolders.add(folder);
    } catch (e) {
      addError = String(e);
    }
  }

  async function scanAll() {
    if (watchFolders.folders.length === 0) return;
    busy = true;
    try {
      for (const folder of watchFolders.folders) {
        const items = await platform.ingest([folder.path], folder.recursive);
        queue.add(items.map(toQueueItem));
      }
      void requestPendingThumbnails();
    } finally {
      busy = false;
    }
  }
</script>

<section class="watch">
  <header>
    <h2>Watch folders</h2>
    <div class="actions">
      <button onclick={onAddFolder}>Add watch folder…</button>
      <button onclick={scanAll} disabled={busy || watchFolders.folders.length === 0}>
        {busy ? "Scanning…" : "Scan all once"}
      </button>
    </div>
  </header>

  {#if addError}
    <div class="error">{addError}</div>
  {/if}

  {#if watchFolders.folders.length === 0}
    <p class="hint">
      Register a folder and new images dropped into it will appear in the queue automatically.
    </p>
  {:else}
    <div class="list">
      {#each watchFolders.folders as folder (folder.path)}
        <WatchFolderRow {folder} />
      {/each}
    </div>
  {/if}
</section>

<style>
  .watch {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  h2 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
    margin: 0;
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .actions button {
    padding: 4px 10px;
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
  .actions button[disabled] {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .error {
    font-size: 12px;
    color: #b03a2e;
  }
</style>
