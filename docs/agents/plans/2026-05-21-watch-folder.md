---
date: 2026-05-21T09:20:01+00:00
git_commit: ""
branch: ""
topic: "Watch folder (live filesystem ingest for the desktop build)"
tags: [plan, watch-folder, tauri, notify, ingest, pipeline, svelte]
status: draft
---

# PLAN: Watch folder

Add live filesystem-watch ingest to the Tauri build. The user can register one or more folders; new files appearing in them are picked up automatically and either added to the queue (`pending`) or processed immediately (when the per-folder `auto-process` flag is on). The web build does not ship this feature — it has no filesystem-watch primitive available in any browser as of 2026-05.

Source research: `docs/agents/research/2026-05-21-file-import-and-processing-pipeline.md`. Backlog entry being delivered: `PLAN.md` → "Watch folder (deferred from MVP)".

## Acceptance Criteria

- User can add multiple folders to a "Watch folders" section; the list persists across app restarts.
- Each folder has independent toggles for `recursive` and `auto-process`.
- New files appearing in a watched folder appear in the queue as `pending` automatically.
- When `auto-process` is on for a folder, new files are processed immediately into the current output directory (no manual "Run batch" click).
- Half-written files (during a long copy) are not ingested mid-write — only files stable for ~500ms emit events.
- A per-folder "Scan now" button does a one-shot ingest of all currently-existing supported files in that folder; a top-level "Scan all once" does the same across every watched folder.
- Adding a folder that is or contains the current output dir is rejected with an inline error; the same check fires if the output dir is changed to overlap an existing watched folder.
- Enabling `auto-process` requires an output dir; the toggle is disabled with a hint otherwise.
- Watchers resume automatically on app launch using the persisted list; if a folder is missing or unreadable, that row shows an error badge but the rest keep working.
- The "Watch folders" section is hidden in the web build (gated by a `platform.supportsWatchFolders` capability flag).
- Existing `settings.json` files (without the new field) continue to load without error.
- A burst of N files arriving in a watched folder within the coalesce window is processed as a single batch run, with one per-folder progress line in the UI — not N independent `runBatch([one])` invocations.
- A second burst arriving while one is in flight for the same folder is queued and dispatched after the first completes; no events are dropped.
- An in-place modification of a file already known to a watched folder (saved over by an external app) re-processes the file under auto-process, or returns the row to `pending` when auto-process is off.
- A previously-failed watched folder (e.g., USB drive unplugged) reattaches automatically within ~30s of becoming readable again; a manual "Retry now" button on the row triggers the same reattach immediately.
- A file deleted (or renamed away) from a watched folder while still `pending` is removed from the queue; files that already reached `processing`, `done`, or `error` stay so the historical record survives.

## Technical Key Decisions and Tradeoffs

1. **Folder count: multiple.** `Vec<WatchedFolder>` in state and persistence.
   - Why: PLAN.md already implies it ("toggles per-folder") and covers more workflows.
   - Impact: list-shaped state and UI; one `RecommendedWatcher` with multiple registered paths.

2. **On-detect action: per-folder `auto_process` boolean.**
   - Why: covers both intake (camera dumps, review-and-run) and drop-in conversion (screenshots, auto-process) workflows.
   - Impact: JS event handler branches between `queue.add(...)` and `queue.add(...) + runBatch([path])`.

3. **Persistence & startup: persist list; re-attach on launch; no auto-scan of pre-existing files; per-folder "Scan now" button.**
   - Why: predictable, never surprises the user with a backlog batch on startup.
   - Impact: extend `PersistedState`; "Scan now" reuses the existing `platform.ingest(...)` command — no new ingest command needed.

4. **File stability: `notify-debouncer-full`, 500ms timeout.**
   - Why: ecosystem-standard, coalesces per-path events, no hand-rolled stability polling.
   - Impact: one new transitive dep; filter to `Create` and `Rename(To)` events only.

5. **Validation: block invalid configs upfront.**
   - Why: an output-overlap configuration causes an infinite processing loop; auto-process without output dir silently discards work.
   - Impact: `add_watched_folder` + output-dir setter both run a shared `is_path_overlap` check; auto-process UI toggle is gated on output dir presence.

6. **UI placement: new `WatchFolders.svelte` directly below `DropZone.svelte`; hidden on web.**
   - Why: discoverable on Tauri without polluting the browser UI.
   - Impact: one new component, one new store, and a `supportsWatchFolders` capability flag on the `Platform` interface.

7. **IPC: a single persistent `Channel<WatchEvent>` registered via `subscribe_watch_events` on app mount; replaced on resubscribe.**
   - Why: keeps the established `Channel` pattern (CLAUDE.md preference over events) while supporting long-lived streaming.
   - Impact: `WatchManager` is held in `tauri::State<Arc<Mutex<_>>>`, owns the debouncer and the active channel; HMR-safe because resubscribing just replaces the channel reference.

8. **Wire type: watcher thread does its own `metadata::read` and emits `WatchEvent::FileAdded { folder, item: ImageMetadata }`.**
   - Why: avoids a per-file IPC round-trip; matches the shape JS already consumes from `ingest_paths`.
   - Impact: the watcher thread blocks briefly on disk for `metadata::read` per event — acceptable for ~hundreds of files at debounce cadence.

9. **Auto-process burst coalescer (JS-side).** Per-folder buckets; flush on quiet-window (250ms) OR size cap (50 paths) OR age cap (10s). Concurrent bursts on the same folder are serialized via `inflight` + `pending` arrays.
   - Why: per-file `runBatch([one])` wastes the rayon worker pool the pipeline already provides; quiet-window-only fails under steady drips (rsync, backup tools); concurrent in-flight bursts per folder would break the single-progress-line UI.
   - Impact: new `src/lib/autoBurstCoalescer.ts` module; `processBurst.ts` replaces the originally-considered `processSingleFile.ts`; auto-process is always batched.

10. **Per-folder batch state.** `WatchedFolderUi.batchInFlight?: { completed, total, errors }`, driven by `runBatch` progress callbacks.
    - Why: the lifetime `filesAdded` counter doesn't answer "how many of _this_ burst are done?"; the global `batch` store stays owned by `RunBar.run()`, so auto-process and manual batches never collide on shared state.
    - Impact: `WatchFolderRow.svelte` renders a compact progress bar when `batchInFlight` is `Some`; no sibling `autoBatch` store.

11. **"Scan now" honors `autoProcess`.** When a folder has `autoProcess: true`, "Scan now" pushes scanned paths into the burst coalescer rather than terminating at `queue.add`.
    - Why: `autoProcess` should mean "when this folder gains files, process them" regardless of _how_ the files were noticed; otherwise a backlog-convert requires toggling auto-process off, scanning, running batch, toggling back on.
    - Impact: `WatchFolderRow.svelte` "Scan now" handler branches on `folder.autoProcess`.

12. **Modify-aware reactivity.** `Modify(Data)` events on already-known paths flip `done`/`error` queue items back to `pending`; under auto-process, the modified path is fed through the coalescer.
    - Why: in-place edits (Photoshop save, ExifTool metadata write, screenshot re-save) otherwise leave the output dir stale, and `queue.add`'s path-dedupe means the user can't recover by drag-drop.
    - Impact: watcher filter extends to `Modify(Data)`; backend stays semantics-dumb and emits the same `FileAdded` variant; JS handler disambiguates by current queue status. **Deferred to Phase 4 per phasing decision #15.**

13. **Recovery from per-folder watch errors: 30s polling + manual "Retry now" button.**
    - Why: the camera-card / external-SSD workflow involves frequent plug-unplug cycles; "remove and re-add to retry" is unacceptable friction for the canonical user.
    - Impact: `WatchManager` owns a `tauri::async_runtime::spawn` polling task that retries error-state folders; new `retry_watched_folder(path)` command shared between polling and the row button. **Deferred to Phase 4 per phasing decision #15.**

14. **Stale-pending cleanup.** Filter `Remove` and `Rename(From)` events. If a `pending` queue item matches the stale path, drop it. Leave `processing`/`done`/`error` items untouched.
    - Why: a `pending` item pointing to a now-deleted file is a known-broken queue entry that will error at Run batch time. `done` items represent past work whose output may still exist — preserving them keeps the historical record.
    - Impact: watcher filter extends to `Remove` + `Rename(From)`; new `WatchEvent::FileRemoved { folder, path }` variant; JS handler runs `queue.items.find` by path and conditionally `queue.remove`.

15. **Phasing: four phases, defer modify-aware + retry policy to Phase 4.**
    - Why: the burst coalescer (#9), per-folder progress (#10), and scan-now-honors (#11) are tightly coupled to the auto-process design and must land in Phase 3. Modify-aware behavior (#12) and recovery polish (#13) are genuinely independent additions that can sit on top of a fully-functional Phase 3 and ship as a smaller follow-up.
    - Impact: Phase 1 unchanged; Phase 2 picks up stale-pending cleanup (#14) cheaply since the event filter is already being expanded; Phase 3 rewrites auto-process around the coalescer; Phase 4 = "edge-case polish" (modify + retry).

## Current State

The ingest pipeline today has three user-gesture entry points (drag-drop, file picker, folder picker) that all funnel into a single `platform.ingest(paths, recursive)` → `invoke("ingest_paths", ...)` → `queue.add` → `requestPendingThumbnails` flow. The Tauri shell holds no app-managed state today (`lib.rs:6-18` doesn't call `.manage(...)`). Persistence is one JSON file at `app_config_dir()/settings.json` holding `{ settings, output_dir }`.

```
                     USER GESTURES (today)
   ┌─────────────────┬───────────────────────────────────┐
   │  drag-drop      │  "Add files…" / "Add folder…"     │
   └────────┬────────┴───────────────────┬───────────────┘
            │                            │
            └──────────┬─────────────────┘
                       ▼
            DropZone.svelte ▶ platform.ingest(paths, recursive)
                       │
                       ▼  invoke("ingest_paths", ...)
       ┌───────────────────────────────────────────────┐
       │  commands::ingest_paths                        │
       │    walkdir → filter_supported → dedupe         │
       │    → metadata::read → Vec<ImageMetadata>       │
       └───────────────────┬───────────────────────────┘
                           ▼
              queue.add(items)  →  requestPendingThumbnails()
                           │
                           ▼ (user clicks "Run batch")
                  RunBar ▶ platform.runBatch(...)
```

No filesystem watcher exists; `commands.rs:35` already has the orphan `read_metadata` command with a comment "useful after a watch-folder event in v2", but no caller. `src-tauri/Cargo.toml:20-28` has no `notify` dependency.

## Desired End State

A fourth ingest entry point — a long-lived `WatchManager` in the Tauri shell — feeds the same `queue.add` / `runBatch` flow with no additional user gesture required after registration.

```
                     USER GESTURES (after)
   ┌──────────────┬──────────────────────┬─────────────────────────┐
   │  drag-drop   │  Add files / folder  │  Add watch folder       │
   └──────┬───────┴──────────┬───────────┴──────────────┬──────────┘
          │                  │                          │
          ▼                  ▼                          ▼
     (existing path)    (existing path)        add_watched_folder
                                                       │
                                                       ▼
                               ┌─────────────────────────────────────┐
                               │  WatchManager (src-tauri/watch.rs)  │
                               │   notify-debouncer-full @ 500ms     │
                               │   Vec<WatchedFolder>  (persisted)   │
                               │   Channel<WatchEvent>               │
                               └─────────────────┬───────────────────┘
                                                 │
              ┌──────────────────────────────────┴────────────────────────┐
              │ debouncer thread:                                          │
              │  filter Create / Rename(To) / Modify(Data) / Remove /     │
              │         Rename(From)                                       │
              │  for adds:    metadata::read → WatchEvent::FileAdded      │
              │  for removes: WatchEvent::FileRemoved { folder, path }    │
              │  status:      WatchEvent::FolderStatus { folder, status } │
              └──────────────────────────────────┬────────────────────────┘
                                                 ▼
                       JS subscribe_watch_events handler
                                                 │
   ┌─────────────────────────────────────────────┴──────────────────────────┐
   │  fileAdded:                                                            │
   │    path NOT in queue          → queue.add([item])                      │
   │    path IS done/error         → queue.update(id, status: "pending")   │
   │                                  (Phase 4: modify-aware)               │
   │    then, if folder.autoProcess → autoBurstCoalescer.push(folder, path) │
   │                                                                        │
   │  fileRemoved:                                                          │
   │    matching queue item is pending → queue.remove(id)                   │
   │    otherwise                       → no-op                              │
   │                                                                        │
   │  folderStatus: watchFolders.applyStatus(folder, status)                │
   └────────────────────────────────────────────────────────────────────────┘

                AutoBurstCoalescer (Phase 3)
   ┌──────────────────────────────────────────────────────────────────────┐
   │  per-folder Bucket = { paths, quietTimer, ageTimer, inflight, pending} │
   │  push():  reset 250ms quietTimer; start 10s ageTimer on first path;   │
   │           flush at 50 paths (size cap) OR when either timer fires;    │
   │           if inflight → append to pending instead                     │
   │  flush(): runBatch(paths, outDir, settings, onProgress)               │
   │           onProgress: update queue items + folder.batchInFlight       │
   │           on resolve: if pending non-empty, recurse (B2 serialize)    │
   └──────────────────────────────────────────────────────────────────────┘
```

`PersistedState` grows one field; the `Platform` interface gains five methods plus a capability flag; one new Rust module (`watch.rs`) and two new Svelte files (`WatchFolders.svelte`, `watchFolders.svelte.ts`) appear.

## Filesystem-event Backend

`notify-debouncer-full` wraps the `notify` crate, which uses native OS APIs — not polling, not direct inode tracking. We use `RecommendedWatcher` so the right backend is picked per platform:

| Platform | Backend                 | Push mechanism                                                                                                    |
| -------- | ----------------------- | ----------------------------------------------------------------------------------------------------------------- |
| macOS    | FSEvents                | Kernel stream, naturally recursive, path-keyed                                                                    |
| Linux    | inotify                 | Kernel events per directory; the kernel tracks watches by inode + watch descriptor internally — we never see them |
| Windows  | `ReadDirectoryChangesW` | Per-directory handle, supports recursive natively                                                                 |

Two platform-specific caveats the implementer needs to know:

- **Linux: inotify is non-recursive at the kernel level.** For `recursive: true`, `notify` walks the tree at watch time and adds one inotify watch per subdirectory. Newly-created subdirectories get watches attached automatically by the debouncer. The watch count is bounded by `/proc/sys/fs/inotify/max_user_watches` (default ~8192 on older distros, up to ~524288 on recent ones). Pointing at a huge tree (e.g. `~`) can fail with `ENOSPC`. If `notify` returns this error when registering a folder, surface it as `WatchFolderStatus::Error { message: "too many subdirectories to watch (Linux inotify limit reached)" }` rather than the raw error string.

- **Network mounts (SMB / NFS / WebDAV) do not emit reliable events** on either FSEvents or inotify. Files dropped on a remote share may go undetected. We do not work around this (no `PollWatcher` fallback in scope); the row's counter will simply stay at zero. Mention this in the README and in the inline error if we can detect it (e.g. on macOS the FSEvents stream silently no-ops; on Linux inotify will `EINVAL` for some remote FS types — surface as the same status-error variant).

We do **not** track inodes ourselves anywhere; the debouncer's path cache is keyed by `PathBuf`. Renames-in (`IN_MOVED_TO` on Linux, the equivalent on macOS/Windows) are still reported with the new path, so our `Create + Rename(To)` filter catches them without any inode bookkeeping.

## Abstractions and Code Reuse

Reused as-is:

- `pixmill_core::ingest::{collect_from_dir, filter_supported}` and `metadata::read` — the watcher thread calls these per stable event.
- `pixmill_core::formats::is_supported_input` — filters watch events to image files.
- `platform.ingest` — "Scan now" reuses the existing command rather than introducing a new one.
- `platform.runBatch` — auto-process invokes it with a one-element `paths` array; existing per-item progress, error capture, and queue updates apply unchanged.
- `queue.add` — dedupe-by-path makes repeat events idempotent (no special handling needed).

New surface:

- `crates/pixmill-core/`
  - `src/settings.rs` — add `WatchedFolder` struct.
    - `WatchedFolder { path: PathBuf, recursive: bool, auto_process: bool }` — wire-compatible (`#[serde(rename_all = "camelCase")]`).
  - `src/lib.rs` — re-export `WatchedFolder`.
- `src-tauri/`
  - `Cargo.toml` — add `notify-debouncer-full` dependency.
  - `src/persistence.rs` — extend `PersistedState`.
    - `PersistedState.watched_folders: Vec<WatchedFolder>` (with `#[serde(default)]` for migration).
  - `src/watch.rs` (NEW) — owns the live-watching logic.
    - `WatchManager` — holds debouncer, registered folder map, current `Channel<WatchEvent>`.
    - `WatchEvent` — tagged enum `FileAdded { folder, item }` | `FolderStatus { folder, status }`.
    - `WatchFolderStatus` — `Watching` | `Paused` | `Error { message }`.
    - `is_path_overlap(a, b)` — pure helper used by both add-folder and output-dir validation.
    - `start_watch_thread` — debouncer callback, filters events, runs `metadata::read`, sends to channel.
  - `src/commands.rs` — add new commands (or move them into `watch.rs` and re-export).
    - `add_watched_folder(app, folder) -> Result<(), String>` — validate (overlap + auto-process gate), persist, register with manager.
    - `remove_watched_folder(app, path) -> Result<(), String>`.
    - `set_watched_folder_config(app, path, recursive, auto_process) -> Result<(), String>`.
    - `subscribe_watch_events(state, channel) -> Result<(), String>` — store channel, ensure watcher is running for persisted folders.
    - `validate_output_dir(state, path) -> Result<(), String>` — used by output-dir picker to reject overlap.
    - `retry_watched_folder(state, path) -> Result<(), String>` — Phase 4: invokes `WatchManager::try_attach_watch(folder)` for a single folder; the result is reported back through the existing `FolderStatus` event rather than the return value.
  - `src/lib.rs` — register new commands; `.manage(Arc::new(Mutex::new(WatchManager::new())))`.
- `src/lib/`
  - `types.ts` — `WatchedFolder`, `WatchEvent`, `WatchFolderStatus`.
  - `platform/types.ts` — extend `Platform` interface.
    - `supportsWatchFolders: boolean`
    - `addWatchedFolder(folder)`, `removeWatchedFolder(path)`, `setWatchedFolderConfig(...)`, `subscribeWatchEvents(handler)`, `validateOutputDir(path)`
  - `platform/tauri/index.ts` — implement via `invoke(...)` and `new Channel<WatchEvent>()`.
  - `platform/web/index.ts` — `supportsWatchFolders: false`; methods throw `Error("not supported in web build")`. Never called because UI is hidden.
  - `stores/watchFolders.svelte.ts` (NEW) — singleton `class WatchFoldersStore` mirroring persisted list plus in-memory status counters.
    - `folders: $state<WatchedFolderUi[]>([])` where `WatchedFolderUi extends WatchedFolder { status, filesAdded, lastEventAt?, batchInFlight? }`.
    - `batchInFlight?: { completed: number; total: number; errors: number }` — populated by `processBurst` for the duration of one burst (decision #10).
    - `load()`, `add(folder)`, `remove(path)`, `setConfig(...)`, `applyEvent(event)`, `startBatch(path, total)`, `applyBurstProgress(path, update)`, `finishBatch(path)`.
  - `autoBurstCoalescer.ts` (NEW, Phase 3) — singleton class implementing decisions #9 (A3 flush policy) and #9 (B2 serialize). Per-folder buckets keyed by `folder.path`; `push(folderPath, filePath)`; internal `flush(folderPath)` calls `processBurst`.
  - `processBurst.ts` (NEW, Phase 3) — `processBurst(folderPath, paths[])`: reads `outputDir` + `settings`; flips queue items to `processing`; calls `platform.runBatch(paths, outDir, settings, onProgress)`; updates both queue items and `watchFolders.batchInFlight` per callback; never touches the global `batch` store.
  - `queueItem.ts` (NEW) — `toQueueItem(raw)` helper extracted from `DropZone.svelte:13-33` so `WatchFolderRow.svelte` and Phase-2 event handling share one implementation.
  - `components/WatchFolders.svelte` (NEW) — renders the list, "Add watch folder" button, "Scan all once" button.
  - `components/WatchFolderRow.svelte` (NEW) — one row: checkboxes, status badge, counter, `batchInFlight` progress bar (when present), Scan-now / Remove / Retry-now (Phase 4, conditional on error state) buttons, inline error.
- `src/routes/+page.svelte` — render `<WatchFolders />` below `<DropZone />`, gated by `{#if platform.supportsWatchFolders}`. Subscribe to watch events on mount.
- `pixmill/CLAUDE.md` — add a "Watch folders" subsection under "Things that will trip you up" (notify deps, libnotify isn't required on macOS but FSEvents has quirks with network volumes).
- `pixmill/PLAN.md` — move "Watch folder" out of V2 backlog into the completed-phases table.

## Logging & Observability

The watcher thread runs detached from any command invocation, so failures need a visible trail:

- **Frontend**: per-row `WatchFolderStatus` is the primary signal (badge in the UI). A counter shows how many files have been added since the row was created. Errors during ingest of a single file land in the queue as `error` items (existing behavior).
- **Backend**: use `eprintln!` for non-fatal warnings from the watcher thread (matches the codebase's current logging level — no `log` crate yet). One line per registered folder at start time, one line per `notify` error.

Example backend log lines:

```
[watch] registered /Users/t/Pictures/CameraDump (recursive=true, auto_process=false)
[watch] notify error for /Users/t/Pictures/CameraDump: NotFound
[watch] dropped channel; pausing 2 folders until next subscribe
```

No `log` crate is added in this plan — keeping logging consistent with the rest of the codebase. If logging needs grow during implementation, that's a follow-up.

## Implementation

### Phase 1: Foundation + manual "Scan now" (no live watching)

Dependencies: None.

Goal: ship the data model, persistence migration, Tauri commands, and the full UI — but no `notify` integration yet. "Scan now" works via the existing `platform.ingest(...)`; the live watcher is stubbed (commands accept folders and persist them, but no events are emitted).

**Tasks**:

- [x] Add `WatchedFolder { path: PathBuf, recursive: bool, auto_process: bool }` to `crates/pixmill-core/src/settings.rs` with `#[derive(Serialize, Deserialize, Clone, Debug)] #[serde(rename_all = "camelCase")]`. Re-export from `crates/pixmill-core/src/lib.rs`.
- [x] Extend `PersistedState` in `src-tauri/src/persistence.rs` with `pub watched_folders: Vec<WatchedFolder>`, annotated `#[serde(default)]` so existing `settings.json` files load without the field.
- [x] Create `src-tauri/src/watch.rs` containing:
  - [x] `WatchManager` struct (fields stubbed for now: `registered: HashMap<PathBuf, WatchedFolder>`, `channel: Option<Channel<WatchEvent>>`; no debouncer yet).
  - [x] `WatchEvent` and `WatchFolderStatus` enums (tagged `kind`, camelCase).
  - [x] `is_path_overlap(a: &Path, b: &Path) -> bool` — pure helper: true iff one is an ancestor of (or equal to) the other, after `canonicalize` (handle the `NotFound` case by falling back to lexical comparison).
  - [x] `WatchManager::add(folder)` / `::remove(path)` / `::set_config(...)` — mutate the in-memory map only (no notify wiring yet).
- [x] Add new commands in `src-tauri/src/commands.rs` (or in `watch.rs`, called from `commands.rs`):
  - [x] `add_watched_folder(app, state, folder: WatchedFolder) -> Result<(), String>` — validate overlap against current `output_dir` (load via `persistence::load`); reject `auto_process: true` when `output_dir` is `None`; append to `PersistedState.watched_folders` via `persistence::save`; register in `WatchManager`.
  - [x] `remove_watched_folder(app, state, path: String) -> Result<(), String>` — remove from persisted list, remove from manager.
  - [x] `set_watched_folder_config(app, state, path: String, recursive: bool, auto_process: bool) -> Result<(), String>` — update both persisted entry and manager.
  - [x] `validate_output_dir(state, path: String) -> Result<(), String>` — return Err if `path` overlaps any registered folder.
- [x] Register new commands and managed state in `src-tauri/src/lib.rs`: `.manage(Arc::new(Mutex::new(WatchManager::new())))` and add to `generate_handler![...]`. (Used `Mutex<WatchManager>` directly — `tauri::State` already gives us shared-borrow semantics; no need for an outer `Arc`.)
- [x] Add `WatchedFolder`, `WatchEvent`, `WatchFolderStatus` to `src/lib/types.ts`.
- [x] Extend `Platform` in `src/lib/platform/types.ts`:
  - [x] `supportsWatchFolders: boolean`
  - [x] `addWatchedFolder(folder: WatchedFolder): Promise<void>`
  - [x] `removeWatchedFolder(path: string): Promise<void>`
  - [x] `setWatchedFolderConfig(path: string, recursive: boolean, autoProcess: boolean): Promise<void>`
  - [x] `validateOutputDir(path: string): Promise<void>` (rejects on overlap)
  - [x] (`subscribeWatchEvents` is added in Phase 2 when the watcher actually emits.)
- [x] Implement those on `src/lib/platform/tauri/index.ts` as thin `invoke(...)` wrappers + `supportsWatchFolders = true`.
- [x] Stub them on `src/lib/platform/web/index.ts`: `supportsWatchFolders = false`; each method throws `new Error("Watch folders are not supported in the web build")`. Never called because UI is hidden.
- [x] Create `src/lib/stores/watchFolders.svelte.ts`:
  - [x] `class WatchFoldersStore { folders = $state<WatchedFolderUi[]>([]) }` exporting a singleton `watchFolders`.
  - [x] `WatchedFolderUi` extends `WatchedFolder` with `{ status: WatchFolderStatus, filesAdded: number, lastEventAt?: number }`.
  - [x] `load(initial: WatchedFolder[])`, `add(folder)`, `remove(path)`, `setConfig(...)`, `applyEvent(event)` (no-op until Phase 2 emits events). (Also `startBatch/applyBurstProgress/finishBatch` stubs ready for Phase 3.)
- [x] Create `src/lib/components/WatchFolderRow.svelte`:
  - [x] Props: `folder: WatchedFolderUi`.
  - [x] Renders path, recursive + auto-process checkboxes, status badge, counter, "Scan now" button, "Remove" button, inline error slot.
  - [x] "Scan now" calls `platform.ingest([folder.path], folder.recursive)` then `queue.add(items.map(toQueueItem))` and `void requestPendingThumbnails()` — same code path as `DropZone.ingest`. Pull `toQueueItem` out of `DropZone.svelte` into `src/lib/queueItem.ts` so both components share it.
  - [x] Auto-process checkbox is disabled when `outputDir` is null; tooltip "Set an output folder first".
- [x] Create `src/lib/components/WatchFolders.svelte`:
  - [x] Header "Watch folders".
  - [x] Render `{#each watchFolders.folders as folder}` → `<WatchFolderRow folder={folder} />`.
  - [x] "Add watch folder…" button: calls `platform.pickFolder()`, then `platform.addWatchedFolder({ path, recursive: true, autoProcess: false })`; catches errors and shows inline.
  - [x] "Scan all once" button: loops over `watchFolders.folders` and runs the same scan logic.
- [x] Modify `src/routes/+page.svelte`: render `{#if platform.supportsWatchFolders}<WatchFolders />{/if}` below `<DropZone />`. (Seeding happens in `+layout.svelte` since that's where existing `loadSettings` is wired.)
- [x] Update the output-dir picker flow (wherever `pickOutputFolder()` is wired to settings save — check `src/lib/components/RunBar.svelte` or settings panel; likely a sibling): before saving the new dir, call `platform.validateOutputDir(newPath)` and surface the Err to the user inline.

**Automated Verification**:

- [x] `cargo test -p pixmill-core` — existing 27 tests still pass. (11 unit + 16 e2e.)
- [x] New `src-tauri/tests/persistence_migration.rs` (or inline in `persistence.rs`): deserialize a `settings.json` blob _without_ `watchedFolders` succeeds with `watched_folders: vec![]`. (Inline in `persistence.rs`; **will run on macOS** — Linux sandbox can't compile `src-tauri` due to missing GTK system libs per CLAUDE.md.)
- [x] New unit test in `src-tauri/src/watch.rs`: `is_path_overlap` returns true for identical paths, true for ancestor/descendant in both directions, false for siblings, false for cousins. Verify with both existing paths (tempdirs) and non-existent paths (lexical fallback). (Written; **will run on macOS**.)
- [x] New unit test in `src-tauri/src/watch.rs`: `WatchManager::add` rejects a folder that overlaps a given output dir; `::add` rejects `auto_process: true` when output dir is None. (Written; **will run on macOS**.)
- [x] `pnpm check` — frontend type-checks clean.
- [x] `pnpm build` — static frontend builds without errors. Tauri AND web variants both build clean.

**Manual Verification**:

- [ ] Run `pnpm tauri dev`. Section "Watch folders" appears below the drop zone. The list is empty.
- [ ] Click "Add watch folder…", pick a directory with a few JPEGs in it. Row appears. Click "Scan now". Queue fills with the JPEGs. Thumbnails generate.
- [ ] Try to add the same folder again — should be deduped via the path equality check (or rejected with a clear message; specify which during implementation).
- [ ] Try to enable auto-process before setting an output dir — checkbox is disabled, tooltip explains.
- [ ] Set an output dir, then try to add a watch folder that equals it — rejected with inline error.
- [ ] Quit and relaunch the app. The watch-folder row is still there.
- [ ] Run the web build (`VITE_PLATFORM=web pnpm build` then preview): the "Watch folders" section is not rendered.

---

### Phase 2: Live watching (auto-process off)

Dependencies: Phase 1.

Goal: wire up `notify-debouncer-full`. New files in registered folders appear in the queue automatically. `auto_process` is still ignored on the JS side — Phase 3 enables it.

**Tasks**:

- [x] Add `notify-debouncer-full` to `src-tauri/Cargo.toml` `[dependencies]`. The latest stable release at implementation time; pin a minor version, e.g. `notify-debouncer-full = "0.5"` (verify and adjust at implementation time — must be compatible with whichever `notify` version it transitively pulls in).
- [x] Flesh out `WatchManager` in `src-tauri/src/watch.rs`:
  - [x] Field `debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>`.
  - [x] `ensure_running(&mut self)`: lazily constructs the debouncer with `new_debouncer(Duration::from_millis(500), None, callback)`; the callback ships events to an internal channel (`std::sync::mpsc::Sender<DebounceEventResult>`) consumed by a thread spawned in `ensure_running` that processes events and forwards `WatchEvent`s to the active `Channel<WatchEvent>`.
  - [x] On every `add`: call `debouncer.watcher().watch(&folder.path, recursive_mode)` where `recursive_mode` is `RecursiveMode::Recursive` or `::NonRecursive`. On `remove`: `unwatch(&path)`.
  - [x] On `subscribe`: store the new channel; if no debouncer yet, build it now; re-register all currently persisted folders (idempotent because `notify` rejects duplicates silently — handle the error). (Plus a `setup` hook in `lib.rs` seeds persisted folders into the manager before `subscribe` is even called, so the very first subscribe attaches them all.)
- [x] Implement the event processing thread:
  - [x] Filter `DebouncedEvent`s to "add-like" (`EventKind::Create(_)` and `EventKind::Modify(ModifyKind::Name(RenameMode::To))`) and "remove-like" (`EventKind::Remove(_)` and `EventKind::Modify(ModifyKind::Name(RenameMode::From))`).
  - [x] For each path: drop unsupported extensions via `pixmill_core::formats::is_supported_input`.
  - [x] Identify which registered folder the path belongs to (`path.starts_with(folder.path)` for each registered folder; the first match wins, but log a warning if multiple match because that means folders overlap — which should already be prevented at add time but a defense-in-depth check is cheap).
  - [x] For add-like events: run `pixmill_core::metadata::read(path)`. Emit `WatchEvent::FileAdded { folder, item }`.
  - [x] For remove-like events: emit `WatchEvent::FileRemoved { folder, path }`.
  - [x] If `channel.send` errors (channel dropped), keep the watcher running — a future `subscribe_watch_events` will install a new channel. (`EventSink::send` returns `()` and ignores Err on the underlying `Channel::send`.)
- [x] Handle `notify` errors per folder: if `watch(...)` returns `Err`, store the error on the folder's status and emit `WatchEvent::FolderStatus { folder, status: Error { message } }`. Phase 4 adds automatic retry + manual "Retry now" button (decision #13); for now Phase 2 ships the error badge only.
- [x] Add `subscribeWatchEvents(handler: (event: WatchEvent) => void): Promise<void>` to the `Platform` interface; implement on Tauri (creates `new Channel<WatchEvent>()`, sets `channel.onmessage = handler`, invokes `subscribe_watch_events`) and stub on web (throws).
- [x] On the JS side, register the subscription on app boot:
  - [x] In `src/routes/+page.svelte`'s mount block, call `platform.subscribeWatchEvents(handleWatchEvent)`. Gated by `platform.supportsWatchFolders` so the web build doesn't try to call the throwing stub.
  - [x] `handleWatchEvent(event)`:
    - `kind: "fileAdded"` → `queue.add([toQueueItem(event.item)])`; `watchFolders.applyEvent(event)`; `requestPendingThumbnails()`.
    - `kind: "fileRemoved"` (decision #14) → look up the queue item by `event.path`; if found and its `status === "pending"`, call `queue.remove(id)`.
    - `kind: "folderStatus"` → `watchFolders.applyEvent(event)`.
  - [x] Defer auto-process dispatch to Phase 3 — for now, every `fileAdded` event lands in the queue as `pending`.
- [x] Update `WatchFolderRow.svelte` to render the live status badge and the live counter. (Already implemented in Phase 1; reads `folder.status.kind` reactively.)

**Automated Verification**:

- [x] `cargo test -p pixmill_lib --tests`: new integration test `src-tauri/tests/watch_integration.rs` covers (a) `FileAdded` within 2s of a new JPEG appearing and (b) `Modify(Data)` does NOT re-emit `FileAdded` in Phase 2. (Test written using a public `EventSink` trait so an `mpsc::Sender` can stand in for the production `tauri::ipc::Channel`. **Will run on macOS** — Linux sandbox can't link against GTK system libs.)
- [x] `pnpm check` passes.
- [x] `cargo check` for the workspace (skip `src-tauri` on Linux sandbox per CLAUDE.md; will be verified on macOS).

**Manual Verification**:

- [ ] `pnpm tauri dev`. Add a watch folder pointing at an empty tempdir.
- [ ] In Finder, copy a JPEG into that folder. Within ~500ms, a `pending` item appears in the queue with a thumbnail.
- [ ] Copy a 30MB file (or simulate a slow copy with `dd ... bs=1M` over several seconds). Verify only one `pending` item appears, after the copy finishes — no mid-write decode errors.
- [ ] Delete the JPEG (still `pending`) from the watched folder. Within ~500ms the queue row disappears (decision #14).
- [ ] Add another JPEG, click "Run batch" until it's `done`, then delete the file from the folder. The `done` row remains in the queue (historical record preserved).
- [ ] Rename a `pending` file inside the watched folder. The old-name row disappears; a new row for the new name appears.
- [ ] Remove the watch folder. Copy another image into it; no item appears in the queue.
- [ ] Delete the underlying folder while watching. Status badge changes to `error`. Other watched folders keep working.

---

### Phase 3: Auto-process with burst coalescer + overlap guards

Dependencies: Phase 2.

Goal: honor `autoProcess` via a JS-side burst coalescer (decisions #9–11). Add per-folder progress state. Validate overlap when output dir is set or changed. Modify-aware behavior and retry policy come in Phase 4.

**Tasks**:

- [x] Create `src/lib/autoBurstCoalescer.ts`:
  - [x] `type Bucket = { paths: string[]; quietTimer: ReturnType<typeof setTimeout> | null; ageTimer: ReturnType<typeof setTimeout> | null; inflight: boolean; pending: string[] }`.
  - [x] `class AutoBurstCoalescer` with private `buckets = new Map<string, Bucket>()` keyed by `folder.path`.
  - [x] Tunables: `QUIET_MS = 250`, `SIZE_CAP = 50`, `AGE_MS = 10_000`.
  - [x] `push(folderPath: string, filePath: string): void`:
    - Look up or create the bucket.
    - If `bucket.inflight`: `bucket.pending.push(filePath)`; return.
    - `bucket.paths.push(filePath)`. Clear and restart `quietTimer` (fires `flush(folderPath)` after `QUIET_MS`).
    - If this is the first path (`bucket.paths.length === 1`), start `ageTimer` (fires `flush(folderPath)` after `AGE_MS`).
    - If `bucket.paths.length >= SIZE_CAP`, call `flush(folderPath)` synchronously.
  - [x] `private async flush(folderPath: string): Promise<void>`:
    - Clear both timers. Snapshot `bucket.paths`, set `bucket.paths = []`, set `bucket.inflight = true`.
    - `await processBurst(folderPath, snapshot)`.
    - In `finally`: `bucket.inflight = false`; if `bucket.pending.length > 0`, splice pending into `paths` and call `flush(folderPath)` again (B2 serialization).
  - [x] Export `export const autoBurstCoalescer = new AutoBurstCoalescer()`.
- [x] Create `src/lib/processBurst.ts` — landed as `src/lib/processBurst.svelte.ts` because it uses `$state.snapshot`, which the Svelte compiler resolves only in `.svelte.ts` files. Imports now use `$lib/processBurst.svelte`.
  - [x] `export async function processBurst(folderPath: string, paths: string[]): Promise<void>`.
  - [x] Read `settings.outputDir` and `settings.current` from stores. If `outputDir` is null, mark each path's queue item as `{ status: "error", error: "output folder not set" }` and return.
  - [x] For each path: `queue.update(path, { status: "processing", error: undefined })`.
  - [x] `watchFolders.startBatch(folderPath, paths.length)` (sets `batchInFlight = { completed: 0, total: paths.length, errors: 0 }`).
  - [x] `try { await platform.runBatch(paths, outputDir, settings.current, (update) => { ... }) }`. The `onProgress` callback:
    - Updates the matching queue item with `done` + `destination` (or `error` + message).
    - Calls `watchFolders.applyBurstProgress(folderPath, update)` (advances `batchInFlight.completed` / `errors`).
  - [x] `finally { watchFolders.finishBatch(folderPath) }` — clears `batchInFlight`.
  - [x] Does NOT touch the global `batch` store (decision #10).
- [x] Extend `WatchFoldersStore` in `src/lib/stores/watchFolders.svelte.ts`:
  - [x] `WatchedFolderUi` gains `batchInFlight?: { completed: number; total: number; errors: number }`.
  - [x] `startBatch(folderPath: string, total: number)`, `applyBurstProgress(folderPath: string, update: ProgressUpdate)`, `finishBatch(folderPath: string)` methods.
- [x] Extend `handleWatchEvent` in `src/routes/+page.svelte` for `kind: "fileAdded"`:
  - [x] After `queue.add(...)`, look up the source folder by path. If `folder.autoProcess === true`, call `autoBurstCoalescer.push(folder.path, event.item.path)`.
  - [x] If `autoProcess` is false, behavior is unchanged from Phase 2 (item sits as `pending`).
- [x] Extend `WatchFolderRow.svelte` "Scan now" handler (decision #11):
  - [x] After `platform.ingest([folder.path], folder.recursive)`, run `queue.add(items.map(toQueueItem))` (existing behavior).
  - [x] If `folder.autoProcess === true`: also push each returned path into `autoBurstCoalescer` so the backlog gets processed.
- [x] Render `batchInFlight` in `WatchFolderRow.svelte`:
  - [x] When `folder.batchInFlight` is `Some`, render a compact progress bar with text `"Processing {completed}/{total}{ · {errors} error(s)}"`.
  - [x] When `batchInFlight` is `None`, show the static `filesAdded` lifetime counter as before.
- [x] Verify Phase 1's overlap validation surfaces actionable error messages: e.g., `"watch folder '/foo' overlaps the output directory '/foo/out'"`. (Confirmed in `watch.rs:67,73`.)
- [x] Polish: error toasts in `WatchFolders.svelte` for failed add operations (overlap, permission). Loading state on "Scan now" while the ingest call is in flight.
- [x] Update `pixmill/CLAUDE.md`: add a "Watch folders" subsection under "Things that will trip you up" mentioning `notify-debouncer-full`, the 500ms debounce, `RecursiveMode`, the overlap guard, the burst coalescer (quiet/size/age caps), and the per-folder `batchInFlight` state.

**Automated Verification**:

- [x] `cargo test -p pixmill_lib --tests` passes including new unit tests in `watch.rs`: `add_watched_folder` overlap rejection (identical / ancestor / descendant; siblings & cousins Ok) and `validate_output_dir` overlap rejection. (Tests written; **will run on macOS** — Linux sandbox can't link against GTK.)
- [x] New JS test for `AutoBurstCoalescer` (vitest): all four scenarios — quiet-window single flush, size-cap (50) + remainder, age-cap (10s) drip, inflight serialization. 4/4 pass via `pnpm test`.
- [x] `pnpm check` passes.

**Manual Verification**:

- [ ] `pnpm tauri dev`. Set output dir to `~/Pictures/Out`. Add a watch folder at `~/Pictures/CameraDump`, recursive = true, auto-process = true.
- [ ] Drop a single JPEG into the watch folder. Within ~1–2 seconds the folder row shows `Processing 1/1`, then clears; the processed output appears in `~/Pictures/Out`.
- [ ] Drop 20 JPEGs into the watch folder at once. The folder row shows `Processing N/20` advancing as the batch progresses (one progress bar, not 20 independent ones); all complete; all outputs appear.
- [ ] Drop 20 more files while the first 20 are still processing. They sit silently until the first batch finishes, then a second `Processing 0/20` appears (B2 serialization).
- [ ] On a folder with auto-process ON and files already present, click "Scan now". The existing files are processed (not just enqueued) per decision #11.
- [ ] Try to set the output dir to a path inside (or equal to) an existing watched folder — the picker shows an inline error and doesn't save.
- [ ] Try to add a watch folder that contains the output dir — same inline error.
- [ ] Add two watch folders both with auto-process. Drop a JPEG in each in quick succession — two independent `Processing 1/1` lines, both complete; the global `RunBar` `batch` store stays untouched throughout.
- [ ] Disable auto-process on a folder; drop a JPEG; it goes to `pending`, not `processing`. The folder row's `filesAdded` counter increments but no `batchInFlight` bar appears.

---

### Phase 4: Modify-aware reactivity + recovery polish

Dependencies: Phase 3.

Goal: pick up in-place edits to known files (decision #12). Recover automatically from transient watcher errors via 30s polling + manual "Retry now" button (decision #13).

**Tasks**:

- [x] Extend the watcher filter in `src-tauri/src/watch.rs` to additionally accept `EventKind::Modify(ModifyKind::Data(_))` for paths that fall inside a registered folder. Emit the same `WatchEvent::FileAdded` variant — the backend stays semantics-dumb; the JS handler disambiguates by current queue status.
- [x] Extend `handleWatchEvent` in `src/routes/+page.svelte` for `kind: "fileAdded"`:
  - [x] If the path is NOT already in the queue: existing Phase 3 behavior (`queue.add`; if `autoProcess`, push to coalescer).
  - [x] If the path IS already in the queue with status `done` or `error`: `queue.update(id, { status: "pending", error: undefined, destination: undefined })`. If the source folder has `autoProcess: true`, also push to the coalescer.
  - [x] If the path IS already in the queue with status `pending` or `processing`: no-op (already going to be processed or being processed).
- [x] Add a polling retry loop. Implemented as a `std::thread::spawn` in `lib.rs`'s setup hook rather than `tauri::async_runtime::spawn` — no need for async since `std::thread::sleep` works just as well and avoids pulling tokio's `time` feature in as a direct dep.
  - [x] Loops with `std::thread::sleep(RETRY_POLL_INTERVAL)` where `RETRY_POLL_INTERVAL = 30s`.
  - [x] Each tick: snapshot `WatchManager::error_folders()` (brief lock), release, then re-acquire per-folder and call `retry_folder(&path)`. Per-folder lock means user commands only ever wait for ONE folder's `watch()` syscall, not the full iteration.
  - [x] `retry_folder` → `try_attach_watch` updates `folder_status` and emits a fresh `FolderStatus` event with `Watching` on success or refreshed `Error { message }` on failure.
- [x] Add `retry_watched_folder(state, path: String) -> Result<(), String>` command in `src-tauri/src/commands.rs` — calls `WatchManager::retry_folder`. Result lands via `FolderStatus`, not the return value.
- [x] Register the new command in `src-tauri/src/lib.rs`.
- [x] Add `retryWatchedFolder(path: string): Promise<void>` to the `Platform` interface; Tauri impl invokes `retry_watched_folder`; web impl throws via `notSupported()`.
- [x] Extend `WatchFolderRow.svelte`: when `folder.status.kind === "error"`, render a "Retry now" button before "Scan now". Click → `platform.retryWatchedFolder(folder.path)`. 2s cooldown via `retryCooldown` state to prevent spam.
- [x] Update `pixmill/CLAUDE.md`: extend the "Watch folders" subsection with notes on the modify-aware behavior, the polling retry interval (30s), and the `retry_watched_folder` command.
- [x] Update `pixmill/PLAN.md`: move "Watch folder" out of V2 backlog into the completed-phases table (one new row covering Phases 1–4).

**Automated Verification**:

- [x] New integration test in `src-tauri/tests/watch_integration.rs` for modify-aware (`watcher_emits_file_added_on_data_modify` — replaces the Phase 2 "is_ignored" test).
- [x] New integration test for retry (`retry_watched_folder_attaches_after_creation`). Both written; **will run on macOS** — Linux sandbox can't link against GTK.
- [x] `pnpm check` passes.

**Manual Verification**:

- [ ] `pnpm tauri dev`. Watch folder with auto-process OFF. Add a JPEG; it lands as `pending`. Click "Run batch"; it processes to `done`. Edit the JPEG in an external app and save over the top. Within ~1s, the queue row flips back to `pending`. Click "Run batch" again; the output reflects the edit.
- [ ] Same setup with auto-process ON. Add a JPEG; it processes automatically. Edit and save. Within ~1s, the row flips to `pending`, then `processing`, then `done`; the output in the output dir reflects the edit.
- [ ] Add a watch folder on an external drive (or removable USB stick). Unplug the drive. The row's status badge changes to `error` within ~1s. Plug the drive back in. Within ~30s, the status badge changes back to `watching` without user intervention; a JPEG added after replug appears as expected.
- [ ] Same as above, but click "Retry now" within seconds of replugging. The status flips immediately rather than waiting for the next poll.

## Implementation Notes

- **`processBurst.ts` was renamed to `processBurst.svelte.ts`** because it uses
  `$state.snapshot`, which the Svelte compiler resolves only in `.svelte.ts`
  files. All imports use `$lib/processBurst.svelte`.
- **`EventSink` trait** was added to `src-tauri/src/watch.rs` as a public
  abstraction so integration tests can capture events through an
  `mpsc::Sender` instead of a real `tauri::ipc::Channel`. The test crate
  defines its own `MpscSink: EventSink`.
- **Setup hook in `src-tauri/src/lib.rs`** seeds persisted folders into the
  `WatchManager` at app boot via `WatchManager::seed(folders)` (bypasses
  validation — persisted state was already validated when added).
- **Phase 4 polling task** uses `std::thread::spawn` + `std::thread::sleep`
  rather than `tauri::async_runtime::spawn` + `tokio::time::sleep`. Same
  semantics, no need for an async runtime, and avoids enabling tokio's
  `time` feature as a direct dep.
- **Per-folder lock discipline** in the retry task: snapshot
  `error_folders()` under one brief lock, release, then re-acquire per
  folder before calling `retry_folder`. The `watch()` syscall happens
  inside `try_attach_watch` while the lock is held, but only for one
  folder at a time — user commands never wait for the full iteration.
- **Test file `autoBurstCoalescer.test.ts`** required two fixes the original
  author missed: (a) `processBurstMock` needed `vi.fn<typeof processBurst>`
  (not `typeof defaultImpl`) so `mock.calls[i][1]` type-checks; (b) the
  age-cap test needed `period = 220` (< `QUIET_MS`) so the quiet timer
  never wins, AND `Date.now()` for capturing elapsed time rather than the
  loop-tracked `cursor`, which lags one step behind mocked time during
  `vi.advanceTimersByTimeAsync`.

## References

- Research: `docs/agents/research/2026-05-21-file-import-and-processing-pipeline.md`
- Backlog source: `PLAN.md` → "Watch folder (deferred from MVP)"
- Existing entry point: `src/lib/components/DropZone.svelte:35-45` (ingest function — pattern to mirror)
- Existing Channel<T> pattern: `src-tauri/src/commands.rs:175-192` and `src/lib/platform/tauri/index.ts:69-83` (per-invoke channel — adapted to long-lived here)
- Persistence shape: `src-tauri/src/persistence.rs:7-12` (extended with `watched_folders`)
- Capability surface: `src/lib/platform/types.ts:51-73` (Platform interface — extended with `supportsWatchFolders` + watch methods)
- `notify` / `notify-debouncer-full`: <https://docs.rs/notify-debouncer-full>
- Conventions: `pixmill/CLAUDE.md`
