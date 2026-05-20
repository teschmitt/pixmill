const UNITS = ["B", "KB", "MB", "GB"];

export function formatBytes(bytes: number | undefined): string {
  if (bytes == null) return "—";
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  const decimals = v >= 100 || i === 0 ? 0 : 1;
  return `${v.toFixed(decimals)} ${UNITS[i]}`;
}

export function formatDimensions(w: number | undefined, h: number | undefined): string {
  if (w == null || h == null) return "—";
  return `${w}×${h}`;
}
