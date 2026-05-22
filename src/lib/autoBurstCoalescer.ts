import { processBurst } from "$lib/processBurst.svelte";

/// Tunables. Quiet-window favors the "drop a folder of photos" scenario;
/// the age cap keeps a steady drip (rsync, backup tools) from sitting in the
/// bucket forever; the size cap prevents one giant accumulated payload from
/// monopolizing the worker pool for minutes.
const QUIET_MS = 250;
const SIZE_CAP = 50;
const AGE_MS = 10_000;

interface Bucket {
  paths: string[];
  quietTimer: ReturnType<typeof setTimeout> | null;
  ageTimer: ReturnType<typeof setTimeout> | null;
  inflight: boolean;
  /// Paths that arrived while a flush was in-flight. Drained into `paths`
  /// when the in-flight flush resolves (decision #9, B2 serialization).
  pending: string[];
}

/// JS-side burst coalescer. Per-folder buckets dedupe streams of `fileAdded`
/// events into batched `processBurst` calls, so the rayon worker pool sees a
/// few large batches instead of N independent `runBatch([one])` invocations.
/// Concurrent bursts on the same folder are serialized — never overlap.
export class AutoBurstCoalescer {
  private buckets = new Map<string, Bucket>();

  /// Tunables exposed for tests so they can probe the size/age/quiet caps
  /// without timing fragility.
  readonly quietMs = QUIET_MS;
  readonly sizeCap = SIZE_CAP;
  readonly ageMs = AGE_MS;

  push(folderPath: string, filePath: string): void {
    const bucket = this.bucketFor(folderPath);
    if (bucket.inflight) {
      bucket.pending.push(filePath);
      return;
    }
    bucket.paths.push(filePath);

    if (bucket.quietTimer) clearTimeout(bucket.quietTimer);
    bucket.quietTimer = setTimeout(() => this.flush(folderPath), this.quietMs);

    if (bucket.paths.length === 1 && bucket.ageTimer === null) {
      bucket.ageTimer = setTimeout(() => this.flush(folderPath), this.ageMs);
    }

    if (bucket.paths.length >= this.sizeCap) {
      void this.flush(folderPath);
    }
  }

  private bucketFor(folderPath: string): Bucket {
    let bucket = this.buckets.get(folderPath);
    if (!bucket) {
      bucket = {
        paths: [],
        quietTimer: null,
        ageTimer: null,
        inflight: false,
        pending: [],
      };
      this.buckets.set(folderPath, bucket);
    }
    return bucket;
  }

  /// Drain the bucket's `paths` and dispatch them. If the bucket is already
  /// inflight at flush-time (size-cap and a timer can race), the call no-ops.
  private async flush(folderPath: string): Promise<void> {
    const bucket = this.buckets.get(folderPath);
    if (!bucket || bucket.inflight || bucket.paths.length === 0) return;

    if (bucket.quietTimer) {
      clearTimeout(bucket.quietTimer);
      bucket.quietTimer = null;
    }
    if (bucket.ageTimer) {
      clearTimeout(bucket.ageTimer);
      bucket.ageTimer = null;
    }

    const snapshot = bucket.paths;
    bucket.paths = [];
    bucket.inflight = true;
    try {
      await processBurst(folderPath, snapshot);
    } finally {
      bucket.inflight = false;
      if (bucket.pending.length > 0) {
        bucket.paths = bucket.pending;
        bucket.pending = [];
        // Recurse: dispatch the queued paths as a fresh burst. The timers
        // will be re-armed by the recursive flush if it doesn't fire
        // immediately, but in practice the recursion path triggers the
        // flush right away.
        if (bucket.paths.length >= this.sizeCap) {
          void this.flush(folderPath);
        } else {
          bucket.quietTimer = setTimeout(() => this.flush(folderPath), this.quietMs);
          bucket.ageTimer = setTimeout(() => this.flush(folderPath), this.ageMs);
        }
      }
    }
  }

  /// Test-only: wait for all per-folder buckets to drain. The tests need a
  /// way to await completion without relying on real wall-clock timing.
  async drainForTest(): Promise<void> {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    while (
      [...this.buckets.values()].some(
        (b) => b.inflight || b.paths.length > 0 || b.pending.length > 0
      )
    ) {
      await new Promise((r) => setTimeout(r, 5));
    }
  }
}

export const autoBurstCoalescer = new AutoBurstCoalescer();
