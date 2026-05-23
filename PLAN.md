# PLAN.md

Current development plan and backlog for Pixmill.

## Status

v1.1.0 shipped (2026-05-23). The v1 backlog is done: JPEG / PNG / WebP
(lossy + lossless), target-file-size compression
(`CompressionMode::TargetFileSize`, binary-search on encoder quality, a
separate enum from `ResizeMode` since it's an encoder concern), watch folders
with optional auto-process, side-by-side preview modal (click a thumbnail to
see source vs. processed-with-current-settings), wasm web build deployed to
GitHub Pages, and bundled in-app docs at `/help`. 27 tests pass in
`pixmill-core`; frontend type-checks clean; CI gates `make check` on a built
wasm pkg.

## Phases (all complete)

| #   | Phase                                             | Notes                                                                                |
| --- | ------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 1   | Scaffold Tauri 2 + SvelteKit + workspace          | `pixmill/`                                                                           |
| 2   | File ingestion (drag-drop, file/folder pickers)   | recursive folder option                                                              |
| 3   | Rust decode + thumbnail generation                | base64 data: URLs over IPC                                                           |
| 4   | Thumbnail grid UI                                 | CSS grid, hover-to-remove, status badges                                             |
| 5   | Settings panel (resize / crop / rotate / format)  | all controls live                                                                    |
| 6   | Batch pipeline (rayon + Tauri `Channel` progress) | JPEG/PNG/WebP                                                                        |
| 7   | AVIF + HEIC support                               | behind Cargo features `avif-decode`, `heic`                                          |
| 8   | EXIF preservation                                 | orientation baked into pixels                                                        |
| 9   | Progress UI + error handling                      | per-file + overall                                                                   |
| 10  | Sticky settings persistence                       | JSON in `app_config_dir`                                                             |
| 11  | Cross-platform packaging                          | `tauri.conf.json` configured                                                         |
| 12  | Watch folder (live filesystem ingest)             | notify-debouncer-full; auto-process via JS burst coalescer; modify-aware + 30s retry |

## V2 backlog

Roughly in suggested implementation order, easiest/highest-value first.

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

### Named presets

- **How**: Settings persistence already serializes one snapshot. Extend to
  `{ presets: Vec<NamedPreset>, last_used: Settings }`. Add UI for save/load/delete.
  Suggested storage: same `settings.json`, key `presets`.

### Smoke-test AVIF/HEIC on Mac

- **Pre-req**: `brew install dav1d libheif`
- Build with `--features "pixmill-core/avif-decode pixmill-core/heic"`
- Drop a HEIC from iPhone Photos, verify it converts to JPEG output.

## Known minor cleanups

- Window title and `productName` in `tauri.conf.json` are now "Pixmill"
  (branded 2026-05-21). Earlier kebab-case placeholder is gone.
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
