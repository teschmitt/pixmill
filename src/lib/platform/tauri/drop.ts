import { getCurrentWebview } from "@tauri-apps/api/webview";

import type { DropHandlers } from "../types";

/**
 * Tauri delivers OS-level drop events through the webview rather than the DOM,
 * so we subscribe to `onDragDropEvent` and translate the payload into the
 * platform-agnostic shape.
 */
export function setupDropHandler(handlers: DropHandlers): () => void {
  const webview = getCurrentWebview();
  const unlisten = webview.onDragDropEvent((event) => {
    const p = event.payload;
    if (p.type === "enter" || p.type === "over") {
      handlers.onDragging(true);
    } else if (p.type === "leave") {
      handlers.onDragging(false);
    } else if (p.type === "drop") {
      handlers.onDragging(false);
      handlers.onDrop(p.paths);
    }
  });
  return () => {
    void unlisten.then((fn) => fn());
  };
}
