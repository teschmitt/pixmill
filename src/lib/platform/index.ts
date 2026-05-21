import { platform as tauriPlatform } from "./tauri";
import { platform as webPlatform } from "./web";
import type { Platform } from "./types";

// Build-time platform selection. `VITE_PLATFORM=web pnpm build` produces the web
// bundle; everything else (including `pnpm tauri dev|build`) gets the Tauri impl.
// The literal is inlined by Vite, so the unused branch is dead-code-eliminated.
const target = import.meta.env.VITE_PLATFORM ?? "tauri";

export const platform: Platform = target === "web" ? webPlatform : tauriPlatform;

export type * from "./types";
