<script lang="ts">
  import { onMount } from "svelte";
  import { SHORTCUT_GROUPS } from "$lib/keymap";

  let { onclose }: { onclose: () => void } = $props();

  let query = $state("");
  let searchInput = $state<HTMLInputElement | null>(null);

  const filteredGroups = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) {
      return SHORTCUT_GROUPS;
    }
    return SHORTCUT_GROUPS.map((group) => {
      if (group.category.toLowerCase().includes(q)) {
        return group;
      }
      const items = group.items.filter(
        (item) => item.label.toLowerCase().includes(q) || item.keys.toLowerCase().includes(q),
      );
      return items.length > 0 ? { ...group, items } : null;
    }).filter((g): g is (typeof SHORTCUT_GROUPS)[number] => g !== null);
  });

  onMount(() => {
    requestAnimationFrame(() => searchInput?.focus());
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      if (query) {
        query = "";
        searchInput?.focus();
      } else {
        onclose();
      }
    }
  }
</script>

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-label="Keyboard shortcuts"
    tabindex="-1"
    onkeydown={onKeydown}
  >
    <div class="hdr">
      <h2>Keyboard shortcuts</h2>
      <button type="button" class="bare x" aria-label="Close" onclick={onclose}>✕</button>
    </div>

    <div class="search">
      <span aria-hidden="true">⌕</span>
      <input
        bind:this={searchInput}
        type="text"
        bind:value={query}
        placeholder="Filter shortcuts or keys…"
        aria-label="Filter shortcuts"
      />
      {#if query}
        <button
          type="button"
          class="bare clr"
          aria-label="Clear filter"
          onclick={() => {
            query = "";
            searchInput?.focus();
          }}>✕</button
        >
      {:else}
        <span class="ab">Esc close</span>
      {/if}
    </div>

    {#if filteredGroups.length === 0}
      <p class="empty ab">No shortcuts match "{query.trim()}".</p>
    {:else}
      <div class="grid">
        {#each filteredGroups as group (group.category)}
          <section class="sgrp">
            <h3>{group.category}</h3>
            <dl>
              {#each group.items as item (item.keys + item.label)}
                <dt><kbd>{item.keys}</kbd></dt>
                <dd>{item.label}</dd>
              {/each}
            </dl>
          </section>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(15, 20, 28, 0.42);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 40;
  }
  .sheet {
    width: min(760px, 94vw);
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 16px 18px;
    box-shadow: 0 18px 46px rgba(0, 0, 0, 0.28);
  }
  .hdr {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
    flex: none;
  }
  h2 {
    margin: 0;
    font: 600 15px var(--display);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 10px;
    margin-bottom: 14px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--bg);
    flex: none;
  }
  .search:focus-within {
    border-color: var(--l0);
    background: var(--panel);
  }
  .search input {
    flex: 1;
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--fg);
    font: 12.5px var(--body);
    outline: none;
  }
  .search input:focus {
    background: transparent;
    border: 0;
  }
  .clr {
    color: var(--muted);
    padding: 2px 4px;
    border-radius: 3px;
  }
  .clr:hover {
    color: var(--fg);
    background: var(--hover);
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(290px, 1fr));
    gap: 16px 24px;
    align-content: start;
  }
  .sgrp h3 {
    margin: 0 0 7px;
    font: 600 11px var(--mono);
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
    margin: 0;
    align-items: center;
  }
  dt {
    margin: 0;
  }
  dd {
    margin: 0;
    font-size: 12px;
  }
  kbd {
    display: inline-block;
    padding: 1px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--bg);
    font: 11px var(--mono);
    white-space: nowrap;
  }
  .x {
    padding: 2px 6px;
    border-radius: 4px;
    color: var(--muted);
  }
  .x:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .empty {
    margin: 18px 0;
  }
</style>
