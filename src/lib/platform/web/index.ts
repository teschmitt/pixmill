/**
 * Web `Platform` implementation. The wasm module runs in a Worker
 * (see `worker.ts`); this module is the main-thread glue: it spawns the
 * worker, holds the Comlink proxy, drives DOM-only pieces (file pickers,
 * settings persistence, ZIP download), and converts wasm output back into
 * the shapes the rest of the app expects.
 *
 * `path: string` is opaque: the worker mints synthetic ids and the rest of
 * the app shuttles them around unchanged.
 */
import * as Comlink from "comlink";

import type { BatchItemResult, Settings, WatchedFolder, WatchEvent } from "$lib/types";

import type {
  DropHandlers,
  PersistedState,
  Platform,
  PreviewResult,
  ProgressUpdate,
  RawMetadata,
  SourceLoad,
} from "../types";
import { setupDropHandler as setupDomDropHandler } from "./drop";
import type { WorkerApi } from "./worker";

const supportedExtensions = ["jpg", "jpeg", "jpe", "png", "webp", "avif", "heic", "heif"];
const PERSISTED_STATE_KEY = "pixmill:state";
// Synthetic constant that lets the UI display *something* in the output-folder
// slot. `runBatch` ignores it because the web build downloads a ZIP instead
// of writing into a directory.
const WEB_OUTPUT_DIR = "__downloads__";

let workerRef: { worker: Worker; api: Comlink.Remote<WorkerApi> } | null = null;

function getWorker(): Comlink.Remote<WorkerApi> {
  if (!workerRef) {
    const worker = new Worker(new URL("./worker.ts", import.meta.url), {
      type: "module",
    });
    workerRef = { worker, api: Comlink.wrap<WorkerApi>(worker) };
  }
  return workerRef.api;
}

async function ingest(paths: string[], recursive: boolean): Promise<RawMetadata[]> {
  // `recursive` is a no-op: the web build expands folders at pick/drop time,
  // so by the time ids reach here they're already leaves (or a single folder
  // id that the worker expands transparently).
  void recursive;
  return await getWorker().ingest(paths);
}

async function readMetadata(path: string): Promise<RawMetadata> {
  return await getWorker().readMetadata(path);
}

async function makeThumbnail(path: string, longEdge = 256): Promise<string> {
  const bytes = await getWorker().makeThumbnail(path, longEdge);
  return bytesToDataUrl(bytes, "image/jpeg");
}

async function previewOne(path: string, settings: Settings): Promise<PreviewResult> {
  const out = await getWorker().processImage(path, settings);
  return {
    dataUrl: bytesToDataUrl(out.bytes, mimeFor(out.format)),
    width: out.width,
    height: out.height,
    sizeBytes: out.sizeBytes,
    format: out.format,
  };
}

async function loadSource(path: string): Promise<SourceLoad> {
  return await getWorker().loadSource(path);
}

async function runBatch(
  paths: string[],
  outDir: string,
  settings: Settings,
  onProgress: (update: ProgressUpdate) => void
): Promise<BatchItemResult[]> {
  // `outDir` is the synthetic `WEB_OUTPUT_DIR` constant. We don't write to
  // disk on the web build; the assembled ZIP is downloaded via the browser.
  void outDir;
  const { blob, filename, results } = await getWorker().runBatch(
    paths,
    settings,
    Comlink.proxy(onProgress)
  );
  triggerDownload(blob, filename);
  return results;
}

async function pickFiles(): Promise<string[]> {
  const files = await openFileDialog({ multiple: true });
  if (files.length === 0) return [];
  return await getWorker().registerFiles(files);
}

async function pickFolder(): Promise<string | null> {
  const files = await openFileDialog({ multiple: true, directory: true });
  const supported = files.filter((f) => isSupportedFilename(f.name));
  if (supported.length === 0) return null;
  return await getWorker().registerFolder(supported);
}

async function pickOutputFolder(): Promise<string | null> {
  // No real folder picker — ZIP download is the output mechanism on web.
  return WEB_OUTPUT_DIR;
}

async function loadSettings(): Promise<PersistedState | null> {
  if (typeof localStorage === "undefined") return null;
  const raw = localStorage.getItem(PERSISTED_STATE_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as PersistedState;
  } catch {
    return null;
  }
}

async function saveSettings(state: PersistedState): Promise<void> {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(PERSISTED_STATE_KEY, JSON.stringify(state));
}

function setupDropHandler(handlers: DropHandlers): () => void {
  return setupDomDropHandler(getWorker(), handlers);
}

interface FileDialogOptions {
  multiple?: boolean;
  directory?: boolean;
}

// Render a hidden `<input type=file>` and fire it. Resolving on `change`
// fails to fire if the user cancels in some browsers, so we also resolve
// on focus restoration to keep the promise from leaking. This is a common
// pattern; the worst case is an empty array on cancel, which the callers
// already handle.
function openFileDialog(options: FileDialogOptions): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.style.display = "none";
    if (options.multiple) input.multiple = true;
    if (options.directory) {
      // Non-standard but universally supported among the browsers we care about.
      (input as HTMLInputElement & { webkitdirectory?: boolean }).webkitdirectory = true;
    } else {
      input.accept = supportedExtensions.map((e) => `.${e}`).join(",");
    }

    let settled = false;
    const finish = (files: File[]) => {
      if (settled) return;
      settled = true;
      window.removeEventListener("focus", onFocus);
      input.remove();
      resolve(files);
    };

    const onChange = () => finish(Array.from(input.files ?? []));
    // Browsers fire `focus` on the window once the picker closes, even on
    // cancel. Give `change` a beat to fire first; if it didn't, treat the
    // dialog as cancelled.
    const onFocus = () => {
      setTimeout(() => finish([]), 200);
    };

    input.addEventListener("change", onChange, { once: true });
    window.addEventListener("focus", onFocus, { once: true });

    document.body.appendChild(input);
    input.click();
  });
}

function triggerDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.style.display = "none";
  document.body.appendChild(a);
  a.click();
  a.remove();
  // Give the browser a tick to start the download before revoking.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

function isSupportedFilename(name: string): boolean {
  const idx = name.lastIndexOf(".");
  if (idx < 0) return false;
  return supportedExtensions.includes(name.slice(idx + 1).toLowerCase());
}

function mimeFor(format: string): string {
  switch (format) {
    case "jpeg":
      return "image/jpeg";
    case "png":
      return "image/png";
    case "webp":
      return "image/webp";
    case "avif":
      return "image/avif";
    case "heic":
      return "image/heic";
    default:
      return "application/octet-stream";
  }
}

// Chunked btoa: `String.fromCharCode.apply` blows the call stack past ~120 KB
// of arguments, so we slice the array into 32 KB windows. Synchronous and
// good enough for thumbnails (~tens of KB) and preview outputs (~MB).
function bytesToDataUrl(bytes: Uint8Array, mime: string): string {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return `data:${mime};base64,${btoa(binary)}`;
}

function notSupported(): never {
  throw new Error("Watch folders are not supported in the web build");
}

async function addWatchedFolder(_folder: WatchedFolder): Promise<void> {
  notSupported();
}
async function removeWatchedFolder(_path: string): Promise<void> {
  notSupported();
}
async function setWatchedFolderConfig(
  _path: string,
  _recursive: boolean,
  _autoProcess: boolean
): Promise<void> {
  notSupported();
}
async function validateOutputDir(_path: string): Promise<void> {
  notSupported();
}
async function subscribeWatchEvents(_handler: (event: WatchEvent) => void): Promise<void> {
  notSupported();
}
async function retryWatchedFolder(_path: string): Promise<void> {
  notSupported();
}

export const platform: Platform = {
  ingest,
  readMetadata,
  makeThumbnail,
  pickFiles,
  pickFolder,
  pickOutputFolder,
  loadSettings,
  saveSettings,
  previewOne,
  loadSource,
  runBatch,
  setupDropHandler,
  supportsWatchFolders: false,
  addWatchedFolder,
  removeWatchedFolder,
  setWatchedFolderConfig,
  validateOutputDir,
  subscribeWatchEvents,
  retryWatchedFolder,
};
