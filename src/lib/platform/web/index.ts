import type { BatchItemResult } from "$lib/types";

import type {
  PersistedState,
  Platform,
  PreviewResult,
  RawMetadata,
  SourceLoad,
} from "../types";

function notImplemented(name: string): never {
  throw new Error(`platform/web: ${name} is not implemented yet (planned for W4)`);
}

// Stub: lets the Tauri build compile a "platform/web" import without pulling
// in any web-only deps. Real implementation lands in W4. Each method throws
// when called, never at import time.
export const platform: Platform = {
  async ingest(): Promise<RawMetadata[]> {
    notImplemented("ingest");
  },
  async readMetadata(): Promise<RawMetadata> {
    notImplemented("readMetadata");
  },
  async makeThumbnail(): Promise<string> {
    notImplemented("makeThumbnail");
  },
  async pickFiles(): Promise<string[]> {
    notImplemented("pickFiles");
  },
  async pickFolder(): Promise<string | null> {
    notImplemented("pickFolder");
  },
  async pickOutputFolder(): Promise<string | null> {
    notImplemented("pickOutputFolder");
  },
  async loadSettings(): Promise<PersistedState | null> {
    notImplemented("loadSettings");
  },
  async saveSettings(): Promise<void> {
    notImplemented("saveSettings");
  },
  async previewOne(): Promise<PreviewResult> {
    notImplemented("previewOne");
  },
  async loadSource(): Promise<SourceLoad> {
    notImplemented("loadSource");
  },
  async runBatch(): Promise<BatchItemResult[]> {
    notImplemented("runBatch");
  },
  setupDropHandler(): () => void {
    notImplemented("setupDropHandler");
  },
};
