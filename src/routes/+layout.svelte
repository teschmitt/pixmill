<script lang="ts">
  import { onMount } from "svelte";

  import { platform } from "$lib/platform";
  import { settings } from "$lib/stores/settings.svelte";
  import { watchFolders } from "$lib/stores/watchFolders.svelte";

  let { children } = $props();
  let loaded = $state(false);

  onMount(async () => {
    try {
      const state = await platform.loadSettings();
      if (state) {
        settings.current = state.settings;
        settings.outputDir = state.outputDir;
        if (state.watchedFolders) watchFolders.load(state.watchedFolders);
      }
    } catch (e) {
      // First-run or read failure — keep defaults.
      console.warn("loadSettings failed:", e);
    } finally {
      loaded = true;
    }
  });

  // Persist settings whenever they change (after initial load). The
  // `watchedFolders` list is also written so the layout's auto-save doesn't
  // wipe the backend-managed list — the watch-folder commands themselves
  // also persist on every mutation (one redundant write each, harmless).
  $effect(() => {
    if (!loaded) return;
    const snapshot = {
      settings: $state.snapshot(settings.current),
      outputDir: settings.outputDir,
      watchedFolders: watchFolders.folders.map((f) => ({
        path: f.path,
        recursive: f.recursive,
        autoProcess: f.autoProcess,
      })),
    };
    void platform.saveSettings(snapshot).catch((e) => console.warn("saveSettings failed:", e));
  });
</script>

{@render children()}
