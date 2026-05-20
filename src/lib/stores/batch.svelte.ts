class BatchStore {
  running = $state(false);
  completed = $state(0);
  total = $state(0);
  errors = $state(0);
  lastMessage = $state<string | null>(null);

  reset() {
    this.running = false;
    this.completed = 0;
    this.total = 0;
    this.errors = 0;
    this.lastMessage = null;
  }

  start(total: number) {
    this.running = true;
    this.completed = 0;
    this.total = total;
    this.errors = 0;
    this.lastMessage = null;
  }

  finish(message?: string) {
    this.running = false;
    if (message !== undefined) this.lastMessage = message;
  }
}

export const batch = new BatchStore();
