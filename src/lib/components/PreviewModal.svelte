<script lang="ts">
  import { platform, type PreviewResult, type SourceLoad } from "$lib/platform";
  import { settings } from "$lib/stores/settings.svelte";
  import { formatBytes, formatDimensions } from "$lib/format";
  import type { QueueItem } from "$lib/types";

  const MIN_VIEW_LEVEL = 1;
  const MAX_VIEW_LEVEL = 50;
  const KEY_ZOOM_FACTOR = 1.2;

  let { item, onClose }: { item: QueueItem; onClose: () => void } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let sourcePane: HTMLDivElement | undefined = $state();
  let previewPane: HTMLDivElement | undefined = $state();

  let preview: PreviewResult | null = $state(null);
  let previewError: string | null = $state(null);
  let previewLoading = $state(true);
  let source: SourceLoad | null = $state(null);
  let sourceError: string | null = $state(null);
  let sourceLoading = $state(true);

  // Shared, synced view across both panes. The same `(viewCenterX, viewCenterY)`
  // normalized point of each image is brought to that pane's center; `viewLevel`
  // is a multiplier on each pane's own fit-to-pane scale. Caveat: with crop, the
  // preview is a sub-region of the source, so "same normalized point" is not the
  // same content region — fixing that needs a crop-aware coordinate transform.
  let viewLevel = $state(1);
  let viewCenterX = $state(0.5);
  let viewCenterY = $state(0.5);

  let sourcePaneSize = $state({ w: 0, h: 0 });
  let previewPaneSize = $state({ w: 0, h: 0 });

  // Natural image dims. Seeded from metadata; img.onload corrects to post-orientation
  // values (modern browsers apply EXIF orientation to naturalWidth/Height).
  let sourceNatural = $state({ w: 0, h: 0 });
  let previewNatural = $state({ w: 0, h: 0 });

  $effect(() => {
    sourceNatural = { w: item.width ?? 0, h: item.height ?? 0 };
  });
  $effect(() => {
    if (preview) previewNatural = { w: preview.width, h: preview.height };
  });

  $effect(() => {
    dialog?.showModal();
  });

  $effect(() => {
    let cancelled = false;
    sourceLoading = true;
    source = null;
    sourceError = null;
    platform
      .loadSource(item.path)
      .then((res) => {
        if (cancelled) return;
        source = res;
      })
      .catch((e) => {
        if (cancelled) return;
        sourceError = String(e);
      })
      .finally(() => {
        if (!cancelled) sourceLoading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    let cancelled = false;
    previewLoading = true;
    preview = null;
    previewError = null;
    resetView();
    platform
      .previewOne(item.path, settings.current)
      .then((res) => {
        if (cancelled) return;
        preview = res;
      })
      .catch((e) => {
        if (cancelled) return;
        previewError = String(e);
      })
      .finally(() => {
        if (!cancelled) previewLoading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    // Re-runs when the bound refs become available or change.
    const src = sourcePane;
    const prev = previewPane;
    if (!src && !prev) return;
    const observer = new ResizeObserver(() => {
      if (src) {
        const r = src.getBoundingClientRect();
        sourcePaneSize = { w: r.width, h: r.height };
      }
      if (prev) {
        const r = prev.getBoundingClientRect();
        previewPaneSize = { w: r.width, h: r.height };
      }
    });
    if (src) observer.observe(src);
    if (prev) observer.observe(prev);
    return () => observer.disconnect();
  });

  function resetView() {
    viewLevel = 1;
    viewCenterX = 0.5;
    viewCenterY = 0.5;
  }

  interface PaneGeom {
    fitScale: number;
    paneScale: number;
    imgW: number;
    imgH: number;
    panX: number;
    panY: number;
  }

  function computeGeom(
    naturalW: number,
    naturalH: number,
    paneW: number,
    paneH: number
  ): PaneGeom {
    if (naturalW <= 0 || naturalH <= 0 || paneW <= 0 || paneH <= 0) {
      return { fitScale: 1, paneScale: 1, imgW: 0, imgH: 0, panX: 0, panY: 0 };
    }
    const fitScale = Math.min(paneW / naturalW, paneH / naturalH);
    const paneScale = fitScale * viewLevel;
    const imgW = naturalW * paneScale;
    const imgH = naturalH * paneScale;

    let panX = paneW / 2 - viewCenterX * imgW;
    let panY = paneH / 2 - viewCenterY * imgH;

    // Clamp pan so image edges stay flush with the pane when image > pane;
    // center small images that fit inside the pane.
    if (imgW <= paneW) {
      panX = (paneW - imgW) / 2;
    } else {
      panX = Math.max(paneW - imgW, Math.min(0, panX));
    }
    if (imgH <= paneH) {
      panY = (paneH - imgH) / 2;
    } else {
      panY = Math.max(paneH - imgH, Math.min(0, panY));
    }

    return { fitScale, paneScale, imgW, imgH, panX, panY };
  }

  let sourceGeom = $derived(
    computeGeom(sourceNatural.w, sourceNatural.h, sourcePaneSize.w, sourcePaneSize.h)
  );
  let previewGeom = $derived(
    computeGeom(previewNatural.w, previewNatural.h, previewPaneSize.w, previewPaneSize.h)
  );

  function pinchZoomAtCursor(
    event: WheelEvent,
    rect: DOMRect,
    geom: PaneGeom,
    naturalW: number,
    naturalH: number
  ) {
    const factor = Math.exp(-event.deltaY * 0.01);
    const newLevel = Math.max(MIN_VIEW_LEVEL, Math.min(MAX_VIEW_LEVEL, viewLevel * factor));
    if (newLevel === viewLevel) return;

    const cx = event.clientX - rect.left;
    const cy = event.clientY - rect.top;
    // Normalized image point currently under the cursor.
    const u = geom.imgW > 0 ? (cx - geom.panX) / geom.imgW : 0.5;
    const v = geom.imgH > 0 ? (cy - geom.panY) / geom.imgH : 0.5;

    // Compute the new viewCenter that keeps (u, v) under the cursor after zoom.
    const newPaneScale = geom.fitScale * newLevel;
    const newImgW = naturalW * newPaneScale;
    const newImgH = naturalH * newPaneScale;
    viewCenterX = u + (rect.width / 2 - cx) / newImgW;
    viewCenterY = v + (rect.height / 2 - cy) / newImgH;
    viewLevel = newLevel;
  }

  function handleWheel(
    event: WheelEvent,
    pane: HTMLDivElement | undefined,
    geom: PaneGeom,
    naturalW: number,
    naturalH: number
  ) {
    if (!pane) return;
    event.preventDefault();
    const rect = pane.getBoundingClientRect();

    if (event.ctrlKey || event.metaKey) {
      // Trackpad pinch surfaces as wheel + ctrlKey on macOS WebKit.
      if (geom.imgW > 0 && geom.imgH > 0) {
        pinchZoomAtCursor(event, rect, geom, naturalW, naturalH);
      }
    } else {
      // Two-finger drag → pan. Skip when this pane's image already fits its pane
      // (the clamp on render would discard the delta, and panning the OTHER pane
      // in that case would be surprising).
      if (geom.imgW === 0 || geom.imgH === 0) return;
      if (geom.imgW <= rect.width && geom.imgH <= rect.height) return;
      // Wheel deltaX/Y > 0 corresponds to "scroll right/down" (natural scrolling),
      // i.e. viewCenter moves toward the right/bottom of the image.
      viewCenterX += event.deltaX / geom.imgW;
      viewCenterY += event.deltaY / geom.imgH;
    }
  }

  function handleSourceWheel(event: WheelEvent) {
    handleWheel(event, sourcePane, sourceGeom, sourceNatural.w, sourceNatural.h);
  }
  function handlePreviewWheel(event: WheelEvent) {
    handleWheel(event, previewPane, previewGeom, previewNatural.w, previewNatural.h);
  }

  type PaneName = "source" | "preview";
  let dragging: { pane: PaneName; lastX: number; lastY: number } | null = $state(null);

  function handlePointerDown(event: PointerEvent, pane: PaneName) {
    if (event.button !== 0) return;
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    dragging = { pane, lastX: event.clientX, lastY: event.clientY };
  }

  function handlePointerMove(
    event: PointerEvent,
    pane: PaneName,
    geom: PaneGeom,
    paneSize: { w: number; h: number }
  ) {
    if (!dragging || dragging.pane !== pane) return;
    const dx = event.clientX - dragging.lastX;
    const dy = event.clientY - dragging.lastY;
    dragging.lastX = event.clientX;
    dragging.lastY = event.clientY;

    if (geom.imgW === 0 || geom.imgH === 0) return;
    if (geom.imgW <= paneSize.w && geom.imgH <= paneSize.h) return;

    // Drag content with the cursor: cursor right → image right → viewCenter left.
    viewCenterX -= dx / geom.imgW;
    viewCenterY -= dy / geom.imgH;
  }

  function handlePointerEnd(event: PointerEvent, pane: PaneName) {
    if (!dragging || dragging.pane !== pane) return;
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) {
      target.releasePointerCapture(event.pointerId);
    }
    dragging = null;
  }

  function zoomBoth(factor: number) {
    viewLevel = Math.max(MIN_VIEW_LEVEL, Math.min(MAX_VIEW_LEVEL, viewLevel * factor));
  }

  function setPaneToPixelPerfect(geom: PaneGeom) {
    if (geom.fitScale <= 0) return;
    viewLevel = Math.max(MIN_VIEW_LEVEL, Math.min(MAX_VIEW_LEVEL, 1 / geom.fitScale));
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    const target = event.target as HTMLElement | null;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;

    if (event.key === "+" || event.key === "=") {
      event.preventDefault();
      zoomBoth(KEY_ZOOM_FACTOR);
    } else if (event.key === "-" || event.key === "_") {
      event.preventDefault();
      zoomBoth(1 / KEY_ZOOM_FACTOR);
    } else if (event.key === "0") {
      event.preventDefault();
      resetView();
    }
  }

  function handleImgLoad(
    event: Event,
    setNatural: (dims: { w: number; h: number }) => void
  ) {
    const img = event.target as HTMLImageElement;
    if (img.naturalWidth > 0 && img.naturalHeight > 0) {
      setNatural({ w: img.naturalWidth, h: img.naturalHeight });
    }
  }

  function handleClose() {
    onClose();
  }

  function handleBackdrop(event: MouseEvent) {
    if (event.target === dialog) onClose();
  }
</script>

<dialog
  bind:this={dialog}
  onclose={handleClose}
  onclick={handleBackdrop}
  onkeydown={handleKeydown}
  aria-labelledby="preview-title"
>
  <header>
    <div class="title" id="preview-title" title={item.path}>{item.filename}</div>
    <button class="close" onclick={handleClose} aria-label="Close preview">×</button>
  </header>

  <div class="body">
    <figure>
      <figcaption>Source</figcaption>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="pane zoom-pane"
        class:pannable={sourceGeom.imgW > sourcePaneSize.w ||
          sourceGeom.imgH > sourcePaneSize.h}
        class:dragging={dragging?.pane === "source"}
        bind:this={sourcePane}
        onwheel={handleSourceWheel}
        ondblclick={resetView}
        onpointerdown={(e) => handlePointerDown(e, "source")}
        onpointermove={(e) => handlePointerMove(e, "source", sourceGeom, sourcePaneSize)}
        onpointerup={(e) => handlePointerEnd(e, "source")}
        onpointercancel={(e) => handlePointerEnd(e, "source")}
      >
        {#if sourceLoading}
          <div class="placeholder">Loading…</div>
        {:else if sourceError}
          <div class="error">{sourceError}</div>
        {:else if source}
          <img
            src={source.dataUrl}
            alt="Source: {item.filename}"
            class="zoomable"
            draggable="false"
            onload={(e) => handleImgLoad(e, (d) => (sourceNatural = d))}
            style:transform="translate({sourceGeom.panX}px, {sourceGeom.panY}px) scale({sourceGeom.paneScale})"
            style:image-rendering={sourceGeom.paneScale >= 1 ? "pixelated" : "auto"}
          />
        {/if}
      </div>
      <div class="meta">
        {formatDimensions(sourceNatural.w || item.width, sourceNatural.h || item.height)}
        {#if item.format}
          · <span class="fmt">{item.format}</span>
        {/if}
        · {formatBytes(item.sizeBytes)}
        ·
        <button
          class="zoom"
          type="button"
          onclick={() => setPaneToPixelPerfect(sourceGeom)}
          title="Click for pixel-perfect 100% (or 0 to fit, +/- to zoom)"
        >
          {Math.round(sourceGeom.paneScale * 100)}%
        </button>
      </div>
    </figure>

    <figure>
      <figcaption>Preview (current settings)</figcaption>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="pane zoom-pane"
        class:pannable={previewGeom.imgW > previewPaneSize.w ||
          previewGeom.imgH > previewPaneSize.h}
        class:dragging={dragging?.pane === "preview"}
        bind:this={previewPane}
        onwheel={handlePreviewWheel}
        ondblclick={resetView}
        onpointerdown={(e) => handlePointerDown(e, "preview")}
        onpointermove={(e) => handlePointerMove(e, "preview", previewGeom, previewPaneSize)}
        onpointerup={(e) => handlePointerEnd(e, "preview")}
        onpointercancel={(e) => handlePointerEnd(e, "preview")}
      >
        {#if previewLoading}
          <div class="placeholder">Rendering…</div>
        {:else if previewError}
          <div class="error">{previewError}</div>
        {:else if preview}
          <img
            src={preview.dataUrl}
            alt="Processed preview"
            class="zoomable"
            draggable="false"
            onload={(e) => handleImgLoad(e, (d) => (previewNatural = d))}
            style:transform="translate({previewGeom.panX}px, {previewGeom.panY}px) scale({previewGeom.paneScale})"
            style:image-rendering={previewGeom.paneScale >= 1 ? "pixelated" : "auto"}
          />
        {/if}
      </div>
      <div class="meta">
        {#if preview}
          {formatDimensions(preview.width, preview.height)}
          · <span class="fmt">{preview.format}</span>
          · {formatBytes(preview.sizeBytes)}
          ·
          <button
            class="zoom"
            type="button"
            onclick={() => setPaneToPixelPerfect(previewGeom)}
            title="Click for pixel-perfect 100% (or 0 to fit, +/- to zoom)"
          >
            {Math.round(previewGeom.paneScale * 100)}%
          </button>
        {:else}
          &nbsp;
        {/if}
      </div>
    </figure>
  </div>
</dialog>

<style>
  dialog {
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel);
    color: var(--text);
    padding: 0;
    width: min(92vw, 1100px);
    max-height: 92vh;
  }
  dialog::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    font-weight: 600;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .close {
    background: transparent;
    border: 0;
    color: var(--muted);
    font-size: 22px;
    line-height: 1;
    cursor: pointer;
    padding: 0 4px;
  }
  .close:hover {
    color: var(--text);
  }
  .body {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    padding: 16px;
  }
  figure {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  figcaption {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
  }
  .pane {
    position: relative;
    background: rgba(127, 127, 127, 0.08);
    border: 1px solid var(--border);
    border-radius: 6px;
    aspect-ratio: 4 / 3;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  .zoom-pane {
    touch-action: none;
    overscroll-behavior: contain;
  }
  .zoom-pane.pannable {
    cursor: grab;
  }
  .zoom-pane.dragging {
    cursor: grabbing;
  }
  .zoomable {
    position: absolute;
    left: 0;
    top: 0;
    /* No width/height: img uses naturalWidth/Height; CSS transform scales it. */
    transform-origin: 0 0;
    will-change: transform;
    user-select: none;
    -webkit-user-drag: none;
  }
  .zoom {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    color: var(--muted);
    font: inherit;
    padding: 0 4px;
    cursor: pointer;
    font-variant-numeric: tabular-nums;
  }
  .zoom:hover {
    color: var(--text);
    border-color: var(--border);
  }
  .placeholder {
    color: var(--muted);
    font-size: 13px;
  }
  .error {
    color: #c0392b;
    font-size: 12px;
    padding: 12px;
    text-align: center;
  }
  .meta {
    color: var(--muted);
    font-size: 12px;
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    align-items: center;
  }
  .fmt {
    text-transform: uppercase;
  }
  @media (max-width: 700px) {
    .body {
      grid-template-columns: 1fr;
    }
  }
</style>
