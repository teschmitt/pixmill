import type { BatchItemResult, ImageFormat, Settings } from "$lib/types";

export interface RawMetadata {
  path: string;
  filename: string;
  format: ImageFormat | null;
  width: number | null;
  height: number | null;
  sizeBytes: number | null;
  error: string | null;
}

export interface ProgressUpdate {
  completed: number;
  total: number;
  item: BatchItemResult;
}

export interface PersistedState {
  settings: Settings;
  outputDir: string | null;
}

export interface PreviewResult {
  dataUrl: string;
  width: number;
  height: number;
  sizeBytes: number;
  format: ImageFormat;
}

export interface SourceLoad {
  dataUrl: string;
  width: number;
  height: number;
}

export interface DropHandlers {
  onDragging: (dragging: boolean) => void;
  onDrop: (paths: string[]) => void;
}

/**
 * Platform-specific operations. Two implementations: `tauri` (desktop) and
 * `web` (browser via wasm). Components import this via `$lib/platform`, never
 * the underlying impls directly.
 *
 * `path` is treated as an opaque per-platform identifier: real filesystem path
 * on Tauri, synthetic id on the web build.
 */
export interface Platform {
  ingest(paths: string[], recursive: boolean): Promise<RawMetadata[]>;
  readMetadata(path: string): Promise<RawMetadata>;
  makeThumbnail(path: string, longEdge?: number): Promise<string>;

  pickFiles(): Promise<string[]>;
  pickFolder(): Promise<string | null>;
  pickOutputFolder(): Promise<string | null>;

  loadSettings(): Promise<PersistedState | null>;
  saveSettings(state: PersistedState): Promise<void>;

  previewOne(path: string, settings: Settings): Promise<PreviewResult>;
  loadSource(path: string): Promise<SourceLoad>;
  runBatch(
    paths: string[],
    outDir: string,
    settings: Settings,
    onProgress: (update: ProgressUpdate) => void
  ): Promise<BatchItemResult[]>;

  setupDropHandler(handlers: DropHandlers): () => void;
}
