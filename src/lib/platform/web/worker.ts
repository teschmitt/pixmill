/**
 * Web Worker that owns the File registry and the wasm instance.
 *
 * Main thread holds only opaque synthetic ids (`web:<n>` for files,
 * `webfolder:<n>` for folder bundles). All `arrayBuffer()` reads and wasm
 * calls happen here, off the main thread, so the UI stays responsive even
 * on multi-megabyte images. Comlink wraps `postMessage` into async function
 * calls and serialises arguments via structured clone (which transfers
 * `File`/`Blob`/`Uint8Array` cheaply).
 */
import * as Comlink from "comlink";
import JSZip from "jszip";

import wasmInit, * as wasm from "$lib/wasm/pkg/pixmill_wasm";
import type { BatchItemResult, ImageFormat, Settings } from "$lib/types";

import type { ProgressUpdate, RawMetadata, SourceLoad } from "../types";

const ready = wasmInit();

const fileRegistry = new Map<string, File>();
const folderRegistry = new Map<string, string[]>();
let nextFileId = 0;
let nextFolderId = 0;

function registerFile(file: File): string {
  const id = `web:${nextFileId++}`;
  fileRegistry.set(id, file);
  return id;
}

function registerFolder(files: File[]): string {
  const ids = files.map(registerFile);
  const folderId = `webfolder:${nextFolderId++}`;
  folderRegistry.set(folderId, ids);
  return folderId;
}

function expandId(id: string): string[] {
  const folder = folderRegistry.get(id);
  return folder ? [...folder] : [id];
}

async function readBytes(id: string): Promise<{ file: File; bytes: Uint8Array }> {
  await ready;
  const file = fileRegistry.get(id);
  if (!file) throw new Error(`unknown file id: ${id}`);
  const bytes = new Uint8Array(await file.arrayBuffer());
  return { file, bytes };
}

interface ProcessedImage {
  bytes: Uint8Array;
  format: ImageFormat;
  width: number;
  height: number;
  sizeBytes: number;
}

export interface BatchResult {
  blob: Blob;
  filename: string;
  results: BatchItemResult[];
}

const api = {
  /** Register a single picked/dropped file; returns the synthetic id. */
  async registerFile(file: File): Promise<string> {
    return registerFile(file);
  },

  /** Register a batch of files (e.g. from a multi-select picker or recursive drop). */
  async registerFiles(files: File[]): Promise<string[]> {
    return files.map(registerFile);
  },

  /**
   * Register a webkitdirectory pick as a single folder id. `ingest()` expands
   * it back into the underlying file ids so the contract of one string from
   * `pickFolder()` matches the desktop side.
   */
  async registerFolder(files: File[]): Promise<string> {
    return registerFolder(files);
  },

  /**
   * Ingest one or more ids. Folder ids are expanded transparently; duplicate
   * file ids in the input are deduped (mirrors the desktop `dedupe_keep_order`).
   */
  async ingest(ids: string[]): Promise<RawMetadata[]> {
    await ready;
    const expanded: string[] = [];
    const seen = new Set<string>();
    for (const id of ids) {
      for (const child of expandId(id)) {
        if (!seen.has(child)) {
          seen.add(child);
          expanded.push(child);
        }
      }
    }
    const results: RawMetadata[] = [];
    for (const id of expanded) {
      results.push(await readMetadata(id));
    }
    return results;
  },

  async readMetadata(id: string): Promise<RawMetadata> {
    await ready;
    return readMetadata(id);
  },

  async makeThumbnail(id: string, longEdge: number): Promise<Uint8Array> {
    const { file, bytes } = await readBytes(id);
    return wasm.makeThumbnail(bytes, file.name, longEdge);
  },

  async loadSource(id: string): Promise<SourceLoad> {
    const { file, bytes } = await readBytes(id);
    return wasm.loadSource(bytes, file.name) as SourceLoad;
  },

  async processImage(id: string, settings: Settings): Promise<ProcessedImage> {
    const { file, bytes } = await readBytes(id);
    return wasm.processImage(bytes, file.name, settings) as ProcessedImage;
  },

  /**
   * Sequential batch: process each id in order, accumulate into a ZIP, and
   * return the assembled blob plus per-item results. Progress is reported
   * through `onProgress` so the existing main-thread UI wiring just works.
   * `onProgress` is a Comlink proxy — calling it triggers a postMessage,
   * we don't need to await it.
   */
  async runBatch(
    ids: string[],
    settings: Settings,
    onProgress: (update: ProgressUpdate) => void
  ): Promise<BatchResult> {
    await ready;
    const zip = new JSZip();
    const results: BatchItemResult[] = [];
    const used = new Set<string>();

    for (let i = 0; i < ids.length; i++) {
      const id = ids[i];
      let item: BatchItemResult;
      try {
        const file = fileRegistry.get(id);
        if (!file) throw new Error(`unknown file id: ${id}`);
        const inBytes = new Uint8Array(await file.arrayBuffer());
        const out = wasm.processImage(inBytes, file.name, settings) as ProcessedImage;
        const outName = uniqueName(used, stemOf(file.name), out.format);
        zip.file(outName, out.bytes);
        item = { source: id, destination: outName, error: null };
      } catch (e) {
        item = { source: id, destination: null, error: errorMessage(e) };
      }
      results.push(item);
      onProgress({ completed: i + 1, total: ids.length, item });
    }

    const blob = await zip.generateAsync({ type: "blob" });
    return { blob, filename: batchZipFilename(), results };
  },
};

async function readMetadata(id: string): Promise<RawMetadata> {
  const { file, bytes } = await readBytes(id);
  const meta = wasm.ingestMetadata(bytes, file.name) as RawMetadata;
  // Replace the filename-as-path that pixmill-core sets in the bytes API with
  // the synthetic id the main thread needs to call back into.
  meta.path = id;
  return meta;
}

function stemOf(filename: string): string {
  const idx = filename.lastIndexOf(".");
  return idx <= 0 ? filename : filename.slice(0, idx);
}

function extensionFor(format: ImageFormat): string {
  return format === "jpeg" ? "jpg" : format;
}

// Mirrors `pixmill_core::pipeline::plan_output_path`: on collision append
// `_1`, `_2`, ... up to a sanity cap.
function uniqueName(used: Set<string>, stem: string, format: ImageFormat): string {
  const ext = extensionFor(format);
  let candidate = `${stem}.${ext}`;
  let n = 1;
  while (used.has(candidate)) {
    candidate = `${stem}_${n}.${ext}`;
    n += 1;
    if (n > 9999) break;
  }
  used.add(candidate);
  return candidate;
}

function batchZipFilename(): string {
  const ts = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
  return `pixmill-${ts}.zip`;
}

function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}

export type WorkerApi = typeof api;

Comlink.expose(api);
