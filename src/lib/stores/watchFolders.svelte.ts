import type { ProgressUpdate } from "$lib/platform/types";
import type { WatchedFolder, WatchEvent, WatchFolderStatus } from "$lib/types";

export interface WatchedFolderUi extends WatchedFolder {
  status: WatchFolderStatus;
  filesAdded: number;
  lastEventAt?: number;
  // Populated by `processBurst` for the duration of one auto-process burst
  // (Phase 3). Render a per-folder progress bar when present.
  batchInFlight?: { completed: number; total: number; errors: number };
}

class WatchFoldersStore {
  folders = $state<WatchedFolderUi[]>([]);

  /// Seed the store from persisted state. Status defaults to `watching`
  /// optimistically — the live attach in Phase 2 will downgrade to `error`
  /// for folders that fail to register.
  load(initial: WatchedFolder[]) {
    this.folders = initial.map((f) => ({
      ...f,
      status: { kind: "watching" },
      filesAdded: 0,
    }));
  }

  add(folder: WatchedFolder) {
    if (this.folders.some((f) => f.path === folder.path)) return;
    this.folders.push({
      ...folder,
      status: { kind: "watching" },
      filesAdded: 0,
    });
  }

  remove(path: string) {
    const idx = this.folders.findIndex((f) => f.path === path);
    if (idx >= 0) this.folders.splice(idx, 1);
  }

  setConfig(path: string, recursive: boolean, autoProcess: boolean) {
    const f = this.folders.find((f) => f.path === path);
    if (f) {
      f.recursive = recursive;
      f.autoProcess = autoProcess;
    }
  }

  /// Apply a backend-emitted event to the matching folder row. No-op for
  /// events whose `folder` field doesn't match any known row (e.g. a
  /// straggler from a folder that was just removed).
  applyEvent(event: WatchEvent) {
    if (event.kind === "fileAdded") {
      const f = this.folders.find((f) => f.path === event.folder);
      if (f) {
        f.filesAdded += 1;
        f.lastEventAt = Date.now();
      }
    } else if (event.kind === "folderStatus") {
      const f = this.folders.find((f) => f.path === event.folder);
      if (f) f.status = event.status;
    }
    // `fileRemoved` doesn't change the folder row directly — the queue
    // handler deals with stale-pending cleanup.
  }

  /// Phase 3: burst processing state.
  startBatch(folderPath: string, total: number) {
    const f = this.folders.find((f) => f.path === folderPath);
    if (f) f.batchInFlight = { completed: 0, total, errors: 0 };
  }

  applyBurstProgress(folderPath: string, update: ProgressUpdate) {
    const f = this.folders.find((f) => f.path === folderPath);
    if (!f || !f.batchInFlight) return;
    f.batchInFlight.completed = update.completed;
    f.batchInFlight.total = update.total;
    if (update.item.error) f.batchInFlight.errors += 1;
  }

  finishBatch(folderPath: string) {
    const f = this.folders.find((f) => f.path === folderPath);
    if (f) f.batchInFlight = undefined;
  }
}

export const watchFolders = new WatchFoldersStore();
