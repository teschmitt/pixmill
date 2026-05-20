<script lang="ts">
  import { settings } from "$lib/stores/settings.svelte";
  import type {
    CompressionMode,
    CropMode,
    OutputFormatChoice,
    ResizeMode,
    RotateMode,
  } from "$lib/types";

  let s = $derived(settings.current);

  let resizeKind = $derived(s.resize.kind);
  let resizePixels = $derived(s.resize.kind === "maxLongEdge" ? s.resize.pixels : 1920);
  let resizePercent = $derived(s.resize.kind === "percentage" ? s.resize.percent : 50);

  let cropKind = $derived(s.crop.kind);
  let cropW = $derived(
    s.crop.kind === "aspectRatio" || s.crop.kind === "pixels" ? s.crop.width : 1
  );
  let cropH = $derived(
    s.crop.kind === "aspectRatio" || s.crop.kind === "pixels" ? s.crop.height : 1
  );

  let compressionKind = $derived(s.compression.kind);
  let compressionKilobytes = $derived(
    s.compression.kind === "targetFileSize" ? s.compression.kilobytes : 500
  );

  function setResize(mode: ResizeMode) {
    settings.current.resize = mode;
  }
  function setCrop(mode: CropMode) {
    settings.current.crop = mode;
  }
  function setRotate(mode: RotateMode) {
    settings.current.rotate = mode;
  }
  function setFormat(fmt: OutputFormatChoice) {
    settings.current.outputFormat = fmt;
  }
  function setCompression(mode: CompressionMode) {
    settings.current.compression = mode;
  }

  const rotateOptions: { value: RotateMode; label: string }[] = [
    { value: "none", label: "None" },
    { value: "cw90", label: "90° CW" },
    { value: "cw180", label: "180°" },
    { value: "cw270", label: "270° CW" },
    { value: "flipH", label: "Flip H" },
    { value: "flipV", label: "Flip V" },
  ];
</script>

<section class="panel">
  <h2>Resize</h2>
  <label class="radio">
    <input
      type="radio"
      name="resize"
      checked={resizeKind === "none"}
      onchange={() => setResize({ kind: "none" })}
    />
    None
  </label>
  <label class="radio">
    <input
      type="radio"
      name="resize"
      checked={resizeKind === "maxLongEdge"}
      onchange={() => setResize({ kind: "maxLongEdge", pixels: resizePixels })}
    />
    Long edge ≤
    <input
      type="number"
      min="1"
      max="20000"
      value={resizePixels}
      disabled={resizeKind !== "maxLongEdge"}
      oninput={(e) =>
        setResize({
          kind: "maxLongEdge",
          pixels: Math.max(1, Number(e.currentTarget.value) || 1),
        })}
    />
    px
  </label>
  <label class="radio">
    <input
      type="radio"
      name="resize"
      checked={resizeKind === "percentage"}
      onchange={() => setResize({ kind: "percentage", percent: resizePercent })}
    />
    Scale to
    <input
      type="number"
      min="1"
      max="1000"
      value={resizePercent}
      disabled={resizeKind !== "percentage"}
      oninput={(e) =>
        setResize({
          kind: "percentage",
          percent: Math.max(1, Number(e.currentTarget.value) || 1),
        })}
    />
    %
  </label>

  <h2>Crop</h2>
  <label class="radio">
    <input
      type="radio"
      name="crop"
      checked={cropKind === "none"}
      onchange={() => setCrop({ kind: "none" })}
    />
    None
  </label>
  <label class="radio">
    <input
      type="radio"
      name="crop"
      checked={cropKind === "aspectRatio"}
      onchange={() => setCrop({ kind: "aspectRatio", width: cropW, height: cropH })}
    />
    Aspect
    <input
      type="number"
      min="1"
      value={cropW}
      disabled={cropKind !== "aspectRatio"}
      oninput={(e) =>
        setCrop({
          kind: "aspectRatio",
          width: Math.max(1, Number(e.currentTarget.value) || 1),
          height: cropH,
        })}
    />
    :
    <input
      type="number"
      min="1"
      value={cropH}
      disabled={cropKind !== "aspectRatio"}
      oninput={(e) =>
        setCrop({
          kind: "aspectRatio",
          width: cropW,
          height: Math.max(1, Number(e.currentTarget.value) || 1),
        })}
    />
  </label>
  <label class="radio">
    <input
      type="radio"
      name="crop"
      checked={cropKind === "pixels"}
      onchange={() => setCrop({ kind: "pixels", width: cropW, height: cropH })}
    />
    Pixels
    <input
      type="number"
      min="1"
      value={cropW}
      disabled={cropKind !== "pixels"}
      oninput={(e) =>
        setCrop({
          kind: "pixels",
          width: Math.max(1, Number(e.currentTarget.value) || 1),
          height: cropH,
        })}
    />
    ×
    <input
      type="number"
      min="1"
      value={cropH}
      disabled={cropKind !== "pixels"}
      oninput={(e) =>
        setCrop({
          kind: "pixels",
          width: cropW,
          height: Math.max(1, Number(e.currentTarget.value) || 1),
        })}
    />
  </label>

  <h2>Rotate</h2>
  <div class="row">
    {#each rotateOptions as opt (opt.value)}
      <button class:active={s.rotate === opt.value} onclick={() => setRotate(opt.value)}>
        {opt.label}
      </button>
    {/each}
  </div>

  <h2>Output</h2>
  <label class="line">
    Format
    <select
      value={s.outputFormat}
      onchange={(e) => setFormat(e.currentTarget.value as OutputFormatChoice)}
    >
      <option value="keep">Keep source</option>
      <option value="jpeg">JPEG</option>
      <option value="png">PNG</option>
      <option value="webp">WebP</option>
    </select>
  </label>

  <h2>Compression</h2>
  <label class="radio">
    <input
      type="radio"
      name="compression"
      checked={compressionKind === "manual"}
      onchange={() => setCompression({ kind: "manual" })}
    />
    Manual quality
  </label>
  {#if compressionKind === "manual"}
    {#if s.outputFormat === "jpeg" || s.outputFormat === "keep"}
      <label class="line indented">
        JPEG quality
        <input
          type="range"
          min="1"
          max="100"
          value={s.jpegQuality ?? 85}
          oninput={(e) => (settings.current.jpegQuality = Number(e.currentTarget.value))}
        />
        <span class="qty">{s.jpegQuality ?? 85}</span>
      </label>
    {/if}
    {#if s.outputFormat === "webp" || s.outputFormat === "keep"}
      <label class="line indented">
        WebP quality
        <input
          type="range"
          min="1"
          max="100"
          value={s.webpQuality ?? 85}
          oninput={(e) => (settings.current.webpQuality = Number(e.currentTarget.value))}
        />
        <span class="qty">{s.webpQuality ?? 85}</span>
      </label>
    {/if}
  {/if}
  <label class="radio">
    <input
      type="radio"
      name="compression"
      checked={compressionKind === "targetFileSize"}
      onchange={() =>
        setCompression({ kind: "targetFileSize", kilobytes: compressionKilobytes })}
    />
    Target file size ≤
    <input
      type="number"
      min="1"
      max="100000"
      value={compressionKilobytes}
      disabled={compressionKind !== "targetFileSize"}
      oninput={(e) =>
        setCompression({
          kind: "targetFileSize",
          kilobytes: Math.max(1, Number(e.currentTarget.value) || 1),
        })}
    />
    KB
  </label>
  {#if compressionKind === "targetFileSize" && s.outputFormat === "png"}
    <p class="hint">Requires JPEG or WebP output</p>
  {/if}

  <label class="line">
    <input
      type="checkbox"
      checked={s.preserveExif}
      onchange={(e) => (settings.current.preserveExif = e.currentTarget.checked)}
    />
    Preserve EXIF metadata
  </label>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  h2 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
    margin: 12px 0 4px;
  }
  h2:first-child {
    margin-top: 0;
  }
  .radio,
  .line {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .radio input[type="radio"] {
    margin-right: 4px;
  }
  input[type="number"] {
    width: 70px;
    padding: 3px 6px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  input[type="number"]:disabled {
    opacity: 0.5;
  }
  select {
    padding: 4px 6px;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    flex: 1;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 4px;
  }
  .row button {
    padding: 4px 6px;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    cursor: pointer;
    font-size: 12px;
  }
  .row button.active {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  input[type="range"] {
    flex: 1;
  }
  .qty {
    min-width: 28px;
    text-align: right;
    color: var(--muted);
    font-size: 12px;
  }
  .hint {
    margin: 2px 0 0 24px;
    font-size: 12px;
    color: var(--warning, #d97757);
  }
  .line.indented {
    margin-left: 24px;
  }
</style>
