<script lang="ts">
  import { batch } from "$lib/stores/batch.svelte";
  import { queue } from "$lib/stores/queue.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { pickOutputFolder, runBatch } from "$lib/tauri/commands";

  let canRun = $derived(
    !batch.running &&
      queue.items.length > 0 &&
      settings.outputDir !== null &&
      queue.items.some((i) => i.status !== "error")
  );

  async function chooseOutput() {
    const dir = await pickOutputFolder();
    if (dir) settings.outputDir = dir;
  }

  async function run() {
    const outDir = settings.outputDir;
    if (!outDir) return;
    const targets = queue.items.filter((i) => i.status !== "error");
    if (targets.length === 0) return;

    batch.start(targets.length);
    for (const t of targets) {
      queue.update(t.id, { status: "processing", error: undefined });
    }

    try {
      await runBatch(
        targets.map((t) => t.path),
        outDir,
        settings.current,
        (update) => {
          batch.completed = update.completed;
          batch.total = update.total;
          const matching = queue.items.find((i) => i.path === update.item.source);
          if (matching) {
            if (update.item.error) {
              queue.update(matching.id, {
                status: "error",
                error: update.item.error,
              });
              batch.errors += 1;
            } else {
              queue.update(matching.id, {
                status: "done",
                destination: update.item.destination ?? undefined,
              });
            }
          }
        }
      );
      batch.finish(
        batch.errors === 0
          ? `Processed ${batch.completed} file(s)`
          : `Processed ${batch.completed} with ${batch.errors} error(s)`
      );
    } catch (err) {
      batch.finish(`Batch failed: ${err}`);
    }
  }
</script>

<section class="run">
  <h2>Output</h2>
  <div class="folder">
    <button onclick={chooseOutput}>Choose folder…</button>
    {#if settings.outputDir}
      <div class="path" title={settings.outputDir}>{settings.outputDir}</div>
    {:else}
      <div class="path muted">(none selected)</div>
    {/if}
  </div>

  {#if batch.running || batch.completed > 0}
    <div class="progress">
      <div
        class="bar"
        style="width: {batch.total > 0 ? (batch.completed / batch.total) * 100 : 0}%"
      ></div>
    </div>
    <div class="muted small">
      {batch.completed} / {batch.total}{#if batch.errors > 0}
        · {batch.errors} error(s)
      {/if}
    </div>
  {/if}

  {#if batch.lastMessage}
    <div class="muted small">{batch.lastMessage}</div>
  {/if}

  <button class="primary" disabled={!canRun} onclick={run}>
    {batch.running ? "Running…" : "Run batch"}
  </button>
</section>

<style>
  .run {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: auto;
  }
  h2 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
    margin: 0;
  }
  .folder {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .folder button {
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
    align-self: flex-start;
  }
  .path {
    font-size: 11px;
    word-break: break-all;
  }
  .progress {
    height: 6px;
    background: var(--border);
    border-radius: 3px;
    overflow: hidden;
  }
  .bar {
    height: 100%;
    background: var(--accent);
    transition: width 0.15s;
  }
  .primary {
    background: var(--accent);
    color: white;
    border: 0;
    padding: 10px 16px;
    border-radius: 6px;
    font-weight: 600;
    cursor: pointer;
  }
  .primary[disabled] {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
</style>
