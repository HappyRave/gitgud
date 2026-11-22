<script lang="ts">
  import type { Conflict } from "$lib/api";
  import { groupByKind, KIND_LABEL } from "$lib/merge/resolve";

  let {
    conflicts,
    selected,
    how,
    onselect,
  }: {
    conflicts: Conflict[];
    selected: string | null;
    /** What each resolved file was resolved with, e.g. "use both". */
    how: ReadonlyMap<string, string>;
    onselect: (path: string) => void;
  } = $props();

  const groups = $derived(groupByKind(conflicts));
  const left = $derived(conflicts.filter((c) => !c.resolved).length);

  function name(path: string) {
    return path.slice(path.lastIndexOf("/") + 1);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }
    event.preventDefault();
    const order = groups.flatMap((g) => g.conflicts.map((c) => c.path));
    const at = selected ? order.indexOf(selected) : -1;
    const next = order[Math.max(0, Math.min(order.length - 1, at + (event.key === "ArrowDown" ? 1 : -1)))];
    if (next) {
      onselect(next);
      const buttons = (event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>("[data-path]");
      [...buttons].find((b) => b.dataset.path === next)?.focus();
    }
  }
</script>

<nav class="cf" aria-label="Conflicts">
  <div class="ph">Conflicts <span class="ct">· {left ? `${left} of ${conflicts.length} left` : "all resolved"}</span><span class="r">↑↓</span></div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <ul onkeydown={onKeydown}>
    {#each groups as group (group.kind)}
      <li class="grp">{KIND_LABEL[group.kind]}<span class="r">{group.conflicts.length}</span></li>
      {#each group.conflicts as c (c.path)}
        <li>
          <button
            class="it"
            class:sel={c.path === selected}
            class:done={c.resolved}
            aria-current={c.path === selected ? "true" : undefined}
            data-path={c.path}
            title={c.path}
            onclick={() => onselect(c.path)}
          >
            <span class="st" class:ok={c.resolved}>{c.resolved ? "✓" : "U"}</span>
            <span class="n">{name(c.path)}</span>
            <span class="r">
              {#if c.resolved}
                <span class="ok">{how.get(c.path) ?? "resolved"}</span>
              {:else}
                {c.kind === "binary" ? "pick a side" : "text merge"}
              {/if}
            </span>
          </button>
        </li>
      {/each}
    {/each}
  </ul>
  <p class="foot ab">Resolved files are staged as you go. Ctrl+Z undoes a pick.</p>
</nav>

<style>
  .cf {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--panel);
    border-right: 1px solid var(--line);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    flex: 1;
  }
  .grp {
    border-top: 0;
  }
  .it {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 24px;
    padding: 0 10px;
    white-space: nowrap;
    overflow: hidden;
    cursor: default;
  }
  .it:hover {
    background: var(--hover);
  }
  .it.sel {
    background: var(--sel);
  }
  .it:focus-visible {
    outline: 2px solid var(--l0);
    outline-offset: -2px;
  }
  .st {
    font: 600 10.5px var(--mono);
    width: 14px;
    text-align: center;
    flex: none;
    color: var(--bad-ink);
  }
  .st.ok {
    color: var(--add-ink);
  }
  .n {
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .done .n {
    color: var(--muted);
  }
  .r {
    margin-left: auto;
    padding-left: 6px;
    color: var(--muted);
    font: 10.5px var(--mono);
    flex: none;
  }
  .foot {
    margin: 0;
    padding: 10px;
    border-top: 1px solid var(--line);
  }
</style>
