<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { branches as loadBranches, stashes as loadStashes, type Branch, type Branches, type StashEntry } from "$lib/api";
  import { aheadBehind, branchTree, visibleRows } from "$lib/sidebar/tree";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";

  let {
    repoId,
    revision,
    busy = false,
    oplogOpen = false,
    merging = false,
    selectedStash = null,
    data = $bindable(null),
    oncheckout,
    onmerge,
    onrebase,
    onoplog,
    oncreatebranch,
    onrenamebranch,
    ondeletebranch,
    onsetupstream,
    oncheckoutremote,
    onopenpr,
    onstashselect,
    onstashapply,
    onstashpop,
    onstashdrop,
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
    selectedStash?: number | null;
    /** Double-click (or Enter) on a local branch. */
    oncheckout: (branch: string) => void;
    /** "Merge into current" from a branch's context menu. */
    onmerge: (branch: string) => void;
    onoplog: () => void;
    oncreatebranch?: (at?: string) => void;
    onrenamebranch?: (branch: string) => void;
    ondeletebranch?: (branch: string, remote?: string) => void;
    onsetupstream?: (branch: string, upstream: string | null) => void;
    oncheckoutremote?: (remoteBranch: string) => void;
    onopenpr?: (branch: string) => void;
    onstashselect?: (index: number) => void;
    onstashapply?: (index: number) => void;
    onstashpop?: (index: number) => void;
    onstashdrop?: (index: number) => void;
  } = $props();

  let error = $state<string | null>(null);
  let stashList = $state<StashEntry[]>([]);
  const collapsed = new SvelteSet<string>();
  const openRemotes = new SvelteSet<string>();

  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const current = $derived(data?.local.find((b) => b.current)?.name ?? null);

  function menuPosition(event: MouseEvent) {
    // Shift+F10 / the menu key fire it with no pointer position: open under the row.
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const fromKeyboard = event.clientX === 0 && event.clientY === 0;
    return { x: fromKeyboard ? rect.left + 16 : event.clientX, y: fromKeyboard ? rect.bottom : event.clientY };
  }

  function openLocalMenu(event: MouseEvent, branch: Branch) {
    event.preventDefault();
    const items: MenuItem[] = [
      { label: `Check out ${branch.name}`, hint: "Dbl-click", disabled: busy || merging || branch.current, run: () => oncheckout(branch.name) },
      {
        label: current ? `Merge ${branch.name} into ${current}…` : `Merge ${branch.name}…`,
        disabled: busy || merging || !current || branch.current,
        run: () => onmerge(branch.name),
      },
      {
        label: current ? `Rebase ${current} onto ${branch.name}…` : `Rebase onto ${branch.name}…`,
        disabled: busy || merging || !current || branch.current,
        run: () => onrebase(branch.name),
      },
    ];
    if (oncreatebranch) {
      items.push({ label: `New branch from ${branch.name}…`, disabled: busy || merging, run: () => oncreatebranch(branch.name) });
    }
    if (onrenamebranch) {
      items.push({ label: "Rename…", hint: "F2", disabled: busy || merging, run: () => onrenamebranch(branch.name) });
    }
    if (onsetupstream) {
      if (branch.upstream) {
        items.push({ label: `Unset upstream (${branch.upstream})`, disabled: busy, run: () => onsetupstream(branch.name, null) });
      } else {
        const candidate = data?.remotes.flatMap((r) => r.branches).find((rb) => rb.endsWith(`/${branch.name}`));
        if (candidate) {
          items.push({ label: `Set upstream to ${candidate}`, disabled: busy, run: () => onsetupstream(branch.name, candidate) });
        }
      }
    }
    if (onopenpr && (branch.upstream || (data?.remotes.length ?? 0) > 0)) {
      items.push({ label: "Open Pull Request ↗", run: () => onopenpr(branch.name) });
    }
    if (ondeletebranch) {
      items.push({ label: `Delete ${branch.name}`, hint: "Del", disabled: busy || merging || branch.current, run: () => ondeletebranch(branch.name) });
    }
    menu = { ...menuPosition(event), items };
  }

  function openRemoteMenu(event: MouseEvent, branch: string, remoteName: string) {
    event.preventDefault();
    const items: MenuItem[] = [];
    if (oncheckoutremote) {
      items.push({ label: `Check out ${branch}…`, disabled: busy || merging, run: () => oncheckoutremote(branch) });
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
    if (ondeletebranch) {
      const short = branch.slice(remoteName.length + 1);
      items.push({ label: `Delete ${branch}…`, disabled: busy, run: () => ondeletebranch(short, remoteName) });
    }
    menu = { ...menuPosition(event), items };
  }

  function openStashMenu(event: MouseEvent, entry: StashEntry) {
    event.preventDefault();
    const items: MenuItem[] = [];
    if (onstashpop) {
      items.push({ label: "Pop stash", disabled: busy || merging, run: () => onstashpop(entry.index) });
    }
    if (onstashapply) {
      items.push({ label: "Apply stash", disabled: busy || merging, run: () => onstashapply(entry.index) });
    }
    if (onstashdrop) {
      items.push({ label: "Drop stash", disabled: busy, run: () => onstashdrop(entry.index) });
    }
    if (items.length > 0) {
      menu = { ...menuPosition(event), items };
    }
  }

  function onBranchKey(event: KeyboardEvent, branch: Branch) {
    if (event.key === "Enter" && !branch.current && !merging) {
      oncheckout(branch.name);
    } else if (event.key === "F2" && !busy && !merging && onrenamebranch) {
      event.preventDefault();
      onrenamebranch(branch.name);
    } else if (event.key === "Delete" && !busy && !merging && !branch.current && ondeletebranch) {
      event.preventDefault();
      ondeletebranch(branch.name);
    }
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
    loadStashes(repoId).then(
      (s) => {
        if (key === `${repoId}:${revision}`) {
          stashList = s;
        }
      },
      () => {},
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
    <h3>
      Branches <span class="ct">{data?.local.length ?? ""}</span>
      {#if oncreatebranch}
        <button class="bare add" title="New branch (Ctrl+B)" aria-label="New branch" disabled={busy || merging} onclick={() => oncreatebranch()}
          >+</button
        >
      {/if}
    </h3>
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
              title={branch.current ? "Current branch · right-click for more" : `Double-click to check out ${branch.name} · right-click for more`}
              disabled={busy && !branch.current}
              ondblclick={() => !branch.current && !merging && oncheckout(branch.name)}
              onkeydown={(e) => onBranchKey(e, branch)}
              oncontextmenu={(e) => openLocalMenu(e, branch)}
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
              <button class="it leaf" title="Right-click for actions" oncontextmenu={(e) => openRemoteMenu(e, name, remote.name)}>
                <span class="ic"></span>{name.slice(remote.name.length + 1)}
              </button>
            </li>
          {/each}
        {/if}
      {/each}
    </ul>
  </section>

  {#if stashList.length > 0}
    <section aria-label="Stashes">
      <h3>Stashes <span class="ct">{stashList.length}</span></h3>
      <ul>
        {#each stashList as s (s.id)}
          <li>
            <button
              class="it"
              class:sel={selectedStash === s.index}
              title="stash@{`{${s.index}}`}: {s.message}"
              onclick={() => onstashselect?.(s.index)}
              oncontextmenu={(e) => openStashMenu(e, s)}
            >
              <span class="ic">⊡</span><span class="nm">{s.message}</span>
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

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
    align-items: center;
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
  .add {
    all: unset;
    cursor: default;
    padding: 0 4px;
    border-radius: 3px;
    color: var(--muted);
    font-size: 13px;
    line-height: 1;
  }
  .add:hover:not(:disabled) {
    background: var(--hover);
    color: var(--fg);
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
  .nm {
    overflow: hidden;
    text-overflow: ellipsis;
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
