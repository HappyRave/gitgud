<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { branches as loadBranches, type Branches } from "$lib/api";
  import { aheadBehind, branchTree, visibleRows } from "$lib/sidebar/tree";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";

  let {
    repoId,
    revision,
    busy = false,
    oplogOpen = false,
    merging = false,
    data = $bindable(null),
    oncheckout,
    onmerge,
    onrebase,
    onoplog,
  }: {
    /** "Rebase current onto" from a branch's context menu. */
    onrebase: (branch: string) => void;
    /** The loaded branches, bindable so the toolbar can show the current one's ahead / behind. */
    data?: Branches | null;
    repoId: number;
    /** Changes after every write or outside change, so the lists reload. */
    revision: number;
    busy?: boolean;
    oplogOpen?: boolean;
    /** A merge or rebase is in progress: no checkout, merge or rebase. */
    merging?: boolean;
    /** Double-click (or Enter) on a local branch. */
    oncheckout: (branch: string) => void;
    /** "Merge into current" from a branch's context menu. */
    onmerge: (branch: string) => void;
    onoplog: () => void;
  } = $props();

  let error = $state<string | null>(null);
  const collapsed = new SvelteSet<string>();
  const openRemotes = new SvelteSet<string>();

  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const current = $derived(data?.local.find((b) => b.current)?.name ?? null);

  function openMenu(event: MouseEvent, branch: string, local: boolean) {
    event.preventDefault();
    const items: MenuItem[] = [];
    if (local) {
      items.push({ label: `Check out ${branch}`, hint: "Dbl-click", disabled: busy || merging, run: () => oncheckout(branch) });
    }
    items.push({
      label: current ? `Merge ${branch} into ${current}…` : `Merge ${branch}…`,
      disabled: busy || merging || !current,
      run: () => onmerge(branch),
    });
    items.push({
      label: current ? `Rebase ${current} onto ${branch}…` : `Rebase onto ${branch}…`,
      disabled: busy || merging || !current,
      run: () => onrebase(branch),
    });
    // Shift+F10 / the menu key fire it with no pointer position: open under the row.
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const fromKeyboard = event.clientX === 0 && event.clientY === 0;
    menu = { x: fromKeyboard ? rect.left + 16 : event.clientX, y: fromKeyboard ? rect.bottom : event.clientY, items };
  }

  const rows = $derived(data ? visibleRows(branchTree(data.local), collapsed) : []);

  $effect(() => {
    const key = `${repoId}:${revision}`;
    loadBranches(repoId).then(
      (b) => {
        if (key === `${repoId}:${revision}`) {
          data = b;
          error = null;
        }
      },
      (err) => (error = String(err)),
    );
  });

  function toggle(set: SvelteSet<string>, key: string) {
    if (set.has(key)) {
      set.delete(key);
    } else {
      set.add(key);
    }
  }
</script>

<nav class="side" aria-label="Sidebar">
  {#if error}
    <p role="alert">{error}</p>
  {/if}

  <section aria-label="Branches">
    <h3>Branches <span class="ct">{data?.local.length ?? ""}</span></h3>
    <ul>
      {#each rows as { node, depth } (node.kind === "folder" ? node.path : node.branch.name)}
        <li style:--depth={depth}>
          {#if node.kind === "folder"}
            <button class="it" aria-expanded={!collapsed.has(node.path)} onclick={() => toggle(collapsed, node.path)}>
              <span class="ic">{collapsed.has(node.path) ? "▸" : "▾"}</span>{node.name}/
            </button>
          {:else}
            {@const branch = node.branch}
            <button
              class="it"
              class:cur={branch.current}
              aria-current={branch.current ? "true" : undefined}
              title={branch.current ? "Current branch" : `Double-click to check out ${branch.name} · right-click for more`}
              disabled={busy && !branch.current}
              ondblclick={() => !branch.current && !merging && oncheckout(branch.name)}
              onkeydown={(e) => e.key === "Enter" && !branch.current && !merging && oncheckout(branch.name)}
              oncontextmenu={(e) => (branch.current ? e.preventDefault() : openMenu(e, branch.name, true))}
            >
              <span class="ic">{branch.current ? "✓" : ""}</span>{node.name}<span class="r">{aheadBehind(branch)}</span>
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  </section>

  <section aria-label="Remotes">
    <h3>Remotes</h3>
    <ul>
      {#each data?.remotes ?? [] as remote (remote.name)}
        <li>
          <button class="it" aria-expanded={openRemotes.has(remote.name)} onclick={() => toggle(openRemotes, remote.name)}>
            <span class="ic">{openRemotes.has(remote.name) ? "▾" : "▸"}</span>{remote.name}
          </button>
        </li>
        {#if openRemotes.has(remote.name)}
          {#each remote.branches as name (name)}
            <li style:--depth={1}>
              <button class="it leaf" title="Right-click to merge {name}" oncontextmenu={(e) => openMenu(e, name, false)}>
                <span class="ic"></span>{name.slice(remote.name.length + 1)}
              </button>
            </li>
          {/each}
        {/if}
      {/each}
    </ul>
  </section>

  <section aria-label="History">
    <h3>History</h3>
    <ul>
      <li>
        <button class="it" class:sel={oplogOpen} aria-label="Op log" aria-pressed={oplogOpen} onclick={onoplog}>
          <span class="ic">↶</span>Op log<span class="r">Ctrl+3</span>
        </button>
      </li>
    </ul>
  </section>
</nav>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .side {
    overflow-y: auto;
    background: var(--panel);
    border-right: 1px solid var(--line);
    padding-bottom: 8px;
  }
  section {
    padding: 6px 0 2px;
  }
  h3 {
    display: flex;
    gap: 6px;
    margin: 0;
    padding: 3px 10px;
    font: 600 10.5px var(--mono);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  .ct {
    margin-left: auto;
    font-weight: 400;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .it {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 22px;
    padding: 0 10px 0 calc(10px + var(--depth, 0) * 14px);
    white-space: nowrap;
    overflow: hidden;
    cursor: default;
  }
  .it:hover {
    background: var(--hover);
  }
  .it:focus-visible {
    outline: 2px solid var(--l0);
    outline-offset: -2px;
  }
  .it:disabled {
    color: var(--muted);
  }
  .it.cur {
    font-weight: 600;
  }
  .it.sel {
    background: var(--sel);
  }
  .ic {
    width: 12px;
    text-align: center;
    color: var(--muted);
    flex: none;
  }
  .r {
    margin-left: auto;
    color: var(--muted);
    font: 10.5px var(--mono);
  }
  .leaf {
    color: var(--muted);
  }
  .leaf:hover {
    color: var(--fg);
  }
</style>
