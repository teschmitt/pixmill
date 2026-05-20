<script lang="ts">
  import DropZone from "$lib/components/DropZone.svelte";
  import QueueList from "$lib/components/QueueList.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import RunBar from "$lib/components/RunBar.svelte";
  import PreviewModal from "$lib/components/PreviewModal.svelte";
  import { queue } from "$lib/stores/queue.svelte";
  import { preview } from "$lib/stores/preview.svelte";
</script>

<main class="app">
  <header class="topbar">
    <h1>Image Batch Processor</h1>
    <div class="muted">{queue.items.length} files queued</div>
  </header>

  <div class="layout">
    <section class="left">
      <DropZone />
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
