<script lang="ts">
  import { graphRows, type RefLabel, type Row } from "$lib/api";
  import { onMount, tick, untrack } from "svelte";
  import { DEFAULT_THEME, drawGraph, LANE_WIDTH, laneCount, laneVar, MAX_LANES, readTheme } from "$lib/graph/draw";
  import { RowStore } from "$lib/graph/rows.svelte";
  import { ROW_HEIGHT, visibleRange } from "$lib/graph/window";
  import { graphEditKey } from "$lib/keys";
  import { prefs } from "$lib/prefs.svelte";
  import { age, fullDate } from "$lib/time";

  let {
    repoId,
    rowCount,
    selected = $bindable(null),
    onselect,
    oncopy,
    hits = null,
    rows: fetchRows = (start, count) => graphRows(repoId, start, count),
    onedit,
    onmenu,
    oncompare,
    renaming = null,
    onrename,
    rowsVersion = 0,
    fadedIds = null,
  }: {
    /** Bump to drop loaded rows and fetch them again (a new plan preview). */
    rowsVersion?: number;
    /** Commit OIDs to render faded (e.g. still-on-origin commits after rewrite). */
    fadedIds?: ReadonlySet<string> | null;
    /** Rows matching the find bar, marked in the gutter. */
    hits?: ReadonlySet<number> | null;
    repoId: number;
    rowCount: number;
    /** Index of the selected row. */
    selected?: number | null;
    /** The selected row once it is loaded, or `null` when the selection is cleared. */
    onselect?: (row: Row | null) => void;
    /** Ctrl+C on a commit row: its full hash. */
    oncopy?: (row: Row) => void;
    /** Where rows come from; a history plan passes its preview. */
    rows?: (start: number, count: number) => Promise<Row[]>;
    /** R / F / D / Shift+E on a commit row (spec §3.4). */
    onedit?: (row: Row, edit: "reword" | "fixup" | "drop" | "plan") => void;
    /** Right-click on a commit row. */
    onmenu?: (row: Row, x: number, y: number) => void;
    /** Ctrl+click on a second row: compare the selected row (A) with it (B) (R12). */
    oncompare?: (a: Row, b: Row) => void;
    /** Row whose subject is being renamed inline (R on the tip). */
    renaming?: number | null;
    /** The new subject on Enter, `null` on Esc. */
    onrename?: (subject: string | null) => void;
  } = $props();

  const store = new RowStore((start, count) => fetchRows(start, count));
  const listId = $props.id();

  let viewport: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let scrollTop = $state(0);
  let height = $state(0);
  let lanes = $state(1);
  let theme = $state(DEFAULT_THEME);
  let dpr = $state(typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1);

  const rowHeight = $derived(prefs.density === "compact" ? 20 : ROW_HEIGHT);
  const range = $derived(visibleRange(scrollTop, height, rowCount, 10, rowHeight));
  const visible = $derived.by(() => {
    void store.version;
    const rows: { index: number; row: Row }[] = [];
    for (let i = range.start; i < range.end; i++) {
      const row = store.get(i);
      if (row) {
        rows.push({ index: i, row });
      }
    }
    return rows;
  });
  const graphWidth = $derived(Math.min(lanes, MAX_LANES) * LANE_WIDTH + 8);

  let loadedVersion = untrack(() => rowsVersion);
  $effect(() => {
    if (rowsVersion !== loadedVersion) {
      loadedVersion = rowsVersion;
      store.reset();
    }
    store.ensure(range);
  });

  $effect(() => {
    for (const { row } of visible) {
      lanes = Math.max(lanes, laneCount(row.layout));
    }
  });

  $effect(() => {
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      console.error("graph canvas has no 2d context");
      return;
    }
    canvas.width = Math.round(graphWidth * dpr);
    canvas.height = Math.round(height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawGraph(
      ctx,
      visible.map(({ index, row }) => ({
        index,
        layout: row.layout,
        wip: row.wip,
        ghost: row.after != null || row.proposal != null,
        faded: !!row.faded || !!fadedIds?.has(row.id),
        stash: row.stash != null,
      })),
      scrollTop,
      graphWidth,
      height,
      theme,
      rowHeight,
    );
  });

  $effect(() => {
    void prefs.theme;
    theme = readTheme(document.documentElement);
  });

  onMount(() => {
    height = viewport.clientHeight;
    theme = readTheme(document.documentElement);
    const dark = window.matchMedia?.("(prefers-color-scheme: dark)");
    const retheme = () => (theme = readTheme(document.documentElement));
    dark?.addEventListener("change", retheme);

    let stopDprWatch: (() => void) | undefined;
    const watchDpr = () => {
      stopDprWatch?.();
      const currentDpr = window.devicePixelRatio || 1;
      dpr = currentDpr;
      const mq = window.matchMedia?.(`(resolution: ${currentDpr}dppx)`);
      if (!mq) {
        stopDprWatch = undefined;
        return;
      }
      const onDprChange = () => watchDpr();
      mq.addEventListener("change", onDprChange);
      stopDprWatch = () => mq.removeEventListener("change", onDprChange);
    };
    watchDpr();

    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== "undefined") {
      observer = new ResizeObserver((entries) => {
        const entry = entries[0];
        const measured = entry ? Math.round(entry.contentRect.height) : 0;
        height = measured > 0 ? measured : viewport.clientHeight;
      });
      observer.observe(viewport);
    }

    return () => {
      dark?.removeEventListener("change", retheme);
      stopDprWatch?.();
      observer?.disconnect();
    };
  });

  let list = $state<HTMLElement | null>(null);

  /** Keyboard focus on the rows (Ctrl+2). */
  export function focus() {
    list?.focus();
  }

  function onClick(event: MouseEvent) {
    const li = (event.target as HTMLElement).closest<HTMLElement>("li[data-index]");
    if (!li) {
      return;
    }
    const index = Number(li.dataset.index);
    if ((event.ctrlKey || event.metaKey) && oncompare && selected !== null && selected !== index) {
      const a = store.get(selected);
      const b = store.get(index);
      if (a && b) {
        oncompare(a, b);
        return;
      }
    }
    select(index);
  }

  /** A selected row that wasn't loaded yet: reported once its chunk arrives. */
  let unreported = $state<number | null>(null);

  $effect(() => {
    void store.version;
    if (unreported === null) {
      return;
    }
    const row = store.get(unreported);
    if (row) {
      unreported = null;
      onselect?.(row);
    }
  });

  function select(index: number | null) {
    selected = index;
    const row = index === null ? null : store.get(index);
    unreported = index !== null && !row ? index : null;
    if (row !== undefined || index === null) {
      onselect?.(row ?? null);
    }
  }

  /** Selects row `index` and scrolls it to the middle of the view (find, Ctrl+F). */
  export function reveal(index: number) {
    select(index);
    const top = index * rowHeight;
    if (top < viewport.scrollTop || top + rowHeight > viewport.scrollTop + height) {
      viewport.scrollTop = Math.max(0, top - height / 2);
      scrollTop = viewport.scrollTop;
    }
  }

  /** A row edits can act on: a real commit (not WIP, ghost, proposal, or stash). */
  const editable = (row: Row | undefined): row is Row =>
    !!row && !row.wip && row.after == null && row.stash == null && row.proposal == null;

  function onContextMenu(event: MouseEvent) {
    const li = (event.target as HTMLElement).closest<HTMLElement>("li[data-index]");
    const index = li ? Number(li.dataset.index) : null;
    const row = index === null ? undefined : store.get(index);
    if (!onmenu || !editable(row)) {
      return;
    }
    event.preventDefault();
    select(index);
    onmenu(row, event.clientX, event.clientY);
  }

  let renameInput = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (renaming !== null) {
      tick().then(() => {
        renameInput?.focus();
        renameInput?.select();
      });
    }
  });

  function onRenameKey(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Enter") {
      event.preventDefault();
      onrename?.(renameInput?.value ?? "");
    } else if (event.key === "Escape") {
      event.preventDefault();
      onrename?.(null);
      list?.focus();
    }
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape") {
      select(null);
      return;
    }
    const edit = graphEditKey(event);
    if (edit && onedit && selected !== null) {
      const row = store.get(selected);
      if (editable(row)) {
        event.preventDefault();
        onedit(row, edit);
      }
      return;
    }
    if (event.ctrlKey && event.key.toLowerCase() === "c" && selected !== null) {
      const row = store.get(selected);
      if (row && !row.wip) {
        event.preventDefault();
        oncopy?.(row);
      }
      return;
    }
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (step === 0 || rowCount === 0) {
      return;
    }
    event.preventDefault();
    const next = Math.min(rowCount - 1, Math.max(0, (selected ?? -1) + step));
    select(next);
    const top = next * rowHeight;
    if (top < viewport.scrollTop) {
      viewport.scrollTop = top;
    } else if (top + rowHeight > viewport.scrollTop + height) {
      viewport.scrollTop = top + rowHeight - height;
    }
    scrollTop = viewport.scrollTop;
  }

  function labelText(label: RefLabel) {
    return label.kind === "head" ? "HEAD" : label.name;
  }
</script>

<div class="graph" style:--row-h="{rowHeight}px">
  {#if store.error}
    <p role="alert">{store.error}</p>
  {/if}
  <canvas bind:this={canvas} style:width="{graphWidth}px" style:height="{height}px" aria-hidden="true"></canvas>
  <div class="viewport" bind:this={viewport} onscroll={() => (scrollTop = viewport.scrollTop)}>
    <ul
      bind:this={list}
      class="rows"
      role="listbox"
      aria-label="Commits"
      aria-activedescendant={selected != null ? `${listId}-row-${selected}` : undefined}
      tabindex="0"
      style:height="{rowCount * rowHeight}px"
      onclick={onClick}
      onkeydown={onKey}
      oncontextmenu={onContextMenu}
    >
      {#each visible as { index, row } (index)}
        <li
          id="{listId}-row-{index}"
          class="row"
          role="option"
          data-index={index}
          class:wip={row.wip}
          class:ghost={row.after != null || row.proposal != null}
          class:faded={!!row.faded || !!fadedIds?.has(row.id)}
          class:selected={selected === index}
          class:hit={hits?.has(index)}
          aria-selected={selected === index}
          aria-setsize={rowCount}
          aria-posinset={index + 1}
          style:top="{index * rowHeight}px"
          style:padding-left="{graphWidth}px"
        >
          <span class="subject">
            {#each row.labels as label (labelText(label))}
              <span
                class="label {label.kind}"
                class:current={label.kind === "branch" && label.current}
                style:--lane={laneVar(row.layout.node)}
                title={label.kind === "branch" && label.current ? "Current branch" : undefined}>{labelText(label)}</span
              >
            {/each}
            {#if renaming === index}
              <input
                bind:this={renameInput}
                class="rename"
                aria-label="New commit subject"
                value={row.subject}
                onkeydown={onRenameKey}
                onblur={() => onrename?.(null)}
              />
              <span class="tag">Enter save · Esc cancel</span>
            {:else}
              <span class="text">{row.subject}</span>
              {#if row.after}<span class="tag">{row.after}</span>{/if}
            {/if}
          </span>
          <span class="author">{row.wip ? "" : row.author}</span>
          <span class="date" title={row.wip ? undefined : fullDate(row.time)}>{row.wip ? "" : age(row.time)}</span>
          <span class="id">{row.wip ? "" : row.shortId}</span>
        </li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .graph {
    position: relative;
    height: 100%;
    min-height: 0;
  }
  canvas {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
    z-index: 1;
  }
  .viewport {
    height: 100%;
    overflow-y: auto;
  }
  .rows {
    position: relative;
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .rows:focus {
    outline: none;
  }
  .rows:focus-visible .row.selected {
    box-shadow: inset 2px 0 0 var(--l0);
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--row-h, 24px);
    display: grid;
    grid-template-columns: minmax(10rem, 1fr) minmax(0, 8rem) 4.5rem 4.5rem;
    gap: 10px;
    padding-right: 10px;
    align-items: center;
    white-space: nowrap;
    cursor: default;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--sel);
  }
  .row.hit .text {
    background: color-mix(in srgb, var(--l4) 30%, transparent);
    border-radius: 2px;
  }
  .wip .text {
    font-style: italic;
    color: var(--muted);
  }
  .ghost .text {
    font-style: italic;
  }
  .faded > * {
    opacity: 0.45;
  }
  .tag {
    flex: none;
    font: 10px var(--mono);
    color: var(--muted);
    border: 1px dashed var(--line);
    border-radius: 3px;
    padding: 0 4px;
  }
  .rename {
    flex: 1 1 auto;
    min-width: 170px;
    height: 18px;
    padding: 0 4px;
    font: inherit;
    border: 1.5px solid var(--l0);
    border-radius: 3px;
    background: var(--panel);
    color: var(--fg);
  }
  .subject {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    overflow: hidden;
  }
  .text,
  .author {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .author,
  .date,
  .id {
    color: var(--muted);
    font-size: 11.5px;
  }
  .id {
    font-family: var(--mono);
    text-align: right;
  }
  .label {
    flex: none;
    max-width: 190px;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 5px;
    border-radius: 3px;
    border: 1px solid currentColor;
    font: 600 10.5px/16px var(--mono);
    color: var(--lane);
  }
  .label.current {
    background: var(--lane);
    border-color: transparent;
    color: var(--panel);
  }
  .label.current::before {
    content: "✓ ";
  }
  .label.remote {
    opacity: 0.75;
    font-weight: 400;
  }
  .label.tag {
    border-style: dashed;
    color: var(--muted);
  }
  .label.head {
    background: var(--warn);
    color: var(--warn-ink);
    border-color: transparent;
  }
  .label.stash {
    border-style: dashed;
    color: var(--l3);
  }
  .label.proposal {
    border-style: dashed;
    color: var(--l0);
  }
</style>
