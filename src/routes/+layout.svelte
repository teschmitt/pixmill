<script lang="ts">
  import { onMount } from "svelte";

  import { settings } from "$lib/stores/settings.svelte";
  import { loadSettings, saveSettings } from "$lib/tauri/commands";

  let { children } = $props();
  let loaded = $state(false);

  onMount(async () => {
    try {
      const state = await loadSettings();
      if (state) {
        settings.current = state.settings;
        settings.outputDir = state.outputDir;
      }
    } catch (e) {
      // First-run or read failure — keep defaults.
      console.warn("loadSettings failed:", e);
    } finally {
      loaded = true;
    }
  });

  // Persist settings whenever they change (after initial load).
  $effect(() => {
    if (!loaded) return;
    const snapshot = {
      settings: $state.snapshot(settings.current),
      outputDir: settings.outputDir,
    };
    void saveSettings(snapshot).catch((e) =>
      console.warn("saveSettings failed:", e),
    );
  });
</script>

{@render children()}
