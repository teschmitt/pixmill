# CLAUDE.md

Guidance for Claude Code working in this repo.

## What this is

A cross-platform desktop app (macOS / Windows / Linux) for batch resizing,
cropping, rotating images. Tauri 2 shell, SvelteKit + Svelte 5 frontend, Rust
image pipeline.

Full feature list and build instructions live in `README.md`. Backlog and phase
status live in `PLAN.md`. Read both before suggesting work.

## Stack at a glance

- **Tauri 2** — desktop shell, IPC via `invoke()` and `tauri::ipc::Channel<T>` for streaming
- **SvelteKit** with `adapter-static` (SPA mode, no SSR — Tauri has no Node runtime)
- **Svelte 5 runes** — `$state`, `$derived`, `$effect`, `$props`. NOT the legacy `$store` syntax.
- **Rust workspace** at the project root:
  - `crates/pixmill-core/` — image pipeline. No Tauri dependency. This is where logic lives and where tests live.
  - `crates/pixmill-wasm/` — wasm-bindgen bridge that re-exports `pixmill-core` for the browser build. Depends on `pixmill-core` with `default-features = false` to drop the `fs` and `lossy-webp` features that can't compile to `wasm32-unknown-unknown`.
  - `src-tauri/` — thin Tauri shell: IPC commands, persistence, watch folders, plugin wiring.

## Commands

```sh
pnpm install            # first time + after pulling deps
pnpm tauri dev          # run the desktop app
pnpm check              # svelte-check (type-check the frontend)
pnpm test               # vitest (frontend unit tests)
pnpm build              # build the static frontend (smoke test)
pnpm build:web          # wasm + VITE_PLATFORM=web static build
cargo test -p pixmill-core  # 27 image-pipeline tests
cargo check -p pixmill-core # fast type-check without GTK system libs
pnpm tauri build        # production bundle
make check              # fmt-check + lint + type-check + test (same as CI)
```

## Conventions

- **All image processing goes in `crates/pixmill-core/`**. The Tauri side calls it.
  Don't put `image::` or `fast_image_resize::` calls in `src-tauri/`.
- **Tauri commands are thin wrappers** in `src-tauri/src/commands.rs`. They convert
  IPC types, call into `pixmill_core::*`, and return. Heavy work goes through
  `tauri::async_runtime::spawn_blocking`.
- **Streaming progress** uses `tauri::ipc::Channel<T>`, not events. See
  `commands::run_batch` and the JS `runBatch` wrapper for the pattern.
- **Settings types are mirrored** between `crates/pixmill-core/src/settings.rs` (Rust)
  and `src/lib/types.ts` (TS). They must stay in sync. Both use `camelCase` on the
  wire — Rust enums are tagged via `#[serde(rename_all = "camelCase", tag = "kind")]`.
- **Stores are Svelte 5 classes**, e.g. `class QueueStore { items = $state<...>([]) }`,
  exported as singletons. Files end in `.svelte.ts`. Use `store.items` directly
  — no `$store` prefix.
- **Tests live with the code**: unit tests inline in `#[cfg(test)] mod tests`,
  integration tests in `crates/pixmill-core/tests/end_to_end.rs`. Add a test when
  you add a new pipeline op; the bar is "does this produce the right output on
  a real fixture image?"

## Releases

- **Label PRs to ship**: add `release:major`, `release:minor`, or `release:patch`
  before merging. The label drives the bump in `.github/workflows/release.yml`.
  No label = merge ships in the next labeled release's changelog. Pick `patch`
  for bug fixes, `minor` for new features, `major` for breaking changes.
- **Don't bump versions in PRs**. The release workflow does it on merge —
  bumping `package.json` / `Cargo.toml` manually creates merge conflicts and
  desyncs from `CHANGELOG.md`.
- **Version source of truth is `package.json`**. `Cargo.toml` mirrors it via
  `workspace.package.version`; `tauri.conf.json` reads it via Tauri 2's
  `"version": "../package.json"` indirection.
- **CHANGELOG.md is generated, not hand-edited**. `scripts/update-changelog.mjs`
  prepends each release's notes (sourced from GitHub's release-notes API,
  grouped by `.github/release.yml`). Only edit it for retroactive fixes.

## Things that will trip you up

- **WebP quality slider is lossy by default; `webp_quality: null` is the
  lossless escape hatch.** Lossy encoding goes through the `webp` crate
  (vendors libwebp via `libwebp-sys`). Lossless still works but is only
  reachable by setting `webpQuality: null` in stored settings JSON — no UI
  toggle.
- **AVIF and HEIC decode are behind Cargo features** (`avif-decode`, `heic`).
  They need `dav1d` and `libheif` system libs. Without the features, those
  formats are accepted into the queue but error on processing with a clear
  "unsupported format" message.
- **HEIC encode is intentionally not implemented.** HEIC/AVIF inputs convert to
  JPEG on output per MVP design (see `encode::resolve_output_format`).
- **Linux dev needs GTK system libs**: `webkit2gtk-4.1-dev`, `librsvg2-dev`, etc.
  Building `src-tauri/` on a fresh Linux box without these will fail at pkg-config.
  In a sandbox without sudo this blocks `cargo check` on `src-tauri/`. Use
  `cargo check -p pixmill-core` for fast iteration instead.
- **pnpm + multi-platform native bindings**: `pnpm-workspace.yaml` includes a
  `supportedArchitectures` block. Don't remove it — it's what makes the lockfile
  include darwin-arm64, win32-x64, etc. native bindings for `@tauri-apps/cli`
  and `esbuild`. If anyone hits "Cannot find native binding" they need to
  `rm -rf node_modules && pnpm install` on their machine.
- **Frontend uses SvelteKit but in SPA mode.** `+layout.ts` has `export const ssr = false`.
  Don't add server-side code or `+page.server.ts` files — there is no server.
- **Web vs. Tauri at build time**: `src/lib/platform/` has a `Platform` interface
  with `tauri/` and `web/` implementations. The active impl is picked from
  `import.meta.env.VITE_PLATFORM` (defaults to `tauri`), so Vite dead-code-
  eliminates the unused branch. Web-only capabilities check
  `platform.supportsWatchFolders` etc. before rendering UI. New filesystem-style
  APIs need a stub on the web side (typically a ZIP-based fallback).
- **`pixmill-core` must keep compiling to wasm**: `pixmill-wasm` depends on
  `pixmill-core` with `default-features = false`. Anything in core that pulls
  in `fs`, `walkdir`, `rayon`, or `libwebp-sys` belongs behind a feature flag
  (`fs`, `lossy-webp`, etc.), not in the default build. If you add a new
  Cargo dependency to `pixmill-core`, run `pnpm build:wasm` to verify.
- **Side-by-side preview** (`PreviewModal.svelte` + `stores/preview.svelte.ts`):
  the two panes share a normalized `(viewCenterX, viewCenterY, viewLevel)`
  state. Crop is a known caveat — the preview is a sub-region of the source,
  so "same normalized point" doesn't line up content-wise. Fixing it needs a
  crop-aware coordinate transform; see the comment in `PreviewModal.svelte`.
- **The `image` crate types**: a `DynamicImage` is the right working type. Pixel
  type detection for `fast_image_resize` uses `DynamicImage::pixel_type()` from
  the `IntoImageView` trait — that trait must be in scope at the call site.
- **Watch folders** (Tauri only — `platform.supportsWatchFolders` is false on
  web):
  - Backend lives in `src-tauri/src/watch.rs`. One `WatchManager` owns a
    `notify-debouncer-full` debouncer at a fixed 500ms timeout. New
    `WatchedFolder { path, recursive, autoProcess }` registrations call
    `debouncer.watcher().watch(&path, RecursiveMode::Recursive | NonRecursive)`.
    Persisted folders are seeded into the manager from `lib.rs`'s setup hook
    BEFORE the JS side calls `subscribe_watch_events`, so the very first
    subscribe attaches them all.
  - Events emitted to JS via a long-lived `Channel<WatchEvent>` (tagged enum:
    `fileAdded { folder, item }`, `fileRemoved { folder, path }`,
    `folderStatus { folder, status }`). Each subscribe replaces the channel.
    Backend tests stand in an `mpsc::Sender` via the public `EventSink` trait
    in `watch.rs`.
  - Overlap guard: `is_path_overlap(a, b)` is true iff one path is an ancestor
    (or equal) of the other, after `canonicalize` with lexical fallback for
    non-existent paths. Enforced at both `add_watched_folder` and
    `validate_output_dir` (called by the output-dir picker).
  - JS-side burst coalescer in `src/lib/autoBurstCoalescer.ts` batches
    `fileAdded` events per folder when `autoProcess` is on. Caps:
    `QUIET_MS=250` (reset on each push), `SIZE_CAP=50`, `AGE_MS=10_000`. A
    second burst on the same folder while one is in flight is deferred into
    `bucket.pending` and dispatched after the first resolves (B2 serialize).
  - Per-folder progress lives on `WatchedFolderUi.batchInFlight` and is owned
    by `processBurst.svelte.ts` (note the `.svelte.ts` extension — it uses
    `$state.snapshot`, so the file needs the Svelte compiler). It deliberately
    does NOT touch the global `batch` store, so a manual "Run batch" via
    `RunBar` and an auto-process burst never collide.
  - Modify-aware (Phase 4): `Modify(ModifyKind::Data(_))` is classified as
    `Added` so an in-place edit re-emits `FileAdded` for the same path. The
    JS `handleWatchEvent` in `src/routes/+page.svelte` then disambiguates:
    new path → enqueue; `done`/`error` → flip back to `pending`;
    `pending`/`processing` → no-op.
  - Recovery (Phase 4): `lib.rs` spawns a 30s `std::thread` polling task that
    snapshots `WatchManager::error_folders()` and calls
    `WatchManager::retry_folder(path)` for each. The same `retry_folder`
    method backs the `retry_watched_folder` command, exposed to the UI as a
    "Retry now" button on rows whose status is `error`. The button has a 2s
    cooldown to prevent spam while notify is busy attaching.
  - Pin: `notify-debouncer-full = "0.5"`. API quirk: call `watch`/`unwatch`
    directly on `&mut Debouncer` — `Debouncer::watcher()` is deprecated and
    now returns `()`, so older snippets like `debouncer.watcher().watch(...)`
    won't compile. `ErrorKind::MaxFilesWatch` is the variant for the Linux
    inotify exhaustion case.

## What's "done" vs what's not

See `PLAN.md` for the phase-by-phase status and the v2 backlog. The short version:
JPEG / PNG / WebP work end-to-end with resize/crop/rotate, parallel processing,
progress streaming, EXIF orientation, sticky settings, target-file-size
compression (binary-search on encoder quality), lossy + lossless WebP, watch
folders with optional auto-process, side-by-side preview, and a wasm web build
deployed via GitHub Pages. AVIF/HEIC decode is gated behind Cargo features and
needs system libs (`dav1d`, `libheif`).

Remaining v2 work in `PLAN.md`: strip-EXIF privacy option, full EXIF blob copy
on output (today only orientation is preserved), and named presets.

## When the user asks for a new feature

1. Check `PLAN.md` first — it might already be in the v2 backlog with notes.
2. Decide where it lives: a new op in `crates/pixmill-core/src/ops/` (most likely),
   a Tauri command in `src-tauri/src/commands.rs`, or UI in `src/lib/components/`.
3. If it touches `Settings`, update Rust _and_ TS together.
4. Add a test in `tests/end_to_end.rs` using `write_red_png` / `tempdir` helpers.
5. Frontend changes: run `pnpm check`. Pipeline changes: `cargo test -p pixmill-core`.
