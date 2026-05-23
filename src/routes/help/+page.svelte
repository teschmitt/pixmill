<script lang="ts">
  import { marked } from "marked";

  import gettingStarted from "$lib/docs/getting-started.md?raw";
  import operations from "$lib/docs/operations.md?raw";
  import preview from "$lib/docs/preview.md?raw";
  import troubleshooting from "$lib/docs/troubleshooting.md?raw";
  import watchFolders from "$lib/docs/watch-folders.md?raw";
  import { resolve } from "$app/paths";

  type Topic = { slug: string; title: string; body: string };
  const topics: Topic[] = [
    { slug: "getting-started", title: "Getting started", body: gettingStarted },
    { slug: "operations", title: "Resize, crop, rotate", body: operations },
    { slug: "watch-folders", title: "Watch folders", body: watchFolders },
    { slug: "preview", title: "Side-by-side preview", body: preview },
    { slug: "troubleshooting", title: "Troubleshooting", body: troubleshooting },
  ];

  let active = $state<string>(topics[0].slug);
  let current = $derived(topics.find((t) => t.slug === active) ?? topics[0]);
  let rendered = $derived(marked.parse(current.body, { async: false }) as string);
</script>

<main class="help">
  <header class="topbar">
    <a class="back" href={resolve("/")} data-sveltekit-preload-data="off">← Back to Pixmill</a>
    <h1>Help</h1>
  </header>

  <div class="layout">
    <nav class="sidebar" aria-label="Help topics">
      <ul>
        {#each topics as topic (topic.slug)}
          <li>
            <button
              type="button"
              class:active={topic.slug === active}
              onclick={() => (active = topic.slug)}
            >
              {topic.title}
            </button>
          </li>
        {/each}
      </ul>
    </nav>
    <article class="content markdown">
      <!-- Content is bundled markdown authored in-repo, not user input -->
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      {@html rendered}
    </article>
  </div>
</main>

<style>
  .help {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .topbar h1 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }
  .back {
    color: var(--accent);
    text-decoration: none;
    font-size: 13px;
  }
  .back:hover {
    text-decoration: underline;
  }
  .layout {
    display: grid;
    grid-template-columns: 220px 1fr;
    flex: 1;
    min-height: 0;
  }
  .sidebar {
    border-right: 1px solid var(--border);
    background: var(--panel);
    padding: 16px 0;
  }
  .sidebar ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .sidebar button {
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    color: var(--text);
    padding: 8px 20px;
    font: inherit;
    cursor: pointer;
    border-left: 3px solid transparent;
  }
  .sidebar button:hover {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .sidebar button.active {
    border-left-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .content {
    padding: 24px 32px;
    max-width: 760px;
    line-height: 1.6;
    overflow-y: auto;
  }
  .content :global(h1) {
    font-size: 24px;
    margin: 0 0 16px;
  }
  .content :global(h2) {
    font-size: 18px;
    margin: 28px 0 12px;
  }
  .content :global(h3) {
    font-size: 15px;
    margin: 20px 0 8px;
  }
  .content :global(p),
  .content :global(ul),
  .content :global(ol) {
    margin: 0 0 12px;
  }
  .content :global(ul),
  .content :global(ol) {
    padding-left: 24px;
  }
  .content :global(li) {
    margin-bottom: 4px;
  }
  .content :global(code) {
    background: color-mix(in srgb, var(--text) 8%, transparent);
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 0.9em;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .content :global(pre) {
    background: color-mix(in srgb, var(--text) 6%, transparent);
    padding: 12px 14px;
    border-radius: 6px;
    overflow-x: auto;
    font-size: 13px;
  }
  .content :global(pre code) {
    background: none;
    padding: 0;
  }
  .content :global(a) {
    color: var(--accent);
  }
  .content :global(strong) {
    font-weight: 600;
  }
</style>
