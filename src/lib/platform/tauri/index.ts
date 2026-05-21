import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type { BatchItemResult, Settings } from "$lib/types";

import type {
  PersistedState,
  Platform,
  PreviewResult,
  ProgressUpdate,
  RawMetadata,
  SourceLoad,
} from "../types";
import { setupDropHandler } from "./drop";

const imageExtensions = ["jpg", "jpeg", "jpe", "png", "webp", "avif", "heic", "heif"];

async function ingest(paths: string[], recursive: boolean): Promise<RawMetadata[]> {
  return await invoke<RawMetadata[]>("ingest_paths", { paths, recursive });
}

async function readMetadata(path: string): Promise<RawMetadata> {
  return await invoke<RawMetadata>("read_metadata", { path });
}

async function makeThumbnail(path: string, longEdge = 256): Promise<string> {
  return await invoke<string>("make_thumbnail", { path, longEdge });
}

async function pickFiles(): Promise<string[]> {
  const selection = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "Images", extensions: imageExtensions }],
  });
  if (selection == null) return [];
  return Array.isArray(selection) ? selection : [selection];
}

async function pickFolder(): Promise<string | null> {
  const selection = await open({
    multiple: false,
    directory: true,
  });
  if (selection == null) return null;
  return Array.isArray(selection) ? selection[0] : selection;
}

async function pickOutputFolder(): Promise<string | null> {
  return await pickFolder();
}

async function loadSettings(): Promise<PersistedState | null> {
  return await invoke<PersistedState | null>("load_settings");
}

async function saveSettings(state: PersistedState): Promise<void> {
  await invoke<void>("save_settings", { state });
}

async function previewOne(path: string, settings: Settings): Promise<PreviewResult> {
  return await invoke<PreviewResult>("preview_one", { path, settings });
}

async function loadSource(path: string): Promise<SourceLoad> {
  return await invoke<SourceLoad>("load_source", { path });
}

async function runBatch(
  paths: string[],
  outDir: string,
  settings: Settings,
  onProgress: (update: ProgressUpdate) => void
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
};
