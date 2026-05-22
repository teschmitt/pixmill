---
date: 2026-05-21T08:57:29+00:00
git_commit: 1c22115d1e2631422e9136f38ef8189394b843ec
branch: web-app
topic: "How the current file import and processing pipeline works end-to-end (drag-drop, Rust commands, processing queue)"
tags: [research, codebase, pipeline, drag-drop, tauri, ipc, queue, rust, svelte]
status: complete
---

# Research: File Import and Processing Pipeline (End-to-End)

## Research Question

How does the current file import and processing pipeline work end-to-end (drag-drop, Rust commands, processing queue)?

## Summary

Pixmill has a layered pipeline that runs from a drag-drop event in the webview down to a parallel Rust image-processing batch and back up to per-item progress in the UI. The frontend is platform-agnostic at the call site (a single `platform` singleton) with two backing implementations: a Tauri impl that talks IPC to a Rust backend, and a Web impl that talks Comlink to a worker running `pixmill-wasm`. This research focuses on the Tauri path because that's the production desktop pipeline.

The pipeline has five distinct stages:

1. **Drop / pick (frontend)** — `DropZone.svelte` calls `platform.setupDropHandler()`; Tauri delivers paths via `getCurrentWebview().onDragDropEvent`; manual entry points (`pickFiles`/`pickFolder`) go through the Tauri dialog plugin.
2. **Ingest (Tauri command `ingest_paths`)** — directories are walked via `walkdir`, filtered to supported extensions, deduped, then `metadata::read` is called for each path. Returns `Vec<ImageMetadata>` (path + filename + format + dims + size + error).
3. **Queue admission (frontend store)** — `queue.add(...)` dedupes by path and pushes `QueueItem`s; status starts at `pending`. `requestPendingThumbnails()` fires up to 4 concurrent `make_thumbnail` calls.
4. **Batch run (Tauri command `run_batch` with `Channel<ProgressUpdate>`)** — `RunBar.svelte` invokes `platform.runBatch`; the command dispatches `pixmill_core::pipeline::run_batch` on `spawn_blocking`; rayon `par_iter()` processes each file; per-file progress is sent through the channel.
5. **Pipeline core (`pixmill-core`)** — decode → EXIF orient → user rotate → crop → resize → encode → write. Output path collisions get `_1`/`_2` suffixes.

```
pixmill/
├── src/                                # SvelteKit frontend (SPA)
│   ├── lib/
│   │   ├── components/
│   │   │   ├── DropZone.svelte         # drop/pick UI, calls platform.ingest → queue.add
│   │   │   ├── RunBar.svelte           # invokes platform.runBatch, threads progress to stores
│   │   │   ├── QueueList.svelte
│   │   │   └── ThumbnailCard.svelte
│   │   ├── platform/
│   │   │   ├── index.ts                # build-time switch (VITE_PLATFORM=web|tauri)
│   │   │   ├── types.ts                # Platform interface, RawMetadata, ProgressUpdate
│   │   │   ├── tauri/
│   │   │   │   ├── index.ts            # invoke() wrappers, Channel<ProgressUpdate>
│   │   │   │   └── drop.ts             # webview.onDragDropEvent → DropHandlers
│   │   │   └── web/
│   │   │       ├── index.ts            # Comlink → worker (WASM)
│   │   │       ├── drop.ts             # DOM drag events + webkitGetAsEntry
│   │   │       └── worker.ts
│   │   ├── stores/
│   │   │   ├── queue.svelte.ts         # QueueStore singleton (items[], add/update/remove)
│   │   │   ├── batch.svelte.ts         # BatchStore singleton (running/completed/total/errors)
│   │   │   ├── settings.svelte.ts
│   │   │   └── preview.svelte.ts
│   │   ├── thumbnails.ts               # concurrency-4 thumbnail fan-out
│   │   └── types.ts                    # QueueItem, Settings, BatchItemResult
│   └── routes/+page.svelte
├── src-tauri/                          # Tauri shell
│   ├── src/
│   │   ├── lib.rs                      # registers 8 IPC commands + plugins
│   │   ├── commands.rs                 # ingest_paths, run_batch, preview_one, ...
│   │   └── persistence.rs              # settings.json load/save
│   ├── capabilities/default.json       # dialog, opener, core perms
│   └── tauri.conf.json                 # dragDropEnabled: true on main window
└── crates/pixmill-core/                # pure-Rust image pipeline (no Tauri dep)
    └── src/
        ├── lib.rs                      # public API surface
        ├── ingest.rs                   # collect_from_dir, filter_supported (feature=fs)
        ├── metadata.rs                 # ImageMetadata + read()/read_many()
        ├── pipeline.rs                 # prepare/process_one/run_batch with progress callback
        ├── decode.rs                   # format-dispatched decoders
        ├── encode.rs                   # format-dispatched encoders + target-size search
        ├── ops/{orient,rotate,crop,resize,mod}.rs
        ├── exif.rs, formats.rs, settings.rs, thumbnail.rs, error.rs
```

### End-to-end flow diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                          USER GESTURE                                │
│   drag-drop          OR        "Add files…" / "Add folder…"          │
└──────────────┬──────────────────────────────────┬────────────────────┘
               │                                  │
               ▼                                  ▼
┌──────────────────────────┐     ┌────────────────────────────────────┐
│ Tauri webview            │     │ tauri-plugin-dialog                │
│ onDragDropEvent          │     │ open({ multiple, directory, ... }) │
│ → DropHandlers.onDrop    │     │ → returns paths                    │
└──────────────┬───────────┘     └────────────────┬───────────────────┘
               │                                  │
               └────────────┬─────────────────────┘
                            ▼
            DropZone.svelte: ingest(paths)
                            │
                            ▼  invoke("ingest_paths", { paths, recursive })
┌──────────────────────────────────────────────────────────────────────┐
│  Rust: commands::ingest_paths                                        │
│    walk dirs (walkdir) ─► filter_supported ─► dedupe ─► metadata::read│
│    returns Vec<ImageMetadata>                                        │
└──────────────────────────────┬───────────────────────────────────────┘
                               ▼
                queue.add(items)  ─►  status: pending
                               │
                               ▼
            requestPendingThumbnails()  (concurrency = 4)
              for each: invoke("make_thumbnail", { path })
              ─► thumbnail::make_thumbnail_data_url  (spawn_blocking)
              ─► queue.update(id, status: "ready", thumbnailDataUrl)

                            │
                            ▼  user clicks "Run batch"
            RunBar.svelte: platform.runBatch(paths, outDir, settings, onProgress)
                            │
              new Channel<ProgressUpdate>() ; channel.onmessage = onProgress
                            │
                            ▼  invoke("run_batch", { paths, outDir, settings, onProgress: channel })
┌──────────────────────────────────────────────────────────────────────┐
│  Rust: commands::run_batch (spawn_blocking)                          │
│   ─► pixmill_core::pipeline::run_batch                               │
│       create_dir_all(out_dir)                                        │
│       sources.par_iter()  // rayon                                   │
│         process_one(src):                                            │
│           prepare → decode → orient(exif) → rotate → crop → resize   │
│           encode (manual quality OR target-size binary search)       │
│           write_to_path(plan_output_path with collision suffix)      │
│         on_progress(ProgressUpdate { completed, total, item })       │
└──────────────────────────────┬───────────────────────────────────────┘
                               ▼ (per item, streamed)
            channel.onmessage → RunBar.onProgress callback
              → batch.completed/total = update...
              → queue.update(matching, { status: done | error, destination })
```

## Detailed Findings

### Stage 1 — Frontend drag-drop and picker (Tauri build)

**`src/lib/components/DropZone.svelte`** owns the import UI. On mount it calls `platform.setupDropHandler({ onDragging, onDrop })` (DropZone.svelte:57-66) and wires `onDrop` to a single `ingest(paths)` function (DropZone.svelte:35-45). The same `ingest` is reused by manual "Add files…" / "Add folder…" buttons (DropZone.svelte:47-55), so all three entry paths converge.

`ingest(paths)`:

- Calls `platform.ingest(paths, recursive)` — returns `RawMetadata[]` (DropZone.svelte:39).
- Maps each into a `QueueItem` via `toQueueItem` (DropZone.svelte:13-33). Status is `"error"` if the Rust side returned an error string for that file, otherwise `"pending"`.
- Pushes the items into the global `queue` singleton via `queue.add(...)` (DropZone.svelte:40).
- Triggers `requestPendingThumbnails()` fire-and-forget (DropZone.svelte:44).

**`src/lib/platform/index.ts`** is a build-time switch (platform/index.ts:8-10). `VITE_PLATFORM=web` selects the web bundle; everything else (including `pnpm tauri dev|build`) gets the Tauri impl. The unused branch is dead-code-eliminated by Vite.

**`src/lib/platform/tauri/drop.ts`** translates the Tauri webview's `onDragDropEvent` into platform-agnostic callbacks (tauri/drop.ts:10-26):

- `enter`/`over` → `onDragging(true)`
- `leave` → `onDragging(false)`
- `drop` → `onDragging(false)` then `onDrop(p.paths)` with native filesystem paths

Returns an unsubscribe function.

**`src/lib/platform/tauri/index.ts`** is the IPC façade. Every method is an `invoke()` wrapper:

- `ingest(paths, recursive)` → `invoke<RawMetadata[]>("ingest_paths", ...)` (tauri/index.ts:18-20)
- `pickFiles()` / `pickFolder()` → `@tauri-apps/plugin-dialog`'s `open(...)` with image extension filter (tauri/index.ts:30-47)
- `makeThumbnail(path, longEdge=256)` → `invoke<string>("make_thumbnail", ...)` (tauri/index.ts:26-28)
- `runBatch(...)` constructs a `new Channel<ProgressUpdate>()`, attaches `channel.onmessage = onProgress`, and invokes `run_batch` (tauri/index.ts:69-83)

The platform `Platform` interface is declared in `src/lib/platform/types.ts:51-73`. `RawMetadata` (platform/types.ts:3-11) and `ProgressUpdate` (platform/types.ts:13-17) shapes mirror the Rust serde structs.

### Stage 2 — Tauri command surface

**`src-tauri/src/lib.rs:5-21`** registers eight `#[tauri::command]` handlers and two plugins (`tauri_plugin_opener`, `tauri_plugin_dialog`):

| Command          | Signature (commands.rs)                                                                                                        | Purpose                                    |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------ |
| `ingest_paths`   | `(Vec<String>, bool) -> Vec<ImageMetadata>` (commands.rs:17-33)                                                                | Expand dirs, filter, dedupe, read metadata |
| `read_metadata`  | `(String) -> ImageMetadata` (commands.rs:36-39)                                                                                | Re-read one path                           |
| `make_thumbnail` | `async (String, Option<u32>) -> Result<String, String>` (commands.rs:42-51)                                                    | Base64 `data:` URL via `spawn_blocking`    |
| `load_settings`  | `(AppHandle) -> Result<Option<PersistedState>, String>` (commands.rs:54-57)                                                    | Read settings.json                         |
| `save_settings`  | `(AppHandle, PersistedState) -> Result<(), String>` (commands.rs:60-62)                                                        | Write settings.json                        |
| `load_source`    | `async (String) -> Result<SourceLoad, String>` (commands.rs:89-95)                                                             | Preview-modal source loader                |
| `preview_one`    | `async (String, Settings) -> Result<PreviewResult, String>` (commands.rs:139-156)                                              | Single-file in-memory pipeline             |
| `run_batch`      | `async (Vec<String>, String, Settings, Channel<ProgressUpdate>) -> Result<Vec<BatchItemResult>, String>` (commands.rs:175-192) | Streaming batch pipeline                   |

`ingest_paths` (commands.rs:17-33) is synchronous (no `async`). It iterates the dropped/picked paths, expands directories via `ingest::collect_from_dir(&p, recursive)` (commands.rs:23), filters via `ingest::filter_supported(...)` (commands.rs:28), passes through `dedupe_keep_order` (commands.rs:194-203), then calls `metadata::read(p)` on each. The result is a `Vec<ImageMetadata>` with per-file `error` populated if dimension read failed.

`run_batch` (commands.rs:175-192) wraps the heavy work in `tauri::async_runtime::spawn_blocking` and forwards each `ProgressUpdate` from the pipeline closure to `on_progress.send(...)`:

```rust
let results = tauri::async_runtime::spawn_blocking(move || {
    pipeline::run_batch(&sources, &out_dir, &settings, move |update| {
        let _ = on_progress.send(update);
    })
}).await...
```

`load_source` (commands.rs:89-135) has a format-aware fast path: for JPEG/PNG/WebP it reads raw bytes and lets the webview decode (the webview applies EXIF orientation natively). For AVIF/HEIC (or unknown) it decodes via `pixmill_core::decode::decode`, applies orientation via `pixmill_core::ops::orient::apply`, then re-encodes as PNG so the webview can display it.

**`src-tauri/src/persistence.rs`** is JSON-file-backed. `PersistedState { settings, output_dir }` is serialized to `app.path().app_config_dir() / "settings.json"` (persistence.rs:14-21). `load` returns `Ok(None)` when the file doesn't exist (persistence.rs:23-32).

**`src-tauri/tauri.conf.json:20`** sets `"dragDropEnabled": true` on the main window — this is what makes Tauri forward OS drag-drop events to the webview's `onDragDropEvent`. **`src-tauri/capabilities/default.json`** grants `core:default`, `opener:default`, `dialog:default`, `core:event:default`, and `core:webview:allow-internal-toggle-devtools`. There's no explicit `fs:scope:*` — paths flow over IPC as opaque strings.

### Stage 3 — Queue admission, thumbnails, and stores

**`src/lib/stores/queue.svelte.ts`** is a Svelte 5 class with a `$state<QueueItem[]>([])` array (queue.svelte.ts:3-4). It exports a singleton:

```ts
export const queue = new QueueStore();
```

- `add(newItems)` (queue.svelte.ts:6-15): dedupes by `path` against an existing-paths `Set`, appends survivors. The lint disable on line 7 is because the dedupe-only `Set` doesn't need to be reactive.
- `update(id, patch)` (queue.svelte.ts:26-29): `Object.assign(item, patch)` — Svelte 5 runes track the mutation.
- `remove(id)` / `clear()` are trivial.

**`QueueItem`** state machine, declared at `src/lib/types.ts:3-23`:

```
pending → thumbnailing → ready → processing → done
                                      └─────► error
   └─► error (ingest failed at this path)
```

Statuses: `"pending" | "thumbnailing" | "ready" | "processing" | "done" | "error"`. Transitions:

- `pending` ← initial after `queue.add` (or `error` if Rust reported an ingest error)
- `pending → thumbnailing` ← `requestPendingThumbnails` (thumbnails.ts:16)
- `thumbnailing → ready` ← thumbnail success (thumbnails.ts:19-22)
- `thumbnailing → error` ← thumbnail failure (thumbnails.ts:24-27)
- `ready → processing` ← `RunBar.run()` loops `queue.update(t.id, { status: "processing" })` (RunBar.svelte:26-28)
- `processing → done` / `processing → error` ← per-item `onProgress` callback (RunBar.svelte:39-52)

**`src/lib/thumbnails.ts:1-35`** fires the thumbnail fan-out. Concurrency is hard-capped at `THUMB_CONCURRENCY = 4` (thumbnails.ts:4). It launches up to 4 workers; each loops a shared `cursor` over the `pending` slice, calls `platform.makeThumbnail(item.path)`, and updates the item.

**`src/lib/stores/batch.svelte.ts`** holds `running`, `completed`, `total`, `errors`, `lastMessage` (batch.svelte.ts:1-30). `start(total)` resets and arms it; `finish(message?)` clears `running`. The store is mutated directly from `RunBar.svelte`'s `onProgress` callback rather than going through a method.

### Stage 4 — Batch execution and progress streaming

**`src/lib/components/RunBar.svelte`** is the consumer of `runBatch`. The button is `$derived` to require: not running, queue non-empty, output dir selected, at least one non-error item (RunBar.svelte:7-12).

`run()` (RunBar.svelte:19-63):

1. Filters out items with `status === "error"` (RunBar.svelte:22).
2. `batch.start(targets.length)` (RunBar.svelte:25).
3. Optimistically marks all targets `processing` (RunBar.svelte:26-28).
4. Calls `platform.runBatch(...)` with an `onProgress` callback that:
   - Updates `batch.completed` / `batch.total` directly (RunBar.svelte:36-37).
   - Locates the queue item by `path === update.item.source` (RunBar.svelte:38).
   - Sets status `error` (+ increments `batch.errors`) or `done` (+ stores `destination`) (RunBar.svelte:39-52).
5. On resolution, `batch.finish(...)` with success or error count message (RunBar.svelte:55-59). Network/IPC failure paths set `batch.finish("Batch failed: ...")` (RunBar.svelte:60-62).

**Streaming wire** (`src/lib/platform/tauri/index.ts:69-83`):

```ts
async function runBatch(paths, outDir, settings, onProgress) {
  const channel = new Channel<ProgressUpdate>();
  channel.onmessage = onProgress;
  return await invoke<BatchItemResult[]>("run_batch", {
    paths,
    outDir,
    settings,
    onProgress: channel,
  });
}
```

The `Channel` is a Tauri IPC primitive (`@tauri-apps/api/core`). Each `on_progress.send(update)` on the Rust side fires `channel.onmessage(update)` on the JS side. The final `invoke()` return is the full `Vec<BatchItemResult>`.

### Stage 5 — pixmill-core pipeline

**`crates/pixmill-core/src/lib.rs:1-19`** declares the module tree and re-exports the public API: `IbpError`/`IbpResult`, `ImageFormat`, `ImageMetadata`, `Settings` (and its op-mode enums), and (under feature `fs`) `read_metadata`/`read_metadata_many`. The `fs` feature gates anything that touches the disk; that's the feature `src-tauri` enables.

**`ingest.rs`** is two utility functions:

- `collect_from_dir(dir, recursive)` (ingest.rs:8-19): wraps `walkdir::WalkDir` with `max_depth = if recursive { MAX } else { 1 }`, filters to files only, then to `is_supported_input`.
- `filter_supported(paths)` (ingest.rs:22-27): drops unsupported extensions from a flat list.

**`metadata.rs`** has `ImageMetadata { path, filename, format, width, height, size_bytes, error }` (metadata.rs:7-17) and two reader functions:

- `from_bytes(bytes, filename)` (metadata.rs:21-47) — for the web/WASM path.
- `read(path)` (metadata.rs:52-81, feature `fs`) — uses `image::ImageReader::open(path).with_guessed_format().into_dimensions()` so the header is read without loading the full pixel buffer. `size_bytes` comes from `std::fs::metadata(path)`.

**`pipeline.rs`** has the layered execution model:

- `prepare_bytes(bytes, filename, settings)` (pipeline.rs:43-60): validate settings → read EXIF orientation from bytes (if `preserve_exif`) → `decode::decode_bytes` → `ops::apply_all` → `encode::resolve_output_format`. Returns `(DynamicImage, ImageFormat)`.
- `process_bytes(...)` (pipeline.rs:63-82): runs `prepare_bytes` then encodes; dispatches on `CompressionMode::TargetFileSize { kilobytes }` (binary search) vs `Manual`. Returns `PreviewBytes { format, width, height, bytes }`.
- `prepare(source, settings)` (pipeline.rs:104-112, feature `fs`): disk version of `prepare_bytes`.
- `process_one(source, out_dir, settings)` (pipeline.rs:115-128, feature `fs`): `prepare` → `plan_output_path` (collision-suffixed filename) → `encode::write_to_path[_target_size]`.
- `process_one_to_bytes(source, settings)` (pipeline.rs:131-139): disk version of `process_bytes`, used by `preview_one`.
- `run_batch(sources, out_dir, settings, on_progress)` (pipeline.rs:145-194): create output dir up front, then `sources.par_iter()` (rayon) calling `process_one`. After each, `completed.fetch_add(1, SeqCst) + 1` then `on_progress(ProgressUpdate { completed, total, item: result.clone() })`. `on_progress: F where F: Fn(ProgressUpdate) + Sync + Send`.

```rust
sources.par_iter().map(|src| {
    let result = match process_one(src, out_dir, settings) {
        Ok(dest) => BatchItemResult { source, destination: Some(dest), error: None },
        Err(e)   => BatchItemResult { source, destination: None,       error: Some(format!("{e}")) },
    };
    let n = completed.fetch_add(1, Ordering::SeqCst) + 1;
    on_progress(ProgressUpdate { completed: n, total, item: result.clone() });
    result
}).collect()
```

`plan_output_path` (pipeline.rs:86-101) appends `_1`, `_2`, ... up to `_9999` to avoid clobbering.

**Op chain** (`crates/pixmill-core/src/ops/mod.rs:12-30`) — `apply_all`:

1. `orient::apply` if `preserve_exif` and EXIF orientation present.
2. `rotate::apply(image, settings.rotate)` — `None | Cw90 | Cw180 | Cw270 | FlipH | FlipV`.
3. `crop::apply(image, settings.crop)` — `None | AspectRatio { w, h } | Pixels { w, h }`, center-crop.
4. `resize::apply(image, settings.resize)` — `None | MaxLongEdge { pixels } | Percentage { percent }` via `fast_image_resize::Resizer` with Lanczos.

**Decode** (`decode.rs:17-31`): format-dispatched. JPEG/PNG/WebP go through `image::ImageReader::with_guessed_format()`. HEIC and AVIF are feature-gated (`heic`, `avif-decode`) and require system libs (`libheif`, `dav1d`).

**Encode** (`encode.rs`):

- `resolve_output_format(source, user_choice)` (encode.rs:14-28): `Keep` returns the source format, except HEIC/AVIF→JPEG per MVP design.
- `to_bytes` / `write_to_path` (encode.rs:31-56): in-memory and disk variants.
- `encode_jpeg`, `encode_png`, `encode_webp` (encode.rs:248+): per-format. WebP: lossless via `image::WebPEncoder::new_lossless` when `webp_quality == None`; lossy via the `webp` crate (feature `lossy-webp`) when `webp_quality == Some(q)`.
- Target-size mode does a 6-iteration binary search over quality 1–100 with target band `[target*0.9, target]`.

**Error types** (`error.rs`): `IbpError::{Io, UnsupportedFormat, Decode, Encode, Resize, InvalidSettings}`. Path-carrying variants include the source path for human-readable messages.

### Where the platforms diverge

| Aspect               | Tauri                                        | Web                                                                |
| -------------------- | -------------------------------------------- | ------------------------------------------------------------------ |
| Drop event source    | `webview.onDragDropEvent` (native, OS-level) | DOM `dragenter/dragover/dragleave/drop`                            |
| Path identity        | Real filesystem `String`                     | Synthetic id minted by worker for each `File`                      |
| Folder recursion     | Server-side (`walkdir` in `ingest_paths`)    | Client-side (`webkitGetAsEntry` recursion in `web/drop.ts:97-113`) |
| `recursive` flag     | Honored by `walkdir` (commands.rs:23)        | No-op (folders already expanded at drop time)                      |
| Heavy compute        | `spawn_blocking` + rayon `par_iter`          | Comlink worker calling `pixmill-wasm`                              |
| Settings persistence | `app_config_dir()/settings.json`             | localStorage                                                       |
| Output               | `write_to_path` into chosen directory        | ZIP download via blob                                              |
| Progress             | `Channel<ProgressUpdate>` IPC                | Comlink callback proxy                                             |

The `Platform` interface (`src/lib/platform/types.ts:51-73`) is what makes the rest of the app blind to which side it's running on. `DropZone.svelte`, `RunBar.svelte`, the stores, and the components never import from `tauri/` or `web/` directly.

## Code References

### Frontend (drag-drop + queue + invocation)

- `src/lib/components/DropZone.svelte:13-33` — `toQueueItem` mapping `RawMetadata` → `QueueItem`
- `src/lib/components/DropZone.svelte:35-45` — `ingest()` orchestration: platform call → queue.add → thumbnail kick-off
- `src/lib/components/DropZone.svelte:57-66` — `onMount` wiring of `setupDropHandler`
- `src/lib/platform/index.ts:8-10` — build-time platform selection
- `src/lib/platform/types.ts:51-73` — `Platform` interface contract
- `src/lib/platform/tauri/drop.ts:10-26` — Tauri webview drop event translation
- `src/lib/platform/tauri/index.ts:18-20` — `ingest` IPC wrapper
- `src/lib/platform/tauri/index.ts:69-83` — `runBatch` with `Channel<ProgressUpdate>`
- `src/lib/platform/web/drop.ts:46-69` — DOM drop handler with `webkitGetAsEntry` recursion
- `src/lib/stores/queue.svelte.ts:3-30` — `QueueStore` class (state + add/update/remove/clear)
- `src/lib/stores/batch.svelte.ts:1-30` — `BatchStore` class (running/completed/total/errors)
- `src/lib/thumbnails.ts:1-35` — `requestPendingThumbnails` with concurrency cap 4
- `src/lib/types.ts:3-23` — `QueueItemStatus` enum + `QueueItem` shape
- `src/lib/types.ts:65-69` — `BatchItemResult` shape (mirrors Rust)
- `src/lib/components/RunBar.svelte:19-63` — `run()` batch orchestration + progress threading
- `src/lib/components/RunBar.svelte:31-54` — `runBatch` call with `onProgress` callback

### Tauri shell

- `src-tauri/src/lib.rs:5-21` — command registration + plugin init
- `src-tauri/src/commands.rs:17-33` — `ingest_paths` walk → filter → dedupe → metadata
- `src-tauri/src/commands.rs:42-51` — `make_thumbnail` `spawn_blocking` wrapper
- `src-tauri/src/commands.rs:89-135` — `load_source` webview-native vs decode-and-reencode dispatch
- `src-tauri/src/commands.rs:139-156` — `preview_one` in-memory pipeline
- `src-tauri/src/commands.rs:175-192` — `run_batch` with `Channel<ProgressUpdate>` streaming
- `src-tauri/src/commands.rs:194-203` — `dedupe_keep_order`
- `src-tauri/src/persistence.rs:14-38` — settings.json read/write under `app_config_dir`
- `src-tauri/tauri.conf.json:20` — `dragDropEnabled: true`
- `src-tauri/capabilities/default.json` — granted permissions (dialog, opener, core)

### pixmill-core pipeline

- `crates/pixmill-core/src/lib.rs:1-19` — module declarations and public re-exports
- `crates/pixmill-core/src/ingest.rs:8-27` — `collect_from_dir` + `filter_supported`
- `crates/pixmill-core/src/metadata.rs:7-17` — `ImageMetadata` struct
- `crates/pixmill-core/src/metadata.rs:21-47` — `from_bytes` reader
- `crates/pixmill-core/src/metadata.rs:52-81` — `read` (disk) using `image::ImageReader::open(...).into_dimensions()`
- `crates/pixmill-core/src/pipeline.rs:13-28` — `BatchItemResult` + `ProgressUpdate` structs
- `crates/pixmill-core/src/pipeline.rs:43-60` — `prepare_bytes` (validate → exif → decode → ops → resolve_format)
- `crates/pixmill-core/src/pipeline.rs:63-82` — `process_bytes` with manual/target-size dispatch
- `crates/pixmill-core/src/pipeline.rs:86-101` — `plan_output_path` collision suffixing
- `crates/pixmill-core/src/pipeline.rs:115-128` — `process_one` (disk-write)
- `crates/pixmill-core/src/pipeline.rs:145-194` — `run_batch` rayon `par_iter` + atomic progress counter
- `crates/pixmill-core/src/ops/mod.rs:12-30` — `apply_all` canonical op order
- `crates/pixmill-core/src/decode.rs:17-31` — format-dispatched decoder
- `crates/pixmill-core/src/encode.rs:14-28` — `resolve_output_format` HEIC/AVIF→JPEG fallback
- `crates/pixmill-core/src/encode.rs:31-56` — `to_bytes` / `write_to_path`

## Architecture Documentation

**IPC boundaries.** The Tauri side does almost no work — `commands.rs` is a thin marshaller over `pixmill_core`. All heavy compute goes through `tauri::async_runtime::spawn_blocking` so the IPC dispatch thread stays free. Documented in CLAUDE.md:38-43.

**Streaming via Channel, not Events.** The codebase uses `tauri::ipc::Channel<T>` for one-way streaming from Rust to JS (commands.rs:180, tauri/index.ts:75-76). This is preferred over `tauri::Manager::emit` events per CLAUDE.md:43-45. The channel is created on the JS side, passed as a command argument, and `send`-ed to from worker threads inside the rayon batch.

**Wire-type symmetry.** All cross-IPC types use `#[serde(rename_all = "camelCase")]` on the Rust side (`ImageMetadata`, `ProgressUpdate`, `BatchItemResult`, `Settings`, `PersistedState`) so the TS types in `src/lib/types.ts` and `src/lib/platform/types.ts` line up 1:1. Enum variants are `#[serde(tag = "kind", rename_all = "camelCase")]` (mirrored in TS as discriminated unions).

**Platform abstraction.** `Platform` (platform/types.ts:51-73) is a single interface implemented twice (`tauri/index.ts:85-98`, `web/index.ts`). Build-time switch in `platform/index.ts:8-10` lets Vite dead-code-eliminate the unused backend. Components never reach across.

**Svelte 5 store pattern.** Every store is `class FooStore { x = $state(...) }; export const foo = new FooStore()`. Files end in `.svelte.ts`. Mutations look like `queue.items.push(x)`, `batch.completed = n`, `queue.update(id, patch)` — direct property writes that the `$state` rune tracks. No `$store` prefix; no `subscribe`. Documented in CLAUDE.md:46-49.

**Ingest filtering policy.** Supported input extensions live in three places that must agree:

- `src/lib/platform/tauri/index.ts:16` — `imageExtensions = [...]` for the dialog filter.
- `src/lib/platform/web/drop.ts:19` — `supportedExtensions = [...]`.
- `crates/pixmill-core/src/formats.rs` — `is_supported_input` (the authoritative filter used at ingest time).

**Concurrency model.**

- Thumbnail generation: JS-side, capped at 4 concurrent `make_thumbnail` IPC calls (thumbnails.ts:4).
- Batch processing: Rust-side, rayon `par_iter()` (default num_cpus), one `BatchItemResult` per file (pipeline.rs:171).
- Progress accounting: `AtomicUsize::fetch_add` for the completed counter, sent through `Channel` (pipeline.rs:168-186).

**Error surfacing.** Per-file errors never abort the batch — they're captured into `BatchItemResult.error` (pipeline.rs:179-183). The frontend interprets `update.item.error` to set the queue item's status to `error` (RunBar.svelte:39-46). Output-directory creation failure is the one fatal case: all sources get the same "could not create output dir" error (pipeline.rs:156-165).

**Settings flow.** `Settings` is one struct shared between `pixmill-core::settings::Settings` and `src/lib/types.ts:43-52`. It's passed verbatim through `invoke("run_batch", { settings, ... })`. Persistence is one file at `app_config_dir()/settings.json` (persistence.rs:14-21).

## Open Questions

- Watch-folder ingest is referenced in a comment on `read_metadata` (commands.rs:35: "useful after a watch-folder event in v2") but there's no code path for it yet.
- The `recursive` parameter to `platform.ingest` is honored by Tauri but ignored by the web build (folders are already expanded at drop time). This is documented in `src/lib/platform/web/index.ts:46-48` but worth flagging for anyone building features that toggle it.
- `dedupe_keep_order` is a HashSet+Vec — fine for current sizes, but the dedupe is on the Tauri side only. The frontend `queue.add` also dedupes by path (queue.svelte.ts:8) so re-drops are idempotent.
