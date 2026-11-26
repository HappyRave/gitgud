<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import type { PDFDocumentProxy } from "pdfjs-dist";
  import type { BinaryMeta, BinarySide } from "$lib/api";
  import type { ModelSide } from "$lib/model/open";
  import { changedRuns, matchPages, type PageMatch, pageLabel, pdfCaption } from "$lib/viewers/pdf";
  import type { Box, PdfPageText } from "$lib/viewers/pdfdoc";
  import { fileUrl } from "$lib/viewers/route";
  import LfsBinaryCard from "./LfsBinaryCard.svelte";
  import LoadingBar from "./LoadingBar.svelte";

  let {
    repoId,
    path,
    before = null,
    after,
    bust = 0,
    meta = null,
    onfetch,
  }: {
    repoId: number;
    path: string;
    before?: ModelSide | null;
    after: ModelSide;
    bust?: number;
    meta?: BinaryMeta | null;
    onfetch?: (side: "before" | "after", meta: BinarySide) => Promise<void>;
  } = $props();

  type Mode = "side" | "swipe" | "onion";
  const SIDES = ["before", "after"] as const;
  type Loaded = { doc: PDFDocumentProxy; pages: PdfPageText[] };

  let docs = $state.raw<{ before: Loaded | null; after: Loaded } | null>(null);
  let error = $state<string | null>(null);
  let progress = $state<{ done: number; total: number }>({ done: 0, total: 0 });
  let pages = $state<PageMatch[]>([]);
  let sel = $state(0);
  let mode = $state<Mode>("side");
  let split = $state(0.5);
  let onion = $state(0.5);
  let expanded = $state(false);
  let stage = $state<HTMLDivElement>();
  let canvases = $state<{ before?: HTMLCanvasElement; after?: HTMLCanvasElement }>({});
  let boxes = $state<{ before: Box[]; after: Box[] }>({ before: [], after: [] });
  let width = $state(0);
  let seq = 0;
  let drawSeq = 0;

  async function load(side: ModelSide, count: (n: number) => void, token: number): Promise<Loaded> {
    const { openPdf, readPages } = await import("$lib/viewers/pdfdoc");
    const res = await fetch(fileUrl(repoId, path, side.at, bust));
    if (!res.ok) {
      throw new Error(await res.text());
    }
    const doc = await openPdf(await res.arrayBuffer());
    if (token === seq) {
      progress = { ...progress, total: progress.total + doc.numPages };
    }
    return { doc, pages: await readPages(doc, count, () => token === seq) };
  }

  function release(d: typeof docs) {
    void d?.before?.doc.loadingTask.destroy();
    void d?.after.doc.loadingTask.destroy();
  }

  $effect(() => {
    const token = ++seq;
    void [repoId, path, bust, JSON.stringify(before?.at), JSON.stringify(after.at)];
    untrack(() => release(docs));
    docs = null;
    error = null;
    progress = { done: 0, total: 0 };
    const done = { before: 0, after: 0 };
    const count = (k: "before" | "after") => (n: number) => {
      done[k] = n;
      if (token === seq) {
        progress = { ...progress, done: done.before + done.after };
      }
    };
    const msg = (e: unknown) => (e instanceof Error ? e.message : String(e));
    Promise.all([
      before ? load(before, count("before"), token).catch(() => null) : Promise.resolve(null),
      load(after, count("after"), token).catch((e) => msg(e)),
    ]).then(([b, a]) => {
      if (token !== seq) {
        void b?.doc.loadingTask.destroy();
        if (a && typeof a !== "string") {
          void a.doc.loadingTask.destroy();
        }
        return;
      }
      if (typeof a === "string") {
        void b?.doc.loadingTask.destroy();
        error = a;
        return;
      }
      docs = { before: b, after: a };
      pages = b
        ? matchPages(
            b.pages.map((p) => p.text),
            a.pages.map((p) => p.text),
          )
        : a.pages.map((_, i) => ({ before: null, after: i, state: "same", noText: false }));
      const first = pages.findIndex((p) => p.state !== "same");
      sel = Math.max(0, first);
    });
  });

  onDestroy(() => {
    seq++;
    release(docs);
  });

  const compare = $derived(!!docs?.before);
  const current = $derived(pages[sel] ?? null);
  const both = $derived(!!current && current.before !== null && current.after !== null);

  $effect(() => {
    if (!stage) {
      return;
    }
    const ro = new ResizeObserver(() => (width = stage?.clientWidth ?? 0));
    ro.observe(stage);
    width = stage.clientWidth;
    return () => ro.disconnect();
  });

  async function draw() {
    const token = ++drawSeq;
    const d = docs;
    const p = current;
    await tick();
    if (!d || !p || width === 0) {
      return;
    }
    const { renderPage } = await import("$lib/viewers/pdfdoc");
    const cols = mode === "side" && both ? 2 : 1;
    const w = Math.max(120, Math.floor((width - 24 - (cols - 1) * 12) / cols));
    const runs = (k: "before" | "after") => {
      const i = p[k];
      const loaded = k === "before" ? d.before : d.after;
      return i === null || !loaded ? [] : loaded.pages[i].runs;
    };
    const mark = (k: "before" | "after") => {
      if (p.state !== "changed" || p.noText) {
        return new Set<number>();
      }
      const other = k === "before" ? "after" : "before";
      return changedRuns(
        runs(k).map((r) => r.str),
        runs(other).map((r) => r.str),
      );
    };
    const next = { before: [] as Box[], after: [] as Box[] };
    for (const k of ["before", "after"] as const) {
      const i = p[k];
      const loaded = k === "before" ? d.before : d.after;
      const canvas = canvases[k];
      if (i === null || !loaded || !canvas) {
        continue;
      }
      const r = await renderPage(loaded.doc, i, canvas, w, runs(k));
      if (!r || token !== drawSeq) {
        return;
      }
      const hit = mark(k);
      next[k] = r.boxes.filter((_, n) => hit.has(n));
    }
    boxes = next;
  }

  $effect(() => {
    void [docs, current, width, mode];
    draw().catch((e) => (error = e instanceof Error ? e.message : String(e)));
  });

  function onkey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input")) {
      return;
    }
    if (e.key === "ArrowDown" || e.key === "PageDown") {
      e.preventDefault();
      sel = Math.min(pages.length - 1, sel + 1);
    } else if (e.key === "ArrowUp" || e.key === "PageUp") {
      e.preventDefault();
      sel = Math.max(0, sel - 1);
    } else if (e.key === "Escape" && expanded) {
      expanded = false;
    }
  }

  function dragSwipe(e: PointerEvent) {
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const host = handle.parentElement?.parentElement;
      if (!host) {
        return;
      }
      const r = host.getBoundingClientRect();
      split = Math.min(0.98, Math.max(0.02, (ev.clientX - r.left) / r.width));
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", () => handle.removeEventListener("pointermove", move), { once: true });
  }

  const sideLabel = (k: "before" | "after", p: PageMatch) => {
    const s = k === "before" ? before : after;
    const i = p[k];
    return `${s?.label ?? ""}${i === null ? "" : ` · p.${i + 1}`}`;
  };
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="pdf" class:expanded role="application" aria-label="PDF preview of {path}" tabindex="0" onkeydown={onkey}>
  <div class="bar">
    {#if docs}<span class="pill">PDF · {docs.after.pages.length} pages</span>{/if}
    <div class="seg" role="radiogroup" aria-label="Compare mode">
      {#each [["side", "Side by side"], ["swipe", "Swipe"], ["onion", "Onion"]] as [m, label] (m)}
        <button
          type="button"
          class="tbtn"
          class:on={both && mode === m}
          role="radio"
          aria-checked={both && mode === m}
          disabled={!both}
          onclick={() => (mode = m as Mode)}>{label}</button
        >
      {/each}
    </div>
    {#if both && mode === "onion"}
      <label class="onion">{before?.label} <input type="range" min="0" max="1" step="0.05" bind:value={onion} aria-label="Onion opacity" /></label>
    {/if}
    <button type="button" class="tbtn solo" onclick={() => (expanded = !expanded)} aria-pressed={expanded}
      >{expanded ? "Close" : "Expand"}</button
    >
  </div>

  {#if error}
    {#if meta?.after?.lfs && /isn't downloaded/.test(error)}
      <div class="state"><LfsBinaryCard {path} {meta} {onfetch} /></div>
    {:else}
      <p class="state err" role="alert">{error}</p>
    {/if}
  {:else if !docs}
    <p class="state">
      <LoadingBar
        label={progress.total ? `Reading pages… ${progress.done} / ${progress.total}` : "Opening the PDF…"}
        value={progress.total ? progress.done / progress.total : null}
      />
    </p>
  {:else}
    <div class="body">
      <nav class="rail" aria-label="Pages">
        {#each pages as p, i (i)}
          <button
            type="button"
            class="page {p.state}"
            class:scan={p.noText}
            class:on={i === sel}
            aria-current={i === sel ? "page" : undefined}
            title={p.noText ? "No text on this page: paired by position" : p.state}
            onclick={() => (sel = i)}>{pageLabel(p)}</button
          >
        {/each}
      </nav>
      <div class="stage" bind:this={stage}>
        {#if current}
          <div class="pages {both ? mode : 'side'}" style:--split={split} style:--onion={onion}>
            {#each SIDES as k (k)}
              {#if current[k] !== null}
                <div class="sheet {k}">
                  <span class="lbl">{sideLabel(k, current)}</span>
                  <canvas bind:this={canvases[k]} aria-label="{sideLabel(k, current)}"></canvas>
                  {#each boxes[k] as b, n (n)}
                    <span class="hl" style:left="{b.x}px" style:top="{b.y}px" style:width="{b.w}px" style:height="{b.h}px"></span>
                  {/each}
                </div>
              {/if}
            {/each}
            {#if both && mode === "swipe"}
              <div class="swipe" style:left="{split * 100}%">
                <button type="button" class="handle" aria-label="Drag to swipe" onpointerdown={dragSwipe}>⇔</button>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    </div>
    <div class="cap">
      {#each pdfCaption(pages, docs.after.pages.length, compare) as t, i (i)}
        {#if i > 0}<span>·</span>{/if}
        {#if i === 0}<b>{t}</b>{:else}<span>{t}</span>{/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .pdf {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    min-height: 420px;
    height: 100%;
    outline: none;
    background: var(--panel);
  }
  .pdf.expanded {
    position: fixed;
    inset: 0;
    z-index: 50;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
  }
  .bar .solo {
    margin-left: auto;
  }
  .pill {
    font: 10.5px var(--mono);
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--hover);
    color: var(--muted);
  }
  .onion {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: 10.5px var(--mono);
    color: var(--muted);
  }
  .onion input {
    width: 90px;
  }
  .state {
    margin: 12px 10px;
    font-size: 11.5px;
  }
  .err {
    color: var(--bad-ink);
  }
  .body {
    display: grid;
    grid-template-columns: 46px minmax(0, 1fr);
    min-height: 0;
  }
  .rail {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 6px;
    overflow-y: auto;
    border-right: 1px solid var(--line);
  }
  .page {
    all: unset;
    flex: none;
    height: 40px;
    display: grid;
    place-items: end center;
    padding-bottom: 2px;
    border-radius: 3px;
    border: 1px solid var(--line);
    background: var(--bg);
    font: 9.5px var(--mono);
    color: var(--muted);
    cursor: default;
  }
  .page.changed {
    border: 2px solid var(--warn-ink);
  }
  .page.added {
    border: 2px solid var(--add-ink);
    background: var(--add);
  }
  .page.removed {
    border: 2px solid var(--bad-ink);
    background: var(--del);
    text-decoration: line-through;
  }
  .page.scan {
    border-style: dashed;
  }
  .page.on {
    outline: 2px solid var(--l0);
    outline-offset: 1px;
    color: var(--fg);
  }
  .stage {
    overflow: auto;
    min-height: 0;
    padding: 12px;
    background: var(--bg);
  }
  .pages {
    position: relative;
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .pages.swipe,
  .pages.onion {
    display: grid;
  }
  .pages.swipe > .sheet,
  .pages.onion > .sheet {
    grid-area: 1 / 1;
  }
  .pages.swipe > .sheet.after {
    clip-path: inset(0 0 0 calc(var(--split) * 100%));
  }
  .pages.onion > .sheet.before {
    z-index: 1;
    opacity: var(--onion);
  }
  .sheet {
    position: relative;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.18);
    background: #fff;
  }
  canvas {
    display: block;
  }
  .lbl {
    position: absolute;
    top: -2px;
    left: 0;
    transform: translateY(-100%);
    font: 10px var(--mono);
    color: var(--muted);
    white-space: nowrap;
  }
  .pages.swipe .lbl,
  .pages.onion .lbl {
    display: none;
  }
  .hl {
    position: absolute;
    background: rgb(255 176 32 / 0.35);
    outline: 1px solid rgb(214 140 0 / 0.7);
    pointer-events: none;
  }
  .swipe {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 0;
    z-index: 2;
  }
  .swipe::before {
    content: "";
    position: absolute;
    inset: 0 auto 0 -1px;
    width: 2px;
    background: var(--l0);
  }
  .handle {
    all: unset;
    position: absolute;
    top: 40px;
    left: -14px;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: #fff;
    color: #333;
    display: grid;
    place-items: center;
    font: 700 12px var(--mono);
    cursor: ew-resize;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.35);
    touch-action: none;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
  }
  .tbtn {
    all: unset;
    font-size: 10.5px;
    padding: 1px 7px;
    color: var(--muted);
    border-right: 1px solid var(--line);
    cursor: default;
    white-space: nowrap;
  }
  .tbtn:last-child {
    border-right: 0;
  }
  .tbtn.solo {
    border: 1px solid var(--line);
    border-radius: 5px;
  }
  .tbtn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--fg);
  }
  .tbtn.on {
    background: var(--sel);
    color: var(--fg);
    font-weight: 600;
  }
  .tbtn:disabled {
    opacity: 0.4;
  }
  .tbtn:focus-visible,
  .page:focus-visible {
    outline: 2px solid var(--l0);
  }
  .cap {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 5px 10px;
    border-top: 1px solid var(--line);
    font-size: 11px;
    color: var(--muted);
  }
</style>
