/**
 * Document-level drag-and-drop handler for the web build.
 *
 * Mirrors `src/lib/platform/tauri/drop.ts` but listens to DOM events instead
 * of the Tauri webview's `onDragDropEvent`. Folder support uses
 * `DataTransferItem.webkitGetAsEntry()` which is the only cross-browser way
 * to recurse into a dropped directory without the File System Access API
 * (which is Chromium-only and asks the user to re-grant permission).
 *
 * Registers each leaf file with the worker up front (recursion happens at
 * drop time, not ingest time) and hands the resulting synthetic ids to
 * `onDrop`, matching what `pickFiles()` produces.
 */
import type { Remote } from "comlink";

import type { DropHandlers } from "../types";
import type { WorkerApi } from "./worker";

const supportedExtensions = ["jpg", "jpeg", "jpe", "png", "webp", "avif", "heic", "heif"];

export function setupDropHandler(worker: Remote<WorkerApi>, handlers: DropHandlers): () => void {
  // dragenter/dragleave fire on every child element, so we count nesting depth
  // and only toggle the dragging flag at the document boundary.
  let depth = 0;

  const onDragEnter = (e: DragEvent) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    depth += 1;
    if (depth === 1) handlers.onDragging(true);
  };

  const onDragOver = (e: DragEvent) => {
    if (!hasFiles(e)) return;
    // Required to keep the browser from blocking the drop.
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  };

  const onDragLeave = (e: DragEvent) => {
    if (!hasFiles(e)) return;
    depth = Math.max(0, depth - 1);
    if (depth === 0) handlers.onDragging(false);
  };

  const onDrop = async (e: DragEvent) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    depth = 0;
    handlers.onDragging(false);

    // Snapshot accessors synchronously: per the HTML drag-drop spec, each
    // DataTransferItem detaches after the first `await`, after which
    // `webkitGetAsEntry()` / `getAsFile()` return null. Safari enforces this
    // strictly — without the snapshot, a multi-select drag from Finder only
    // landed the file the drag originated on.
    const items = Array.from(e.dataTransfer?.items ?? []);
    const captured = items.map((item) => {
      const entry = item.webkitGetAsEntry?.() ?? null;
      // Fallback to getAsFile only when no entry was available (older browsers
      // or non-FS drag sources). Grabbed synchronously for the same reason.
      return { entry, file: entry ? null : item.getAsFile() };
    });

    const files: File[] = [];
    for (const { entry, file } of captured) {
      if (entry) {
        await collectFromEntry(entry, files);
      } else if (file && isSupported(file.name)) {
        files.push(file);
      }
    }
    if (files.length === 0) return;

    const ids = await worker.registerFiles(files);
    handlers.onDrop(ids);
  };

  document.addEventListener("dragenter", onDragEnter);
  document.addEventListener("dragover", onDragOver);
  document.addEventListener("dragleave", onDragLeave);
  document.addEventListener("drop", onDrop);

  return () => {
    document.removeEventListener("dragenter", onDragEnter);
    document.removeEventListener("dragover", onDragOver);
    document.removeEventListener("dragleave", onDragLeave);
    document.removeEventListener("drop", onDrop);
  };
}

function hasFiles(e: DragEvent): boolean {
  return Array.from(e.dataTransfer?.types ?? []).includes("Files");
}

function isSupported(name: string): boolean {
  const idx = name.lastIndexOf(".");
  if (idx < 0) return false;
  return supportedExtensions.includes(name.slice(idx + 1).toLowerCase());
}

// `FileSystemDirectoryEntry.createReader().readEntries` paginates: it returns
// the next batch on each call and an empty array when exhausted, so we loop
// until empty rather than assuming a single call enumerates everything.
async function collectFromEntry(entry: FileSystemEntry, out: File[]): Promise<void> {
  if (entry.isFile) {
    const file = await fileFromEntry(entry as FileSystemFileEntry);
    if (isSupported(file.name)) out.push(file);
    return;
  }
  if (entry.isDirectory) {
    const reader = (entry as FileSystemDirectoryEntry).createReader();
    while (true) {
      const batch = await readEntriesBatch(reader);
      if (batch.length === 0) break;
      for (const child of batch) {
        await collectFromEntry(child, out);
      }
    }
  }
}

function fileFromEntry(entry: FileSystemFileEntry): Promise<File> {
  return new Promise((resolve, reject) => {
    entry.file(resolve, reject);
  });
}

function readEntriesBatch(reader: FileSystemDirectoryReader): Promise<FileSystemEntry[]> {
  return new Promise((resolve, reject) => {
    reader.readEntries(resolve, reject);
  });
}
