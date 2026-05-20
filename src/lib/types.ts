export type ImageFormat = "jpeg" | "png" | "webp" | "avif" | "heic";

export type QueueItemStatus =
  | "pending"
  | "thumbnailing"
  | "ready"
  | "processing"
  | "done"
  | "error";

export interface QueueItem {
  id: string;
  path: string;
  filename: string;
  format: ImageFormat | null;
  width?: number;
  height?: number;
  sizeBytes?: number;
  thumbnailDataUrl?: string;
  status: QueueItemStatus;
  error?: string;
  destination?: string;
}

export type ResizeMode =
  | { kind: "none" }
  | { kind: "maxLongEdge"; pixels: number }
  | { kind: "percentage"; percent: number };

export type CompressionMode =
  | { kind: "manual" }
  | { kind: "targetFileSize"; kilobytes: number };

export type CropMode =
  | { kind: "none" }
  | { kind: "aspectRatio"; width: number; height: number }
  | { kind: "pixels"; width: number; height: number };

export type RotateMode = "none" | "cw90" | "cw180" | "cw270" | "flipH" | "flipV";

export type OutputFormatChoice = "keep" | "jpeg" | "png" | "webp";

export interface Settings {
  resize: ResizeMode;
  crop: CropMode;
  rotate: RotateMode;
  outputFormat: OutputFormatChoice;
  compression: CompressionMode;
  jpegQuality: number | null;
  webpQuality: number | null;
  preserveExif: boolean;
}

export const defaultSettings: Settings = {
  resize: { kind: "maxLongEdge", pixels: 1920 },
  crop: { kind: "none" },
  rotate: "none",
  outputFormat: "keep",
  compression: { kind: "manual" },
  jpegQuality: 85,
  webpQuality: 85,
  preserveExif: true,
};

export interface BatchItemResult {
  source: string;
  destination: string | null;
  error: string | null;
}
