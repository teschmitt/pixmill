import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

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

export async function ingestPaths(
  paths: string[],
  recursive: boolean,
): Promise<RawMetadata[]> {
  return await invoke<RawMetadata[]>("ingest_paths", { paths, recursive });
}

export async function readMetadata(path: string): Promise<RawMetadata> {
  return await invoke<RawMetadata>("read_metadata", { path });
}

export async function makeThumbnail(
  path: string,
  longEdge = 256,
): Promise<string> {
  return await invoke<string>("make_thumbnail", { path, longEdge });
}

const imageExtensions = [
  "jpg",
  "jpeg",
  "jpe",
  "png",
  "webp",
  "avif",
  "heic",
  "heif",
];

export async function pickFiles(): Promise<string[]> {
  const selection = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "Images", extensions: imageExtensions }],
  });
  if (selection == null) return [];
  return Array.isArray(selection) ? selection : [selection];
}

export async function pickFolder(): Promise<string | null> {
  const selection = await open({
    multiple: false,
    directory: true,
  });
  if (selection == null) return null;
  return Array.isArray(selection) ? selection[0] : selection;
}

export async function pickOutputFolder(): Promise<string | null> {
  return await pickFolder();
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

export async function loadSettings(): Promise<PersistedState | null> {
  return await invoke<PersistedState | null>("load_settings");
}

export async function saveSettings(state: PersistedState): Promise<void> {
  await invoke<void>("save_settings", { state });
}

export async function runBatch(
  paths: string[],
  outDir: string,
  settings: Settings,
  onProgress: (update: ProgressUpdate) => void,
): Promise<BatchItemResult[]> {
  const channel = new Channel<ProgressUpdate>();
  channel.onmessage = onProgress;
  return await invoke<BatchItemResult[]>("run_batch", {
    paths,
    outDir,
    settings,
    onProgress: channel,
  });
}

