import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { processBurst } from "$lib/processBurst.svelte";

// Mock the processBurst module before the coalescer is imported, so the
// singleton picks up the spy. We model `processBurst` as an async function
// whose resolution we control per-test via `manualPromise`. The mock is
// typed against the real signature so `mock.calls[i][1]` is well-typed.
let manualPromise: { promise: Promise<void>; resolve: () => void } | null = null;
const defaultImpl: typeof processBurst = async () => {
  if (manualPromise) await manualPromise.promise;
};
const processBurstMock = vi.fn<typeof processBurst>(defaultImpl);

vi.mock("$lib/processBurst.svelte", () => ({
  processBurst: processBurstMock,
}));

// Import the coalescer AFTER the mock so the import resolves to the mock.
const { AutoBurstCoalescer } = await import("$lib/autoBurstCoalescer");

describe("AutoBurstCoalescer", () => {
  beforeEach(() => {
    processBurstMock.mockReset();
    // mockReset clears the implementation; restore the default impl.
    processBurstMock.mockImplementation(defaultImpl);
    manualPromise = null;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("flushes a single batch when 3 paths arrive within the quiet window", async () => {
    const coalescer = new AutoBurstCoalescer();
    coalescer.push("/a", "/a/1.jpg");
    coalescer.push("/a", "/a/2.jpg");
    coalescer.push("/a", "/a/3.jpg");
    // No flush yet — still inside the quiet window.
    expect(processBurstMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(coalescer.quietMs + 1);
    expect(processBurstMock).toHaveBeenCalledTimes(1);
    expect(processBurstMock).toHaveBeenCalledWith("/a", ["/a/1.jpg", "/a/2.jpg", "/a/3.jpg"]);
  });

  it("flushes at the size cap (50) and again after the quiet window for the remainder", async () => {
    const coalescer = new AutoBurstCoalescer();
    for (let i = 0; i < 60; i += 1) {
      coalescer.push("/a", `/a/${i}.jpg`);
    }
    // The 50th push triggers a size-cap flush synchronously.
    expect(processBurstMock).toHaveBeenCalledTimes(1);
    expect(processBurstMock.mock.calls[0][1]).toHaveLength(coalescer.sizeCap);

    // Yield the microtask queue so the inflight promise resolves (the mock
    // returns immediately when `manualPromise` is null) and the bucket
    // re-arms its timer for the remaining 10.
    await vi.advanceTimersByTimeAsync(0);

    await vi.advanceTimersByTimeAsync(coalescer.quietMs + 1);
    expect(processBurstMock).toHaveBeenCalledTimes(2);
    expect(processBurstMock.mock.calls[1][1]).toHaveLength(10);
  });

  it("flushes at the age cap when paths drip in faster than the quiet window", async () => {
    const coalescer = new AutoBurstCoalescer();
    // Period 220ms is shorter than the 250ms quiet window — each push
    // resets the quiet timer before it can fire, so the quiet flush never
    // wins. It's also long enough that fewer than 50 pushes fit in the
    // 10s age window (~46), so the size cap is not hit either. That
    // isolates the age-cap path.
    const period = 220;
    const elapsedAtFirstFlush: number[] = [];
    const startTime = Date.now();
    processBurstMock.mockImplementation(async () => {
      elapsedAtFirstFlush.push(Date.now() - startTime);
    });

    let cursor = 0;
    while (cursor < coalescer.ageMs + 1000 && processBurstMock.mock.calls.length === 0) {
      coalescer.push("/a", `/a/${cursor}.jpg`);
      await vi.advanceTimersByTimeAsync(period);
      cursor += period;
    }

    expect(processBurstMock).toHaveBeenCalledTimes(1);
    expect(elapsedAtFirstFlush[0]).toBeGreaterThanOrEqual(coalescer.ageMs);
    expect(elapsedAtFirstFlush[0]).toBeLessThan(coalescer.ageMs + period * 2);
    // The size cap wasn't reached — the flush should have well under 50.
    expect(processBurstMock.mock.calls[0][1].length).toBeLessThan(coalescer.sizeCap);
  });

  it("serializes concurrent bursts on the same folder", async () => {
    const coalescer = new AutoBurstCoalescer();
    // Block the first processBurst call so we can push more paths while it
    // is inflight.
    let resolveFirst!: () => void;
    manualPromise = {
      promise: new Promise<void>((r) => {
        resolveFirst = r;
      }),
      resolve: () => resolveFirst(),
    };

    coalescer.push("/a", "/a/1.jpg");
    await vi.advanceTimersByTimeAsync(coalescer.quietMs + 1);
    expect(processBurstMock).toHaveBeenCalledTimes(1);

    // Second burst lands while first is still pending — should be deferred.
    coalescer.push("/a", "/a/2.jpg");
    coalescer.push("/a", "/a/3.jpg");
    await vi.advanceTimersByTimeAsync(coalescer.quietMs + 1);
    expect(processBurstMock).toHaveBeenCalledTimes(1);

    // Resolve the first burst. The pending paths drain into a new batch.
    manualPromise = null;
    resolveFirst();
    // Allow the inflight resolution + recursive flush schedule to run.
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(coalescer.quietMs + 1);
    expect(processBurstMock).toHaveBeenCalledTimes(2);
    expect(processBurstMock.mock.calls[1][1]).toEqual(["/a/2.jpg", "/a/3.jpg"]);
  });
});
