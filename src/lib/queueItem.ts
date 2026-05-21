import type { QueueItem } from "$lib/types";

export interface RawIngestItem {
  path: string;
  filename: string;
  format: string | null;
  width: number | null;
  height: number | null;
  sizeBytes: number | null;
  error: string | null;
}

/// Convert the `RawMetadata` / `ImageMetadata` shape that ingest commands and
/// watch events emit into the in-app `QueueItem` shape. Status is `error` iff
/// the ingest itself failed (corrupt/unreadable image header).
export function toQueueItem(raw: RawIngestItem): QueueItem {
  return {
    id: raw.path,
    path: raw.path,
    filename: raw.filename,
    format: (raw.format as QueueItem["format"]) ?? null,
    width: raw.width ?? undefined,
    height: raw.height ?? undefined,
    sizeBytes: raw.sizeBytes ?? undefined,
    status: raw.error ? "error" : "pending",
    error: raw.error ?? undefined,
  };
}
