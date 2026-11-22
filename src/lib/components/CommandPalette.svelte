<script lang="ts">
  import { onMount } from "svelte";
  import {
    prefs,
    setDensity,
    setTheme,
    toggleIgnoreWhitespace,
    toggleSplit,
  } from "$lib/prefs.svelte";

  export interface PaletteBranch {
    name: string;
    current: boolean;
  }

  export interface PaletteRepoActions {
    repoName: string;
    branches: PaletteBranch[];
    canPull: boolean;
    canPush: boolean;
    canStash: boolean;
    canUndo: boolean;
    canRedo: boolean;
    onfetch: () => void;
    onpull: () => void;
    onpush: () => void;
    onstash: () => void;
    onundo: () => void;
    onredo: () => void;
    onnewbranch: () => void;
    oncheckout: (branch: string) => void;
    onmerge: (branch: string) => void;
    onrebase: (branch: string) => void;
    ondeletebranch: (branch: string) => void;
    onfind: () => void;
    onoplog: () => void;
  }

  type BranchOp = "checkout" | "merge" | "rebase" | "delete";

  interface CommandItem {
    id: string;
    label: string;
    group: string;
    hint?: string;
    disabled?: boolean;
    branchOp?: BranchOp;
    run?: () => void;
  }

  let {
    repo = null,
    onopenrepo,
    onscanfolder,
    onclone,
    onquickswitch,
    onshortcuts,
    onsettings,
    onclose,
  }: {
    repo?: PaletteRepoActions | null;
    onopenrepo: () => void;
    onscanfolder: () => void;
    onclone: () => void;
    onquickswitch: () => void;
    onshortcuts: () => void;
    onsettings: () => void;
    onclose: () => void;
  } = $props();

  let query = $state("");
  let branchOp = $state<{ op: BranchOp; title: string } | null>(null);
  let activeIdx = $state(0);
  let input = $state<HTMLInputElement | null>(null);

  const commands = $derived.by((): CommandItem[] => {
    const items: CommandItem[] = [];
    if (repo) {
      items.push(
        { id: "fetch", label: "Fetch all remotes", group: repo.repoName, hint: "Ctrl+Shift+F", run: repo.onfetch },
        { id: "pull", label: "Pull current branch", group: repo.repoName, hint: "Ctrl+Shift+L", disabled: !repo.canPull, run: repo.onpull },
        { id: "push", label: "Push current branch", group: repo.repoName, hint: "Ctrl+Shift+P", disabled: !repo.canPush, run: repo.onpush },
        { id: "stash", label: "Stash uncommitted changes…", group: repo.repoName, disabled: !repo.canStash, run: repo.onstash },
        { id: "checkout", label: "Checkout branch…", group: repo.repoName, branchOp: "checkout" },
        { id: "merge", label: "Merge branch into HEAD…", group: repo.repoName, branchOp: "merge" },
        { id: "rebase", label: "Rebase current branch onto…", group: repo.repoName, branchOp: "rebase" },
        { id: "new-branch", label: "Create new branch…", group: repo.repoName, hint: "Ctrl+B", run: repo.onnewbranch },
        { id: "delete-branch", label: "Delete branch…", group: repo.repoName, branchOp: "delete" },
        { id: "find", label: "Find in commit graph", group: repo.repoName, hint: "Ctrl+F", run: repo.onfind },
        { id: "oplog", label: "Toggle operation history (Op log)", group: repo.repoName, hint: "Ctrl+3", run: repo.onoplog },
        { id: "undo", label: "Undo last repository action", group: repo.repoName, hint: "Ctrl+Z", disabled: !repo.canUndo, run: repo.onundo },
        { id: "redo", label: "Redo last undone action", group: repo.repoName, hint: "Ctrl+Y", disabled: !repo.canRedo, run: repo.onredo },
      );
    }
    items.push(
      {
        id: "toggle-split",
        label: `Diff layout: switch to ${prefs.split ? "unified" : "side-by-side split"}`,
        group: "View",
        hint: "Ctrl+D",
        run: toggleSplit,
      },
      {
        id: "toggle-ws",
        label: prefs.ignoreWhitespace ? "Diff: show whitespace changes" : "Diff: ignore whitespace changes",
        group: "View",
        run: toggleIgnoreWhitespace,
      },
      {
        id: "toggle-density",
        label: `Graph density: switch to ${prefs.density === "compact" ? "standard" : "compact"}`,
        group: "View",
        run: () => setDensity(prefs.density === "compact" ? "standard" : "compact"),
      },
      {
        id: "theme-cycle",
        label: `Theme: switch (current: ${prefs.theme})`,
        group: "View",
        run: () => {
          const order = ["system", "light", "dark"] as const;
          setTheme(order[(order.indexOf(prefs.theme) + 1) % order.length]);
        },
      },
      { id: "quick-switch", label: "Go to repository…", group: "Library", hint: "Ctrl+P", run: onquickswitch },
      { id: "open-repo", label: "Open local repository…", group: "Library", run: onopenrepo },
      { id: "scan-folder", label: "Scan folder for repositories…", group: "Library", run: onscanfolder },
      { id: "clone-repo", label: "Clone repository from URL…", group: "Library", run: onclone },
      { id: "settings", label: "Open settings", group: "Application", hint: "Ctrl+,", run: onsettings },
      { id: "shortcuts", label: "Keyboard shortcuts", group: "Application", hint: "?", run: onshortcuts },
    );
    return items;
  });

  function fuzzyMatch(text: string, needle: string): number | null {
    const q = needle.trim().toLowerCase();
    if (!q) {
      return 0;
    }
    const hay = text.toLowerCase();
    const direct = hay.indexOf(q);
    if (direct >= 0) {
      return direct === 0 ? 100 : 80 - direct * 0.1;
    }
    let qi = 0;
    let score = 0;
    for (let i = 0; i < hay.length && qi < q.length; i++) {
      if (hay[i] === q[qi]) {
        score += i === 0 || hay[i - 1] === " " ? 5 : 2;
        qi++;
      }
    }
    return qi === q.length ? score : null;
  }

  const filteredCommands = $derived.by(() => {
    const scored: { item: CommandItem; score: number }[] = [];
    for (const item of commands) {
      const score = fuzzyMatch(`${item.group} ${item.label}`, query);
      if (score !== null) {
        scored.push({ item, score: score - (item.disabled ? 50 : 0) });
      }
    }
    scored.sort((a, b) => b.score - a.score);
    return scored.map((s) => s.item);
  });

  const filteredBranches = $derived.by(() => {
    if (!repo || !branchOp) {
      return [];
    }
    const candidates =
      branchOp.op === "delete" || branchOp.op === "merge"
        ? repo.branches.filter((b) => !b.current)
        : repo.branches;
    return candidates.filter((b) => fuzzyMatch(b.name, query) !== null);
  });

  const itemCount = $derived(branchOp ? filteredBranches.length : filteredCommands.length);

  $effect(() => {
    if (activeIdx >= itemCount) {
      activeIdx = Math.max(0, itemCount - 1);
    }
  });

  onMount(() => {
    requestAnimationFrame(() => input?.focus());
  });

  function activateCommand(item: CommandItem) {
    if (item.disabled) {
      return;
    }
    if (item.branchOp) {
      const titles: Record<BranchOp, string> = {
        checkout: "Checkout branch",
        merge: "Merge branch into HEAD",
        rebase: "Rebase current branch onto",
        delete: "Delete branch",
      };
      branchOp = { op: item.branchOp, title: titles[item.branchOp] };
      query = "";
      activeIdx = 0;
      input?.focus();
      return;
    }
    onclose();
    item.run?.();
  }

  function activateBranch(branch: PaletteBranch) {
    if (!repo || !branchOp) {
      return;
    }
    const op = branchOp.op;
    onclose();
    if (op === "checkout") {
      repo.oncheckout(branch.name);
    } else if (op === "merge") {
      repo.onmerge(branch.name);
    } else if (op === "rebase") {
      repo.onrebase(branch.name);
    } else if (op === "delete") {
      repo.ondeletebranch(branch.name);
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    } else if (event.key === "Backspace" && branchOp && query === "") {
      event.preventDefault();
      branchOp = null;
      activeIdx = 0;
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      if (itemCount > 0) {
        activeIdx = Math.min(itemCount - 1, activeIdx + 1);
      }
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      activeIdx = Math.max(0, activeIdx - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (branchOp) {
        const b = filteredBranches[activeIdx];
        if (b) {
          activateBranch(b);
        }
      } else {
        const c = filteredCommands[activeIdx];
        if (c) {
          activateCommand(c);
        }
      }
    }
  }
</script>

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="pal" role="dialog" aria-modal="true" aria-label="Command palette" tabindex="-1" onkeydown={onKeydown}>
    <div class="search">
      {#if branchOp}
        <span class="step">{branchOp.title} ›</span>
      {:else}
        <span aria-hidden="true">›</span>
      {/if}
      <input
        bind:this={input}
        type="text"
        bind:value={query}
        placeholder={branchOp ? "Filter branch name… (Backspace to go back)" : "Type a command or action…"}
        aria-label={branchOp ? `Filter ${branchOp.title}` : "Filter commands"}
      />
      <span class="ab">↵ run · Esc</span>
    </div>

    {#if branchOp}
      {#if filteredBranches.length === 0}
        <div class="empty ab">No matching branches</div>
      {:else}
        <ul class="list" role="listbox" aria-label="Branches">
          {#each filteredBranches as b, i (b.name)}
            <li>
              <button
                type="button"
                class="bare row"
                class:active={i === activeIdx}
                role="option"
                aria-selected={i === activeIdx}
                onclick={() => activateBranch(b)}
              >
                <span class="nm">{b.name}</span>
                {#if b.current}<span class="badge">current</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {:else if filteredCommands.length === 0}
      <div class="empty ab">No matching commands</div>
    {:else}
      <ul class="list" role="listbox" aria-label="Commands">
        {#each filteredCommands as c, i (c.id)}
          <li>
            <button
              type="button"
              class="bare row"
              class:active={i === activeIdx}
              disabled={c.disabled}
              role="option"
              aria-selected={i === activeIdx}
              onclick={() => activateCommand(c)}
            >
              <span class="grp">{c.group}</span>
              <span class="nm">{c.label}</span>
              {#if c.hint}<span class="k">{c.hint}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
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
    align-items: flex-start;
    padding-top: 11vh;
    z-index: 40;
  }
  .pal {
    width: min(580px, 92vw);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 18px 46px rgba(0, 0, 0, 0.28);
    overflow: hidden;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
  }
  .step {
    font: 600 11.5px var(--body);
    color: var(--l0);
    background: var(--sel);
    padding: 2px 7px;
    border-radius: 4px;
    white-space: nowrap;
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
    max-height: 360px;
    overflow: auto;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 6px;
    text-align: left;
  }
  .row:disabled {
    opacity: 0.45;
  }
  .row:hover:not(:disabled),
  .row.active {
    background: var(--sel);
  }
  .grp {
    min-width: 72px;
    font: 500 10.5px var(--mono);
    color: var(--muted);
  }
  .nm {
    flex: 1;
    font: 500 12.5px var(--body);
  }
  .badge {
    font: 10px var(--mono);
    color: var(--l0);
  }
  .empty {
    padding: 14px 12px;
  }
</style>
