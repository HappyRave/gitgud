<script lang="ts">
  import { pathKey } from "$lib/library/filter";
  import LockIcon from "./LockIcon.svelte";

  export interface RepoTabInfo {
    path: string;
    name: string;
    dirtyCount: number;
    locksHeld: number;
    opening?: boolean;
  }

  let {
    tabs,
    activePath,
    onselect,
    onclose,
    onadd,
    onquickswitch,
    onpalette,
    onshortcuts,
    onsettings,
  }: {
    tabs: RepoTabInfo[];
    /** `null` when the always-first Library tab is active. */
    activePath: string | null;
    onselect: (path: string | null) => void;
    onclose: (path: string) => void;
    onadd: () => void;
    onquickswitch: () => void;
    onpalette?: () => void;
    onshortcuts?: () => void;
    onsettings?: () => void;
  } = $props();

  const VISIBLE_CAP = 8;
  let overflowOpen = $state(false);
  let overflowWrap = $state<HTMLDivElement | null>(null);

  function onWindowPointerDown(event: PointerEvent) {
    if (overflowOpen && overflowWrap && !overflowWrap.contains(event.target as Node)) {
      overflowOpen = false;
    }
  }

  function onWindowKeydown(event: KeyboardEvent) {
    if (overflowOpen && event.key === "Escape") {
      event.preventDefault();
      overflowOpen = false;
    }
  }

  const visibleTabs = $derived.by(() => {
    if (tabs.length <= VISIBLE_CAP) {
      return tabs;
    }
    const head = tabs.slice(0, VISIBLE_CAP);
    if (activePath && !head.some((t) => pathKey(t.path) === pathKey(activePath))) {
      const active = tabs.find((t) => pathKey(t.path) === pathKey(activePath));
      if (active) {
        return [...head.slice(0, VISIBLE_CAP - 1), active];
      }
    }
    return head;
  });

  const overflowTabs = $derived.by(() => {
    const shown = new Set(visibleTabs.map((t) => pathKey(t.path)));
    return tabs.filter((t) => !shown.has(pathKey(t.path)));
  });

  const isSelected = (path: string) => activePath !== null && pathKey(activePath) === pathKey(path);
</script>

<svelte:window onpointerdown={onWindowPointerDown} onkeydown={onWindowKeydown} />

<nav class="tabs" aria-label="Workspace tabs">
  <div class="list" role="tablist" aria-label="Repositories">
    <button
      type="button"
      role="tab"
      class="tab lib"
      class:on={activePath === null}
      aria-selected={activePath === null}
      onclick={() => {
        overflowOpen = false;
        onselect(null);
      }}
    >
      <span aria-hidden="true">⌂</span> Library
    </button>

    {#each visibleTabs as tab (pathKey(tab.path))}
      {@const on = isSelected(tab.path)}
      <div class="tabwrap" class:on>
        <button
          type="button"
          role="tab"
          class="tab"
          class:on
          aria-selected={on}
          title={tab.path}
          onclick={() => {
            overflowOpen = false;
            onselect(tab.path);
          }}
        >
          {#if tab.dirtyCount > 0}
            <span class="dot" aria-label="{tab.dirtyCount} uncommitted"></span>
          {/if}
          <span class="nm">{tab.name}</span>
          {#if tab.locksHeld > 0}
            <span class="lk" aria-label="{tab.locksHeld} locks held"><LockIcon size={10} /></span>
          {/if}
        </button>
        <button
          type="button"
          class="bare cls"
          aria-label="Close {tab.name}"
          title="Close tab (Ctrl+W)"
          onclick={() => {
            overflowOpen = false;
            onclose(tab.path);
          }}
        >
          ✕
        </button>
      </div>
    {/each}

    {#if overflowTabs.length > 0}
      <div class="ovwrap" bind:this={overflowWrap}>
        <button
          type="button"
          class="tab ov"
          aria-expanded={overflowOpen}
          aria-label="{overflowTabs.length} more open tabs"
          onclick={() => (overflowOpen = !overflowOpen)}
        >
          +{overflowTabs.length} ▾
        </button>
        {#if overflowOpen}
          <div class="menu" role="menu" aria-label="More open tabs">
            {#each overflowTabs as tab (pathKey(tab.path))}
              <button
                type="button"
                role="menuitem"
                class="bare mi"
                onclick={() => {
                  overflowOpen = false;
                  onselect(tab.path);
                }}
              >
                <span class="nm">{tab.name}</span>
                {#if tab.dirtyCount > 0}<span class="ab">{tab.dirtyCount}</span>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <button type="button" class="bare add" aria-label="New tab" title="Open repository in new tab" onclick={onadd}>+</button>
  </div>

  <span class="sp"></span>
  <button type="button" class="bare quick" title="Go to repository (Ctrl+P)" onclick={onquickswitch}>
    Ctrl+P go to repo
  </button>
  {#if onpalette}
    <button type="button" class="bare quick" title="Command palette (Ctrl+K)" onclick={onpalette}>Ctrl+K</button>
  {/if}
  {#if onshortcuts}
    <button type="button" class="bare quick" aria-label="Keyboard shortcuts" title="Keyboard shortcuts (?)" onclick={onshortcuts}
      >?</button
    >
  {/if}
  {#if onsettings}
    <button type="button" class="bare quick" aria-label="Settings" title="Settings (Ctrl+,)" onclick={onsettings}>⚙</button>
  {/if}
</nav>

<style>
  .tabs {
    display: flex;
    align-items: stretch;
    gap: 2px;
    padding: 4px 10px 0;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
    min-height: 32px;
  }
  .list {
    display: flex;
    align-items: stretch;
    gap: 3px;
    min-width: 0;
    overflow: visible;
  }
  .tabwrap {
    display: inline-flex;
    align-items: center;
    background: var(--bg);
    border: 1px solid transparent;
    border-bottom: 0;
    border-radius: 6px 6px 0 0;
    padding-right: 4px;
  }
  .tabwrap.on {
    background: var(--panel);
    border-color: var(--line);
    margin-bottom: -1px;
    padding-bottom: 1px;
  }
  .tabwrap:hover:not(.on) {
    background: var(--hover);
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 27px;
    padding: 0 8px 0 10px;
    border: 1px solid transparent;
    border-bottom: 0;
    border-radius: 6px 6px 0 0;
    background: transparent;
    color: var(--muted);
    font: 500 12px var(--body);
  }
  .tabwrap .tab {
    border: 0;
    padding-right: 4px;
  }
  .tab.lib.on {
    background: var(--panel);
    border-color: var(--line);
    margin-bottom: -1px;
    padding-bottom: 1px;
  }
  .tab.on {
    color: var(--fg);
  }
  .tab:hover:not(.on) {
    color: var(--fg);
  }
  .nm {
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--l0);
    flex: none;
  }
  .lk {
    font-size: 10.5px;
  }
  .cls {
    font-size: 10.5px;
    color: var(--muted);
    padding: 2px 4px;
    border-radius: 3px;
    opacity: 0.7;
  }
  .cls:hover {
    opacity: 1;
    background: var(--hover);
    color: var(--fg);
  }
  .add {
    padding: 0 8px;
    color: var(--muted);
    border-radius: 4px;
    align-self: center;
    height: 22px;
  }
  .add:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .ovwrap {
    position: relative;
    display: inline-flex;
  }
  .menu {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    min-width: 190px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 7px;
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.2);
    padding: 4px;
    z-index: 25;
    display: grid;
    gap: 1px;
  }
  .mi {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-radius: 4px;
    text-align: left;
  }
  .mi:hover {
    background: var(--hover);
  }
  .sp {
    flex: 1;
  }
  .quick {
    align-self: center;
    padding: 2px 8px;
    border-radius: 4px;
    color: var(--muted);
    font: 11.5px var(--mono);
  }
  .quick:hover {
    background: var(--hover);
    color: var(--fg);
  }
</style>
