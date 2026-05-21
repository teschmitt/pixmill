import { platform } from "$lib/platform";
import { queue } from "$lib/stores/queue.svelte";
import { settings } from "$lib/stores/settings.svelte";
import { watchFolders } from "$lib/stores/watchFolders.svelte";

/// Process one burst of paths for a single watched folder. Owns per-folder
/// progress state on `watchFolders.batchInFlight`; deliberately does NOT
/// touch the global `batch` store, so a user-initiated "Run batch" via
/// `RunBar` and an auto-process burst never collide on shared state
/// (decision #10).
export async function processBurst(folderPath: string, paths: string[]): Promise<void> {
  if (paths.length === 0) return;

  const outputDir = settings.outputDir;
  if (!outputDir) {
    // No output dir means we can't write — surface the failure on the queue
    // items themselves. This shouldn't happen in practice because the
    // auto-process toggle is gated on `outputDir` being set, but a stale
    // state (e.g. user cleared the output dir mid-flight) can land us here.
    for (const path of paths) {
      const item = queue.items.find((i) => i.path === path);
      if (item) queue.update(item.id, { status: "error", error: "output folder not set" });
    }
    return;
  }

  for (const path of paths) {
    const item = queue.items.find((i) => i.path === path);
    if (item) queue.update(item.id, { status: "processing", error: undefined });
  }

  watchFolders.startBatch(folderPath, paths.length);

  try {
    await platform.runBatch(
      paths,
      outputDir,
      $state.snapshot(settings.current),
      (update) => {
        const matching = queue.items.find((i) => i.path === update.item.source);
        if (matching) {
          if (update.item.error) {
            queue.update(matching.id, { status: "error", error: update.item.error });
          } else {
            queue.update(matching.id, {
              status: "done",
              destination: update.item.destination ?? undefined,
            });
          }
        }
        watchFolders.applyBurstProgress(folderPath, update);
      }
    );
  } finally {
    watchFolders.finishBatch(folderPath);
  }
}
