import { defaultSettings, type Settings } from "$lib/types";

class SettingsStore {
  current = $state<Settings>(structuredClone(defaultSettings));
  outputDir = $state<string | null>(null);

  reset() {
    this.current = structuredClone(defaultSettings);
  }
}

export const settings = new SettingsStore();
