<script lang="ts">
  import { onMount } from "svelte";
  import type { LibraryFolder, LibraryRepo } from "$lib/api";
  import { folderPathLabel, pathKey, scoreRepoMatch } from "$lib/library/filter";

  let {
    repos,
    folders,
    openPaths,
    onopen,
    onaddrepo,
    onscanfolder,
    onclone,
    onclose,
  }: {
    repos: LibraryRepo[];
    folders: LibraryFolder[];
    openPaths: ReadonlySet<string>;
    onopen: (path: string) => void;
    onaddrepo: () => void;
    onscanfolder: () => void;
    onclone: () => void;
    onclose: () => void;
  } = $props();

  let query = $state("");
  let activeIdx = $state(0);
  let input = $state<HTMLInputElement | null>(null);

  const matches = $derived.by(() => {
    const scored: { repo: LibraryRepo; folder: string; isOpen: boolean; score: number }[] = [];
    for (const r of repos) {
      const folder = folderPathLabel(folders, r.folderId);
      const isOpen = openPaths.has(pathKey(r.path));
      const score = scoreRepoMatch(r, folder, query, isOpen);
      if (score !== null) {
        scored.push({ repo: r, folder, isOpen, score });
      }
    }
    scored.sort((a, b) => b.score - a.score || a.repo.name.localeCompare(b.repo.name));
    return scored.slice(0, 12);
  });

  $effect(() => {
    if (activeIdx >= matches.length) {
      activeIdx = Math.max(0, matches.length - 1);
    }
  });

  onMount(() => {
    requestAnimationFrame(() => input?.focus());
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      if (matches.length > 0) {
        activeIdx = Math.min(matches.length - 1, activeIdx + 1);
      }
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      activeIdx = Math.max(0, activeIdx - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      const target = matches[activeIdx];
      if (target) {
        onopen(target.repo.path);
      }
    }
  }
</script>

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="pal" role="dialog" aria-modal="true" aria-label="Go to repository" tabindex="-1" onkeydown={onKeydown}>
    <div class="search">
      <span aria-hidden="true">⌕</span>
      <input bind:this={input} type="text" bind:value={query} placeholder="Jump to repository… name, branch, folder" aria-label="Go to repository" />
      <span class="ab">↵ open</span>
    </div>

    {#if matches.length === 0}
      <div class="empty ab">No matching repositories</div>
    {:else}
      <ul class="list" role="listbox" aria-label="Matching repositories">
        {#each matches as m, i (m.repo.id)}
          <li>
            <button
              type="button"
              role="option"
              aria-selected={i === activeIdx}
              class="bare row"
              class:sel={i === activeIdx}
              onclick={() => onopen(m.repo.path)}
            >
              <span class="ic">{m.isOpen ? "⇥" : "▣"}</span>
              <span class="main">
                <span class="pf">{m.folder} /</span>
                <b>{m.repo.name}</b>
              </span>
              <span class="sp"></span>
              <span class="ab">
                {#if m.isOpen}open · {/if}
                {m.repo.cached?.branch ?? ""}
                {#if m.repo.cached?.dirtyCount} · {m.repo.cached.dirtyCount} changed{/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <div class="acts" aria-label="Library actions">
      <button type="button" class="bare act" onclick={onaddrepo}>+ Open repository folder…</button>
      <button type="button" class="bare act" onclick={onscanfolder}>Scan a folder…</button>
      <button type="button" class="bare act" onclick={onclone}>Clone a repository…</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: grid;
    place-items: start center;
    padding-top: 10vh;
    z-index: 30;
  }
  .pal {
    width: min(580px, 92vw);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 20px 64px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
  }
  .search input {
    flex: 1;
    border: 0;
    background: transparent;
    color: var(--fg);
    font: 13px var(--body);
    outline: none;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 4px;
    max-height: 50vh;
    overflow-y: auto;
    display: grid;
    gap: 1px;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: 6px;
    text-align: left;
  }
  .row.sel,
  .row:hover {
    background: var(--sel);
  }
  .ic {
    color: var(--muted);
    width: 16px;
    text-align: center;
  }
  .pf {
    color: var(--muted);
    font-size: 11.5px;
    margin-right: 4px;
  }
  .sp {
    flex: 1;
  }
  .acts {
    display: flex;
    gap: 6px;
    padding: 6px 10px;
    border-top: 1px solid var(--line);
    background: var(--bg);
  }
  .act {
    padding: 3px 8px;
    border-radius: 5px;
    color: var(--l0);
    font-size: 12px;
  }
  .act:hover {
    background: var(--hover);
  }
  .empty {
    padding: 16px;
    text-align: center;
  }
</style>
