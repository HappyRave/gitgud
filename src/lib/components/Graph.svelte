<script lang="ts">
  import { graphRows, type RefLabel, type Row } from "$lib/api";
  import { onMount } from "svelte";
  import { drawGraph, LANE_WIDTH, laneCount, MAX_LANES } from "$lib/graph/draw";
  import { RowStore } from "$lib/graph/rows.svelte";
  import { ROW_HEIGHT, visibleRange } from "$lib/graph/window";

  let { repoId, rowCount }: { repoId: number; rowCount: number } = $props();

  const store = new RowStore((start, count) => graphRows(repoId, start, count));

  let viewport: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let scrollTop = $state(0);
  let height = $state(0);
  let lanes = $state(1);
  let selected = $state<number | null>(null);

  const range = $derived(visibleRange(scrollTop, height, rowCount));
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

  $effect(() => {
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
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(graphWidth * dpr);
    canvas.height = Math.round(height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawGraph(
      ctx,
      visible.map(({ index, row }) => ({ index, layout: row.layout, wip: row.wip })),
      scrollTop,
      graphWidth,
      height,
    );
  });

  onMount(() => {
    height = viewport.clientHeight;
  });

  function onClick(event: MouseEvent) {
    const li = (event.target as HTMLElement).closest<HTMLElement>("li[data-index]");
    if (li) {
      selected = Number(li.dataset.index);
    }
  }

  function onKey(event: KeyboardEvent) {
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (step === 0 || rowCount === 0) {
      return;
    }
    event.preventDefault();
    selected = Math.min(rowCount - 1, Math.max(0, (selected ?? -1) + step));
    const top = selected * ROW_HEIGHT;
    if (top < viewport.scrollTop) {
      viewport.scrollTop = top;
    } else if (top + ROW_HEIGHT > viewport.scrollTop + height) {
      viewport.scrollTop = top + ROW_HEIGHT - height;
    }
    scrollTop = viewport.scrollTop;
  }

  function labelText(label: RefLabel) {
    return label.kind === "head" ? "HEAD" : label.name;
  }

  function date(seconds: number) {
    const d = new Date(seconds * 1000);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }
</script>

<svelte:window onresize={() => (height = viewport.clientHeight)} />

<div class="graph">
  {#if store.error}
    <p role="alert">{store.error}</p>
  {/if}
  <canvas bind:this={canvas} style:width="{graphWidth}px" style:height="{height}px" aria-hidden="true"></canvas>
  <div class="viewport" bind:this={viewport} onscroll={() => (scrollTop = viewport.scrollTop)}>
    <ul
      class="rows"
      role="listbox"
      aria-label="Commits"
      tabindex="0"
      style:height="{rowCount * ROW_HEIGHT}px"
      onclick={onClick}
      onkeydown={onKey}
    >
      {#each visible as { index, row } (index)}
        <li
          class="row"
          role="option"
          data-index={index}
          class:wip={row.wip}
          class:selected={selected === index}
          aria-selected={selected === index}
          style:top="{index * ROW_HEIGHT}px"
          style:padding-left="{graphWidth}px"
        >
          <span class="subject">
            {#each row.labels as label (labelText(label))}
              <span class="label {label.kind}" class:current={label.kind === "branch" && label.current}>{labelText(label)}</span>
            {/each}
            <span class="text">{row.subject}</span>
          </span>
          <span class="author">{row.author}</span>
          <span class="date">{row.wip ? "" : date(row.time)}</span>
          <span class="id">{row.shortId}</span>
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
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 24px;
    display: grid;
    grid-template-columns: minmax(12rem, 1fr) minmax(0, 9rem) 8.5rem 4.5rem;
    gap: 0.75rem;
    align-items: center;
    font-size: 0.85rem;
    white-space: nowrap;
    cursor: default;
  }
  .row:hover {
    background: #f4f4f4;
  }
  .row.selected {
    background: #e3ecfd;
  }
  .wip .subject {
    font-style: italic;
    color: #666;
  }
  .subject,
  .author {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .author,
  .date,
  .id {
    color: #777;
  }
  .id {
    font-family: ui-monospace, monospace;
  }
  .label {
    display: inline-block;
    padding: 0 0.35rem;
    margin-right: 0.3rem;
    border-radius: 3px;
    font-size: 0.75rem;
    line-height: 16px;
    background: #eee;
  }
  .label.current {
    font-weight: 600;
  }
  .label.tag {
    background: #fdf1d6;
  }
  .label.remote {
    background: #e8eefb;
  }
  .label.head {
    background: #fbe3e1;
  }
</style>
