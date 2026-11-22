<script lang="ts">
  import { onMount } from "svelte";
  import { graphFind } from "$lib/api";

  let {
    repoId,
    graphVersion,
    hits = $bindable(null),
    onreveal,
    onclose,
  }: {
    repoId: number;
    /** A rebuilt graph means new row numbers: search again. */
    graphVersion: number;
    /** Matching rows, for the graph to mark. */
    hits?: ReadonlySet<number> | null;
    onreveal: (row: number) => void;
    onclose: () => void;
  } = $props();

  let query = $state("");
  let matches = $state<number[]>([]);
  let at = $state(0);
  let error = $state<string | null>(null);
  let input = $state<HTMLInputElement | null>(null);

  /** Ctrl+F again while open: back into the field. */
  export function focus() {
    input?.focus();
    input?.select();
  }

  onMount(focus);

  $effect(() => {
    const q = query;
    void graphVersion;
    if (!q.trim()) {
      matches = [];
      hits = null;
      return;
    }
    const timer = setTimeout(() => {
      graphFind(repoId, q).then(
        (rows) => {
          if (q !== query) {
            return;
          }
          error = null;
          matches = rows;
          hits = new Set(rows);
          at = 0;
          if (rows.length) {
            onreveal(rows[0]);
          }
        },
        (err) => (error = String(err)),
      );
    }, 150);
    return () => clearTimeout(timer);
  });

  function step(delta: number) {
    if (matches.length === 0) {
      return;
    }
    at = (at + delta + matches.length) % matches.length;
    onreveal(matches[at]);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      step(event.shiftKey ? -1 : 1);
    } else if (event.key === "Escape") {
      event.preventDefault();
      hits = null;
      onclose();
    }
  }
</script>

<div class="find" role="search">
  <span aria-hidden="true">⌕</span>
  <input
    bind:this={input}
    type="text"
    aria-label="Find commits"
    placeholder="Subject, author, hash or branch"
    bind:value={query}
    onkeydown={onKeydown}
  />
  <span class="count" aria-live="polite">
    {#if error}
      <span class="bad" title={error}>error</span>
    {:else if query.trim()}
      {matches.length ? `${at + 1} of ${matches.length}` : "no match"}
    {/if}
  </span>
  <button class="bare nav" aria-label="Previous match" title="Shift+Enter" disabled={!matches.length} onclick={() => step(-1)}>↑</button>
  <button class="bare nav" aria-label="Next match" title="Enter" disabled={!matches.length} onclick={() => step(1)}>↓</button>
  <button
    class="bare nav"
    aria-label="Close find"
    title="Esc"
    onclick={() => {
      hits = null;
      onclose();
    }}>✕</button
  >
</div>

<style>
  .find {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
    font: 12px var(--body);
    color: var(--muted);
  }
  input {
    width: 220px;
    height: 22px;
    padding: 0 6px;
    font: 12px var(--body);
  }
  .count {
    min-width: 58px;
    font: 11px var(--mono);
    text-align: right;
  }
  .bad {
    color: var(--bad-ink);
  }
  .nav {
    all: unset;
    width: 20px;
    text-align: center;
    border-radius: 3px;
    cursor: default;
  }
  .nav:hover:not(:disabled) {
    background: var(--hover);
    color: var(--fg);
  }
  .nav:disabled {
    opacity: 0.4;
  }
</style>
