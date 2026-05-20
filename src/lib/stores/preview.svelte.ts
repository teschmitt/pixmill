import type { QueueItem } from "$lib/types";

class PreviewStore {
  item = $state<QueueItem | null>(null);

  open(item: QueueItem) {
    this.item = item;
  }

  close() {
    this.item = null;
  }
}

export const preview = new PreviewStore();
