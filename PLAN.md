# PLAN.md

Current development plan and backlog for Image Batch Processor.

## Status

MVP scaffolded and end-to-end functional for JPEG / PNG / WebP (lossy + lossless).
21 tests pass in `ibp-core`. Frontend type-checks clean. Smoke-tested on macOS
2026-05-20 — golden path (queue, settings persistence, batch run) works.

## Phases (all complete)

| #   | Phase                                             | Notes                                       |
| --- | ------------------------------------------------- | ------------------------------------------- |
| 1   | Scaffold Tauri 2 + SvelteKit + workspace          | `image-batch-processor/`                    |
| 2   | File ingestion (drag-drop, file/folder pickers)   | recursive folder option                     |
| 3   | Rust decode + thumbnail generation                | base64 data: URLs over IPC                  |
| 4   | Thumbnail grid UI                                 | CSS grid, hover-to-remove, status badges    |
| 5   | Settings panel (resize / crop / rotate / format)  | all controls live                           |
| 6   | Batch pipeline (rayon + Tauri `Channel` progress) | JPEG/PNG/WebP                               |
| 7   | AVIF + HEIC support                               | behind Cargo features `avif-decode`, `heic` |
| 8   | EXIF preservation                                 | orientation baked into pixels               |
| 9   | Progress UI + error handling                      | per-file + overall                          |
| 10  | Sticky settings persistence                       | JSON in `app_config_dir`                    |
| 11  | Cross-platform packaging                          | `tauri.conf.json` configured                |

## V2 backlog

Roughly in suggested implementation order, easiest/highest-value first.

### Target-file-size resize (currently stubbed in v2 type)

- **Why**: User picked this in MVP scoping; deferred because of complexity.
- **How**: In `ops/resize.rs`, add a new `ResizeMode::TargetFileSize { kilobytes }`
  variant. Implement via binary search on JPEG quality (and optional max
  long-edge). Cap iterations at ~6. Only valid for JPEG/WebP output.
- **Constraint**: Settings type currently doesn't have this variant — add to
  both Rust `settings.rs` and TS `types.ts`. Add a UI radio.

### Strip EXIF option

- **Why**: Privacy. The `preserve_exif` checkbox currently only controls
  orientation, but the name suggests it controls metadata copy too.
- **How**: When `preserve_exif = false`, still apply orientation (so output
  looks right) but mark this as "strip metadata". When `preserve_exif = true`,
  in a follow-up, also copy non-orientation EXIF tags. For now consider renaming
  the field to clarify scope (e.g. `apply_orientation` + separate
  `copy_exif_blob`).

### Full EXIF blob copy on output JPEG

- **Why**: User wants real EXIF preservation, not just orientation.
- **How**: Use the `little_exif` crate (purpose-built) to read EXIF from source,
  set Orientation to 1 (since pixels are already oriented), inject the modified
  blob as an APP1 segment in the output JPEG. WebP has an EXIF chunk too.

### Watch folder (deferred from MVP)

- **Why**: User wanted it but accepted v2 deferral.
- **How**: `notify = "6"` crate, watch a directory, debounce events ~500ms,
  feed new file paths into `ingest_paths`. UI: an "Add watch folder" button
  that toggles per-folder. Background task lives in `src-tauri/`.

### Named presets

- **How**: Settings persistence already serializes one snapshot. Extend to
  `{ presets: Vec<NamedPreset>, last_used: Settings }`. Add UI for save/load/delete.
  Suggested storage: same `settings.json`, key `presets`.

### Side-by-side preview (single image)

- **How**: When a card is clicked, open a modal/panel showing source + processed
  preview side by side using the current settings. Implementation: call a new
  Tauri command `preview_one(path, settings) -> data_url` that runs the pipeline
  in-memory and returns a JPEG. Reuse `ibp_core::pipeline::process_one` but
  return bytes instead of writing to disk.

### Smoke-test AVIF/HEIC on Mac

- **Pre-req**: `brew install dav1d libheif`
- Build with `--features "ibp-core/avif-decode ibp-core/heic"`
- Drop a HEIC from iPhone Photos, verify it converts to JPEG output.

## Known minor cleanups

- `image-batch-processor` window title currently uses kebab-case in
  `tauri.conf.json` — fine, but could be "Image Batch Processor" once branded.
- `src/lib/components/QueueList.svelte` is misnamed — it renders a grid now,
  not a list. Rename to `ThumbnailGrid.svelte`.
- `src-tauri/Cargo.toml` still has `authors = ["you"]` from the template.
- Icons in `src-tauri/icons/` are still the Tauri default. Replace before
  shipping.

## Out of scope (don't suggest)

These were explicitly excluded from the MVP and we shouldn't add them without
re-discussion with the user:

- Watermark (text or image overlay)
- Rename patterns
- "Overwrite originals" output mode
- Multiple GUI frameworks — we're committed to Tauri + SvelteKit
- Mobile / Tauri Android target
