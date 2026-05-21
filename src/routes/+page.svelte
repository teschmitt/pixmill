<script lang="ts">
  import { onMount } from "svelte";

  import { autoBurstCoalescer } from "$lib/autoBurstCoalescer";
  import DropZone from "$lib/components/DropZone.svelte";
  import PreviewModal from "$lib/components/PreviewModal.svelte";
  import QueueList from "$lib/components/QueueList.svelte";
  import RunBar from "$lib/components/RunBar.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import WatchFolders from "$lib/components/WatchFolders.svelte";
  import { platform } from "$lib/platform";
  import { toQueueItem } from "$lib/queueItem";
  import { preview } from "$lib/stores/preview.svelte";
  import { queue } from "$lib/stores/queue.svelte";
  import { watchFolders } from "$lib/stores/watchFolders.svelte";
  import { requestPendingThumbnails } from "$lib/thumbnails";
  import type { WatchEvent } from "$lib/types";

  function handleWatchEvent(event: WatchEvent) {
    if (event.kind === "fileAdded") {
      // Phase 4 modify-aware (decision #12): the backend emits the same
      // FileAdded variant for Create / Rename(To) / Modify(Data). We
      // disambiguate by inspecting the current queue status.
      const existing = queue.items.find((i) => i.path === event.item.path);
      const folder = watchFolders.folders.find((f) => f.path === event.folder);
      if (!existing) {
        // New file — Phase 3 behavior: enqueue and (if autoProcess) coalesce.
        queue.add([toQueueItem(event.item)]);
        watchFolders.applyEvent(event);
        void requestPendingThumbnails();
        if (folder?.autoProcess) {
          autoBurstCoalescer.push(event.folder, event.item.path);
        }
      } else if (existing.status === "done" || existing.status === "error") {
        // In-place edit of a previously-processed file. Flip back to pending
        // (clearing the stale destination/error) and re-feed the coalescer
        // for autoProcess folders.
        queue.update(existing.id, {
          status: "pending",
          error: undefined,
          destination: undefined,
        });
        watchFolders.applyEvent(event);
        if (folder?.autoProcess) {
          autoBurstCoalescer.push(event.folder, event.item.path);
        }
      }
      // `pending` / `processing` → no-op: the item is already in line for
      // processing, so duplicating work would be wasted.
    } else if (event.kind === "fileRemoved") {
      // Stale-pending cleanup (decision #14): drop only `pending` rows.
      // `processing` / `done` / `error` rows survive so the historical
      // record is preserved when a user moves processed files around.
      const item = queue.items.find((i) => i.path === event.path);
      if (item && item.status === "pending") queue.remove(item.id);
    } else if (event.kind === "folderStatus") {
      watchFolders.applyEvent(event);
    }
  }

  onMount(() => {
    if (platform.supportsWatchFolders) {
      void platform
        .subscribeWatchEvents(handleWatchEvent)
        .catch((e) => console.warn("subscribeWatchEvents failed:", e));
    }
  });
</script>

<main class="app">
  <header class="topbar">
    <h1>Pixmill</h1>
    <div class="muted">{queue.items.length} files queued</div>
  </header>

  <div class="layout">
    <section class="left">
      <DropZone />
      {#if platform.supportsWatchFolders}
        <WatchFolders />
      {/if}
      <QueueList />
    </section>
    <aside class="right">
      <SettingsPanel />
      <RunBar />
    </aside>
  </div>

  {#if preview.item}
    <PreviewModal item={preview.item} onClose={() => preview.close()} />
  {/if}
</main>

<style>
  :global(:root) {
    color-scheme: light dark;
    --bg: #fafafa;
    --panel: #ffffff;
    --border: #e2e2e2;
    --text: #111;
    --muted: #666;
    --accent: #396cd8;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #1a1a1a;
      --panel: #232323;
      --border: #333;
      --text: #f0f0f0;
      --muted: #999;
      --accent: #6b8eff;
    }
  }
  :global(html, body) {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family:
      system-ui,
      -apple-system,
      Segoe UI,
      Roboto,
      sans-serif;
  }
  .app {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
  }
  .topbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .topbar h1 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }
  .muted {
    color: var(--muted);
    font-size: 13px;
  }
  .layout {
    display: grid;
    grid-template-columns: 1fr 320px;
    flex: 1;
    min-height: 0;
  }
  .left {
    display: flex;
    flex-direction: column;
    padding: 16px;
    gap: 16px;
    min-height: 0;
  }
  .right {
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--border);
    background: var(--panel);
    padding: 16px;
    gap: 16px;
  }
</style>
