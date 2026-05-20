import { queue } from "$lib/stores/queue.svelte";
import { makeThumbnail } from "$lib/tauri/commands";

const THUMB_CONCURRENCY = 4;

/// Fire off thumbnail jobs for any queue items still in `pending` state.
/// Runs with a small concurrency cap so we don't oversaturate the worker pool.
export async function requestPendingThumbnails(): Promise<void> {
  const pending = queue.items.filter((i) => i.status === "pending");
  if (pending.length === 0) return;

  let cursor = 0;
  const next = async () => {
    while (cursor < pending.length) {
      const item = pending[cursor++];
      queue.update(item.id, { status: "thumbnailing" });
      try {
        const dataUrl = await makeThumbnail(item.path);
        queue.update(item.id, {
          status: "ready",
          thumbnailDataUrl: dataUrl,
        });
      } catch (err) {
        queue.update(item.id, {
          status: "error",
          error: String(err),
        });
      }
    }
  };

  await Promise.all(
    Array.from(
      { length: Math.min(THUMB_CONCURRENCY, pending.length) },
      () => next(),
    ),
  );
}
