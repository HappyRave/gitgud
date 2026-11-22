<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { branches as loadBranches, type Branches } from "$lib/api";
  import { aheadBehind, branchTree, visibleRows } from "$lib/sidebar/tree";

  let {
    repoId,
    revision,
    busy = false,
    oplogOpen = false,
    oncheckout,
    onoplog,
  }: {
    repoId: number;
    /** Changes after every write or outside change, so the lists reload. */
    revision: number;
    busy?: boolean;
    oplogOpen?: boolean;
    /** Double-click (or Enter) on a local branch. */
    oncheckout: (branch: string) => void;
    onoplog: () => void;
  } = $props();

  let data = $state<Branches | null>(null);
  let error = $state<string | null>(null);
  const collapsed = new SvelteSet<string>();
  const openRemotes = new SvelteSet<string>();

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
              title={branch.current ? "Current branch" : `Double-click to check out ${branch.name}`}
              disabled={busy && !branch.current}
              ondblclick={() => !branch.current && oncheckout(branch.name)}
              onkeydown={(e) => e.key === "Enter" && !branch.current && oncheckout(branch.name)}
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
            <li style:--depth={1}><span class="it leaf"><span class="ic"></span>{name.slice(remote.name.length + 1)}</span></li>
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

<style>
  .side {
    overflow-y: auto;
    font-size: 0.85rem;
    border-right: 1px solid #eee;
    padding-right: 0.5rem;
  }
  h3 {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #777;
    margin: 0.75rem 0 0.2rem;
  }
  .ct {
    color: #aaa;
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
    width: 100%;
    height: 22px;
    padding-left: calc(var(--depth, 0) * 12px);
    white-space: nowrap;
    overflow: hidden;
    cursor: default;
  }
  .it:hover {
    background: #f4f4f4;
  }
  .it:focus-visible {
    outline: 2px solid #4c8bf5;
  }
  .it.cur {
    font-weight: 600;
  }
  .it.sel {
    background: #e3ecfd;
  }
  .ic {
    display: inline-block;
    width: 1.2em;
    color: #888;
    flex: none;
  }
  .r {
    margin-left: auto;
    color: #888;
    font-size: 0.75rem;
    padding-right: 0.3rem;
  }
  .leaf {
    color: #555;
  }
</style>
