import type { QueueItem } from "$lib/types";

class QueueStore {
  items = $state<QueueItem[]>([]);

  add(newItems: QueueItem[]) {
    const existingPaths = new Set(this.items.map((i) => i.path));
    for (const item of newItems) {
      if (!existingPaths.has(item.path)) {
        this.items.push(item);
        existingPaths.add(item.path);
      }
    }
  }

  remove(id: string) {
    const idx = this.items.findIndex((i) => i.id === id);
    if (idx >= 0) this.items.splice(idx, 1);
  }

  clear() {
    this.items.length = 0;
  }

  update(id: string, patch: Partial<QueueItem>) {
    const item = this.items.find((i) => i.id === id);
    if (item) Object.assign(item, patch);
  }
}

export const queue = new QueueStore();
