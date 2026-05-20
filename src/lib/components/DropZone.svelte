<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";

  import { queue } from "$lib/stores/queue.svelte";
  import { ingestPaths, pickFiles, pickFolder } from "$lib/tauri/commands";
  import { requestPendingThumbnails } from "$lib/thumbnails";
  import type { QueueItem } from "$lib/types";

  let recursive = $state(true);
  let busy = $state(false);
  let dragging = $state(false);

  function toQueueItem(raw: {
    path: string;
    filename: string;
    format: string | null;
    width: number | null;
    height: number | null;
    sizeBytes: number | null;
    error: string | null;
  }): QueueItem {
    return {
      id: raw.path,
      path: raw.path,
      filename: raw.filename,
      format: (raw.format as QueueItem["format"]) ?? null,
      width: raw.width ?? undefined,
      height: raw.height ?? undefined,
      sizeBytes: raw.sizeBytes ?? undefined,
      status: raw.error ? "error" : "pending",
      error: raw.error ?? undefined,
    };
  }

  async function ingest(paths: string[]) {
    if (paths.length === 0) return;
    busy = true;
    try {
      const results = await ingestPaths(paths, recursive);
      queue.add(results.map(toQueueItem));
    } finally {
      busy = false;
    }
    void requestPendingThumbnails();
  }

  async function onAddFiles() {
    const paths = await pickFiles();
    await ingest(paths);
  }

  async function onAddFolder() {
    const folder = await pickFolder();
    if (folder) await ingest([folder]);
  }

  onMount(() => {
    const webview = getCurrentWebview();
    const unlisten = webview.onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === "enter" || p.type === "over") {
        dragging = true;
      } else if (p.type === "leave") {
        dragging = false;
      } else if (p.type === "drop") {
        dragging = false;
        void ingest(p.paths);
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });
</script>

<div class="drop" class:active={dragging}>
  <div class="hint">
    {dragging ? "Release to add" : "Drop images or folders here"}
  </div>
  <div class="row">
    <button onclick={onAddFiles} disabled={busy}>Add files…</button>
    <button onclick={onAddFolder} disabled={busy}>Add folder…</button>
    <label class="recurse">
      <input type="checkbox" bind:checked={recursive} />
      recursive
    </label>
  </div>
</div>

<style>
  .drop {
    border: 2px dashed var(--border);
    border-radius: 8px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    background: var(--panel);
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .drop.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, var(--panel));
  }
  .hint {
    font-weight: 500;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  button {
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
  button:hover:not([disabled]) {
    border-color: var(--accent);
  }
  button[disabled] {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .recurse {
    font-size: 13px;
    color: var(--muted);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
</style>
