<script lang="ts">
  import type { Branches, ComparePoint, StashEntry, TagInfo } from "$lib/api";
  import { pointLabel, rev, WORKING_COPY } from "$lib/compare/tree";

  let {
    side,
    value,
    branches,
    tags,
    stashes,
    onpick,
  }: {
    /** "A" or "B", for the tag and the labels. */
    side: "A" | "B";
    value: ComparePoint;
    branches: Branches | null;
    tags: TagInfo[];
    stashes: StashEntry[];
    onpick: (p: ComparePoint) => void;
  } = $props();

  let open = $state(false);
  let query = $state("");
  let input = $state<HTMLInputElement | null>(null);

  type Item = { group: string; label: string; point: ComparePoint; note?: string };

  const items = $derived.by<Item[]>(() => {
    const all: Item[] = [{ group: "Working copy", label: "Working copy", point: WORKING_COPY, note: "uncommitted changes included" }];
    for (const b of branches?.local ?? []) {
      all.push({ group: "Branches", label: b.name, point: rev(b.name), note: b.current ? "current" : undefined });
    }
    for (const r of branches?.remotes ?? []) {
      for (const b of r.branches) {
        all.push({ group: "Remote branches", label: b, point: rev(b) });
      }
    }
    for (const t of tags) {
      all.push({ group: "Tags", label: t.name, point: rev(t.name) });
    }
    for (const s of stashes) {
      all.push({ group: "Stashes", label: `stash@{${s.index}}`, point: rev(`stash@{${s.index}}`), note: s.message });
    }
    const q = query.trim().toLowerCase();
    return q ? all.filter((i) => i.label.toLowerCase().includes(q) || i.note?.toLowerCase().includes(q)) : all;
  });

  /** A typed hash or ref the lists don't hold: offered as is, checked by the backend. */
  const typed = $derived(query.trim() && !items.some((i) => i.label === query.trim()) ? query.trim() : null);

  function pick(p: ComparePoint) {
    open = false;
    query = "";
    onpick(p);
  }

  function toggle() {
    open = !open;
    if (open) {
      queueMicrotask(() => input?.focus());
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      open = false;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const first = items[0]?.point ?? (typed ? rev(typed) : null);
      if (first) {
        pick(first);
      }
    }
  }
</script>

<span class="picker">
  <button type="button" class="cur" aria-haspopup="listbox" aria-expanded={open} aria-label="{side}: {pointLabel(value)}" onclick={toggle}>
    <span class="ab {side.toLowerCase()}">{side}</span>
    <span class="lbl">{pointLabel(value)}</span>
    <span class="caret">▾</span>
  </button>
  {#if open}
    <div class="menu">
      <input
        bind:this={input}
        bind:value={query}
        placeholder="Branch, tag, stash or commit hash"
        aria-label="Find a branch, tag, stash or commit for {side}"
        onkeydown={onKeydown}
      />
      <ul role="listbox" aria-label="{side} side">
        {#if typed}
          <li><button type="button" role="option" aria-selected="false" onclick={() => pick(rev(typed))}>Use <b>{typed}</b></button></li>
        {/if}
        {#each items as item, i (item.group + item.label)}
          {#if i === 0 || items[i - 1].group !== item.group}
            <li class="grp">{item.group}</li>
          {/if}
          <li>
            <button type="button" role="option" aria-selected="false" onclick={() => pick(item.point)}>
              <span class="nm">{item.label}</span>
              {#if item.note}<span class="note">{item.note}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</span>

<style>
  .picker {
    position: relative;
  }
  .cur {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 200px;
    max-width: 320px;
    height: 24px;
    padding: 0 8px 0 4px;
  }
  .lbl {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
    font: 12px var(--mono);
  }
  .caret {
    color: var(--muted);
  }
  .ab {
    display: inline-grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    font: 800 10px var(--mono);
    color: #10141b;
    flex: none;
  }
  .ab.a {
    background: var(--yours);
  }
  .ab.b {
    background: var(--theirs);
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 40;
    width: 340px;
    max-height: 420px;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 6px;
    box-shadow: 0 10px 30px #0008;
  }
  input {
    margin: 6px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 0 6px;
    overflow-y: auto;
  }
  li.grp {
    padding: 6px 10px 2px;
    font: 600 10px var(--mono);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  li button {
    all: unset;
    box-sizing: border-box;
    display: flex;
    gap: 8px;
    width: 100%;
    padding: 2px 10px;
    cursor: pointer;
    font: 12px var(--mono);
  }
  li button:hover,
  li button:focus-visible {
    background: var(--hover);
  }
  .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .note {
    margin-left: auto;
    color: var(--muted);
    font: 11px var(--body);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 50%;
  }
</style>
