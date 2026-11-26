<script lang="ts" module>
  export interface ImageSide {
    label: string;
    url: string;
  }

  let nextFilterUid = 0;
</script>

<script lang="ts">
  import { onDestroy } from "svelte";
  import { LFS_AUTO_FETCH_MAX_BYTES, type BinarySide } from "$lib/api";
  import { formatSizeDelta, imageFormatLabel } from "$lib/images";
  import { formatSize } from "$lib/merge/resolve";

  let {
    before,
    after,
    path = "",
    beforeMeta = null,
    afterMeta = null,
    onfetch,
  }: {
    before: ImageSide;
    after: ImageSide;
    path?: string;
    beforeMeta?: BinarySide | null;
    afterMeta?: BinarySide | null;
    onfetch?: (side: "before" | "after", meta: BinarySide) => Promise<void>;
  } = $props();

  const filterUid = `imgch-${++nextFilterUid}`;

  type Mode = "side" | "swipe" | "onion" | "diff";
  type Channel = "rgba" | "rgb" | "r" | "g" | "b" | "a";
  type Backdrop = "checker" | "dark" | "light";

  const ZOOM_STEPS = [0.25, 0.5, 1, 2, 4, 8, 16];

  let mode = $state<Mode>("side");
  let channel = $state<Channel>("rgba");
  let backdrop = $state<Backdrop>("checker");
  let pixelated = $state(false);
  let zoom = $state<number | "fit">("fit");
  let pan = $state({ x: 0, y: 0 });
  /** Swipe position or onion opacity of the "after" image, 0–100. */
  let mix = $state(50);
  let diffGain = $state<1 | 4 | 16>(4);
  /** Per URL: its pixel size once loaded, or why it didn't load. */
  let loaded = $state<Record<string, { w: number; h: number } | { error: string }>>({});
  let loadedImgs = $state<Record<string, HTMLImageElement>>({});
  let fetching = $state<{ before: boolean; after: boolean }>({ before: false, after: false });
  let fetchError = $state<{ before: string | null; after: string | null }>({ before: null, after: null });
  let diffCanvas = $state<HTMLCanvasElement | null>(null);
  let diffStats = $state<{ changedPct: string; maxDelta: number; sizeDiff: string | null } | null>(null);
  let diffError = $state<string | null>(null);
  /** Both images' pixels for the diff heatmap; plain, so reading them tracks nothing. */
  let pixels: {
    b: HTMLImageElement;
    a: HTMLImageElement;
    w: number;
    h: number;
    before: Uint8ClampedArray;
    after: Uint8ClampedArray;
  } | null = null;
  let stopDrag: (() => void) | null = null;

  function endDrag() {
    stopDrag?.();
    stopDrag = null;
  }

  onDestroy(() => endDrag());

  $effect(() => {
    void before.url;
    void after.url;
    loaded = {};
    loadedImgs = {};
    pixels = null;
    diffStats = null;
    diffError = null;
    fetching = { before: false, after: false };
    fetchError = { before: null, after: null };
  });

  function onLoad(url: string, event: Event) {
    const img = event.currentTarget as HTMLImageElement;
    loaded[url] = { w: img.naturalWidth, h: img.naturalHeight };
    loadedImgs[url] = img;
  }

  /** The protocol answers failures with a plain-text reason; show it. */
  async function onError(url: string) {
    loaded[url] = { error: "not in this version" };
    try {
      const response = await fetch(url);
      if (!response.ok) {
        loaded[url] = { error: await response.text() };
      }
    } catch (err) {
      console.error(`image ${url}:`, err);
    }
  }

  const both = $derived(
    [before, after].every((s) => {
      const l = loaded[s.url];
      return l && !("error" in l);
    }),
  );

  $effect(() => {
    if (!both && mode !== "side") {
      mode = "side";
    }
  });

  const formatLabel = $derived(path ? imageFormatLabel(path) : null);
  const deltaLabel = $derived(formatSizeDelta(beforeMeta?.size, afterMeta?.size));

  function isGated(meta: BinarySide | null | undefined): meta is BinarySide {
    return Boolean(meta?.lfs && meta.hydrated === false && meta.size > LFS_AUTO_FETCH_MAX_BYTES);
  }

  function info(url: string, meta?: BinarySide | null) {
    if (isGated(meta)) {
      return `not downloaded (${formatSize(meta.size)})`;
    }
    const l = loaded[url];
    return !l ? "loading…" : "error" in l ? l.error : `${l.w} × ${l.h}`;
  }

  const filterCss = $derived(channel === "rgba" ? undefined : `url(#${filterUid}-${channel})`);

  const zoomLabel = $derived(zoom === "fit" ? "Fit" : `${Math.round(zoom * 100)}%`);

  const imgStyle = $derived.by(() => {
    const parts: string[] = [];
    if (filterCss) {
      parts.push(`filter: ${filterCss}`);
    }
    if (zoom !== "fit") {
      parts.push(`transform: translate(${pan.x}px, ${pan.y}px) scale(${zoom})`);
    }
    return parts.join("; ");
  });

  function stepZoom(dir: -1 | 1) {
    const current = zoom === "fit" ? 1 : zoom;
    if (dir > 0) {
      const next = ZOOM_STEPS.find((z) => z > current + 0.001) ?? ZOOM_STEPS[ZOOM_STEPS.length - 1];
      zoom = next;
    } else {
      const rev = [...ZOOM_STEPS].reverse();
      const next = rev.find((z) => z < current - 0.001) ?? ZOOM_STEPS[0];
      zoom = next;
    }
  }

  function toggleFitZoom() {
    pan = { x: 0, y: 0 };
    zoom = zoom === "fit" ? 1 : "fit";
  }

  function resetView() {
    zoom = "fit";
    pan = { x: 0, y: 0 };
  }

  function onWheel(event: WheelEvent) {
    if (!event.ctrlKey && zoom === "fit") {
      return;
    }
    event.preventDefault();
    stepZoom(event.deltaY < 0 ? 1 : -1);
  }

  function onViewportPointerDown(event: PointerEvent) {
    if (event.button !== 0) {
      return;
    }
    if (mode === "swipe" && zoom === "fit") {
      endDrag();
      const box = event.currentTarget as HTMLElement;
      const updateMix = (clientX: number) => {
        const rect = box.getBoundingClientRect();
        if (rect.width <= 0) return;
        const pct = Math.max(0, Math.min(100, ((clientX - rect.left) / rect.width) * 100));
        mix = Math.round(pct);
      };
      updateMix(event.clientX);
      const onMove = (e: PointerEvent) => updateMix(e.clientX);
      const onUp = () => endDrag();
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
      window.addEventListener("pointercancel", onUp);
      stopDrag = () => {
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
        window.removeEventListener("pointercancel", onUp);
      };
      return;
    }
    if (zoom === "fit") {
      return;
    }
    event.preventDefault();
    endDrag();
    const startX = event.clientX - pan.x;
    const startY = event.clientY - pan.y;
    const onMove = (e: PointerEvent) => {
      pan = { x: e.clientX - startX, y: e.clientY - startY };
    };
    const onUp = () => endDrag();
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    stopDrag = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }

  async function runFetch(which: "before" | "after", meta: BinarySide) {
    if (!onfetch || fetching[which]) {
      return;
    }
    fetching = { ...fetching, [which]: true };
    fetchError = { ...fetchError, [which]: null };
    try {
      await onfetch(which, meta);
    } catch (err) {
      fetchError = { ...fetchError, [which]: String(err) };
    } finally {
      fetching = { ...fetching, [which]: false };
    }
  }

  $effect(() => {
    if (mode !== "diff" || !both || !diffCanvas) {
      return;
    }
    const bImg = loadedImgs[before.url];
    const aImg = loadedImgs[after.url];
    const bInfo = loaded[before.url];
    const aInfo = loaded[after.url];
    if (!bImg || !aImg || !bInfo || !aInfo || "error" in bInfo || "error" in aInfo) {
      return;
    }

    const w = Math.max(bInfo.w, aInfo.w, 1);
    const h = Math.max(bInfo.h, aInfo.h, 1);
    const ch = channel;
    const gain = diffGain;

    try {
      const outCtx = diffCanvas.getContext("2d");
      if (!outCtx) {
        diffError = "Pixel diff unavailable in this environment.";
        return;
      }
      // Channel and gain only redo the loop below; the pixels are read once per image pair.
      if (!pixels || pixels.b !== bImg || pixels.a !== aImg || pixels.w !== w || pixels.h !== h) {
        const read = (img: HTMLImageElement) => {
          const c = document.createElement("canvas");
          c.width = w;
          c.height = h;
          const ctx = c.getContext("2d");
          if (!ctx) {
            return null;
          }
          ctx.drawImage(img, 0, 0);
          return ctx.getImageData(0, 0, w, h).data;
        };
        const before = read(bImg);
        const after = read(aImg);
        if (!before || !after) {
          diffError = "Pixel diff unavailable in this environment.";
          return;
        }
        pixels = { b: bImg, a: aImg, w, h, before, after };
      }

      diffCanvas.width = w;
      diffCanvas.height = h;
      const dataA = pixels.before;
      const dataB = pixels.after;
      const out = outCtx.createImageData(w, h);
      const px = out.data;

      let changed = 0;
      let maxDelta = 0;
      const total = w * h;
      for (let i = 0; i < dataA.length; i += 4) {
        const dr = Math.abs(dataA[i] - dataB[i]);
        const dg = Math.abs(dataA[i + 1] - dataB[i + 1]);
        const db = Math.abs(dataA[i + 2] - dataB[i + 2]);
        const da = Math.abs(dataA[i + 3] - dataB[i + 3]);
        const d =
          ch === "r"
            ? dr
            : ch === "g"
              ? dg
              : ch === "b"
                ? db
                : ch === "a"
                  ? da
                  : ch === "rgb"
                    ? Math.max(dr, dg, db)
                    : Math.max(dr, dg, db, da);

        if (d > maxDelta) maxDelta = d;
        if (d > 0) {
          changed++;
          const amp = Math.min(255, d * gain);
          px[i] = 255;
          px[i + 1] = Math.round(amp * 0.55);
          px[i + 2] = Math.max(48, 220 - Math.round(amp * 0.6));
          px[i + 3] = 255;
        } else {
          const luma = Math.round((dataB[i] * 0.299 + dataB[i + 1] * 0.587 + dataB[i + 2] * 0.114) * 0.28);
          px[i] = luma;
          px[i + 1] = luma;
          px[i + 2] = luma;
          px[i + 3] = 255;
        }
      }

      outCtx.putImageData(out, 0, 0);
      diffError = null;
      diffStats = {
        changedPct: total > 0 ? ((changed / total) * 100).toFixed(2) : "0.00",
        maxDelta,
        sizeDiff: bInfo.w !== aInfo.w || bInfo.h !== aInfo.h ? `${bInfo.w}×${bInfo.h} → ${aInfo.w}×${aInfo.h}` : null,
      };
    } catch (err) {
      diffError = err instanceof Error ? err.message : String(err);
      diffStats = null;
    }
  });
</script>

<svg class="filters" aria-hidden="true" width="0" height="0">
  <defs>
    <filter id="{filterUid}-rgb">
      <feColorMatrix type="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 0 1" />
    </filter>
    <filter id="{filterUid}-r">
      <feColorMatrix type="matrix" values="1 0 0 0 0  1 0 0 0 0  1 0 0 0 0  0 0 0 0 1" />
    </filter>
    <filter id="{filterUid}-g">
      <feColorMatrix type="matrix" values="0 1 0 0 0  0 1 0 0 0  0 1 0 0 0  0 0 0 0 1" />
    </filter>
    <filter id="{filterUid}-b">
      <feColorMatrix type="matrix" values="0 0 1 0 0  0 0 1 0 0  0 0 1 0 0  0 0 0 0 1" />
    </filter>
    <filter id="{filterUid}-a">
      <feColorMatrix type="matrix" values="0 0 0 1 0  0 0 0 1 0  0 0 0 1 0  0 0 0 0 1" />
    </filter>
  </defs>
</svg>

<section class="imgdiff" aria-label="Image diff">
  <div class="toolbar">
    <div class="modes" role="radiogroup" aria-label="Compare">
      {#each [["side", "Side by side"], ["swipe", "Swipe"], ["onion", "Onion"], ["diff", "Diff"]] as [m, label] (m)}
        <button
          type="button"
          class="bare chip"
          class:on={mode === m}
          role="radio"
          aria-checked={mode === m}
          disabled={m !== "side" && !both}
          onclick={() => (mode = m as Mode)}>{label}</button
        >
      {/each}
    </div>

    <div class="tools">
      <div class="seg" role="radiogroup" aria-label="Channel">
        {#each [["rgba", "RGBA"], ["rgb", "RGB"], ["r", "R"], ["g", "G"], ["b", "B"], ["a", "A"]] as [c, label] (c)}
          <button
            type="button"
            class="bare tbtn"
            class:on={channel === c}
            role="radio"
            aria-checked={channel === c}
            onclick={() => (channel = c as Channel)}>{label}</button
          >
        {/each}
      </div>

      <div class="seg" role="group" aria-label="Zoom">
        <button type="button" class="bare tbtn" aria-label="Zoom out" onclick={() => stepZoom(-1)}>−</button>
        <button type="button" class="bare tbtn" aria-label="Toggle fit zoom" onclick={toggleFitZoom}>{zoomLabel}</button>
        <button type="button" class="bare tbtn" aria-label="Zoom in" onclick={() => stepZoom(1)}>+</button>
      </div>

      <button
        type="button"
        class="bare chip"
        class:on={pixelated}
        aria-pressed={pixelated}
        title="Nearest-neighbor pixel scaling"
        onclick={() => (pixelated = !pixelated)}>Pixelated</button
      >

      <div class="seg" role="radiogroup" aria-label="Backdrop">
        {#each [["checker", "Checker"], ["dark", "Dark"], ["light", "Light"]] as [bg, label] (bg)}
          <button
            type="button"
            class="bare tbtn"
            class:on={backdrop === bg}
            role="radio"
            aria-checked={backdrop === bg}
            onclick={() => (backdrop = bg as Backdrop)}>{label}</button
          >
        {/each}
      </div>

      {#if formatLabel}
        <span class="badge">{formatLabel}</span>
      {/if}
      {#if deltaLabel}
        <span class="ab">{deltaLabel}</span>
      {/if}
    </div>
  </div>

  {#if mode === "side"}
    <div class="side">
      {#each [{ which: "before" as const, side: before, meta: beforeMeta }, { which: "after" as const, side: after, meta: afterMeta }] as item (item.which)}
        {@const loadState = loaded[item.side.url]}
        {@const gated = isGated(item.meta)}
        <figure>
          <figcaption>
            <span>{item.side.label}</span>
            {#if item.meta?.lfs}
              <span class="badge lfs">LFS</span>
              <span class="pill" class:ok={item.meta.hydrated !== false} class:ptr={item.meta.hydrated === false}>
                {item.meta.hydrated === false ? "Pointer only" : "Hydrated"}
              </span>
            {/if}
          </figcaption>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="frame bg-{backdrop}"
            class:zoomed={zoom !== "fit"}
            onwheel={onWheel}
            onpointerdown={onViewportPointerDown}
            ondblclick={resetView}
          >
            {#if gated && item.meta}
              <div class="gate">
                <span>LFS object not downloaded ({formatSize(item.meta.size)})</span>
                {#if onfetch}
                  <button
                    type="button"
                    class="fetchbtn"
                    disabled={fetching[item.which]}
                    onclick={() => item.meta && runFetch(item.which, item.meta)}
                  >
                    {fetching[item.which] ? "Fetching…" : `Fetch LFS object (${formatSize(item.meta.size)})`}
                  </button>
                {/if}
                {#if fetchError[item.which]}
                  <span class="ferr" role="alert">{fetchError[item.which]}</span>
                {/if}
              </div>
            {:else if !(loadState && "error" in loadState)}
              <img
                src={item.side.url}
                alt={item.side.label}
                crossorigin="anonymous"
                class:pixelated
                style={imgStyle}
                onload={(e) => onLoad(item.side.url, e)}
                onerror={() => onError(item.side.url)}
              />
            {:else if item.meta?.lfs && onfetch}
              <div class="gate">
                <span>{loadState.error}</span>
                <button
                  type="button"
                  class="fetchbtn"
                  disabled={fetching[item.which]}
                  onclick={() => item.meta && runFetch(item.which, item.meta)}
                >
                  {fetching[item.which] ? "Fetching…" : "Fetch LFS object"}
                </button>
                {#if fetchError[item.which]}
                  <span class="ferr" role="alert">{fetchError[item.which]}</span>
                {/if}
              </div>
            {/if}
          </div>
          <span class="meta ab">
            <span>{info(item.side.url, item.meta)}</span>
            {#if item.meta}
              <span>· {formatSize(item.meta.size)}</span>
            {/if}
            {#if item.meta?.oid}
              <span class="mono" title="sha256:{item.meta.oid}">· {item.meta.oid.slice(0, 8)}</span>
            {/if}
          </span>
        </figure>
      {/each}
    </div>
  {:else if mode === "diff"}
    <div class="diffbar">
      <div class="seg" role="radiogroup" aria-label="Diff gain">
        {#each [1, 4, 16] as g (g)}
          <button
            type="button"
            class="bare tbtn"
            class:on={diffGain === g}
            role="radio"
            aria-checked={diffGain === g}
            onclick={() => (diffGain = g as 1 | 4 | 16)}>{g}×</button
          >
        {/each}
      </div>
      {#if diffStats}
        <span class="ab">{diffStats.changedPct}% changed · max Δ {diffStats.maxDelta}/255</span>
        {#if diffStats.sizeDiff}
          <span class="pill ptr">Dimensions differ: {diffStats.sizeDiff}</span>
        {/if}
      {/if}
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="stack bg-{backdrop}"
      class:zoomed={zoom !== "fit"}
      onwheel={onWheel}
      onpointerdown={onViewportPointerDown}
      ondblclick={resetView}
    >
      {#if diffError}
        <p class="ab errpad" role="status">{diffError}</p>
      {:else}
        <canvas bind:this={diffCanvas} class:pixelated style={imgStyle} aria-label="Pixel difference heatmap"></canvas>
      {/if}
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="stack bg-{backdrop}"
      class:zoomed={zoom !== "fit"}
      class:swipable={mode === "swipe" && zoom === "fit"}
      style:--mix="{mix}%"
      onwheel={onWheel}
      onpointerdown={onViewportPointerDown}
      ondblclick={resetView}
    >
      <img src={before.url} alt={before.label} crossorigin="anonymous" class:pixelated style={imgStyle} />
      <img
        class="top"
        class:swipe={mode === "swipe"}
        class:pixelated
        src={after.url}
        alt={after.label}
        crossorigin="anonymous"
        style:opacity={mode === "onion" ? mix / 100 : 1}
        style:filter={filterCss}
        style:transform={zoom !== "fit" ? `translate(${pan.x}px, ${pan.y}px) scale(${zoom})` : undefined}
      />
      {#if mode === "swipe"}<span class="line"></span>{/if}
    </div>
    <label class="slider">
      <span class="ab">{before.label}</span>
      <input type="range" min="0" max="100" bind:value={mix} aria-label={mode === "swipe" ? "Swipe position" : "Opacity of the new image"} />
      <span class="ab">{after.label}</span>
    </label>
  {/if}
</section>

<style>
  .filters {
    position: absolute;
    width: 0;
    height: 0;
    pointer-events: none;
  }
  .imgdiff {
    display: grid;
    gap: 8px;
    padding: 8px 10px;
  }
  .toolbar,
  .diffbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .modes,
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
  }
  .tools {
    margin-left: auto;
  }
  .seg {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
  }
  .tbtn {
    all: unset;
    font-size: 10.5px;
    padding: 1px 6px;
    color: var(--muted);
    cursor: default;
    border-right: 1px solid var(--line);
  }
  .tbtn:last-child {
    border-right: 0;
  }
  .tbtn:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .tbtn.on {
    background: var(--sel);
    color: var(--fg);
    font-weight: 600;
  }
  .chip {
    all: unset;
    font-size: 11px;
    padding: 1px 9px;
    border-radius: 999px;
    border: 1px solid var(--line);
    color: var(--muted);
    cursor: default;
  }
  .chip.on {
    background: var(--sel);
    color: var(--fg);
    border-color: transparent;
  }
  .chip:disabled {
    opacity: 0.45;
  }
  .badge {
    font: 700 10px var(--mono);
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--hover);
    color: var(--fg);
  }
  .badge.lfs {
    background: var(--sel);
    color: var(--l0);
  }
  .pill {
    font: 600 10px var(--mono);
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--hover);
    color: var(--muted);
  }
  .pill.ok {
    background: var(--add);
    color: var(--add-ink);
  }
  .pill.ptr {
    background: var(--warn);
    color: var(--warn-ink);
  }
  .side {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  figure {
    margin: 0;
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  figcaption {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
    font: 600 11.5px var(--body);
  }
  .frame,
  .stack {
    display: grid;
    place-items: center;
    min-height: 100px;
    border: 1px solid var(--line);
    border-radius: 4px;
    overflow: hidden;
  }
  .bg-checker {
    background: repeating-conic-gradient(var(--hover) 0 25%, var(--panel) 0 50%) 0 0 / 14px 14px;
  }
  .bg-dark {
    background: #11151c;
  }
  .bg-light {
    background: #f4f6f9;
  }
  .frame.zoomed,
  .stack.zoomed {
    height: 320px;
    cursor: grab;
  }
  .frame.zoomed:active,
  .stack.zoomed:active {
    cursor: grabbing;
  }
  .stack.swipable {
    cursor: ew-resize;
  }
  img,
  canvas {
    max-width: 100%;
    max-height: 320px;
    image-rendering: auto;
    user-select: none;
    -webkit-user-drag: none;
  }
  .zoomed img,
  .zoomed canvas {
    max-width: none;
    max-height: none;
    transform-origin: center center;
  }
  img.pixelated,
  canvas.pixelated {
    image-rendering: pixelated;
  }
  .gate {
    display: grid;
    gap: 6px;
    justify-items: center;
    padding: 14px 10px;
    text-align: center;
    color: var(--muted);
    font-size: 11.5px;
  }
  .fetchbtn {
    height: 24px;
    padding: 0 9px;
    font-size: 11.5px;
  }
  .ferr {
    color: var(--bad-ink);
    font-size: 11px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .mono {
    font-family: var(--mono);
  }
  .stack {
    position: relative;
  }
  .stack img {
    grid-area: 1 / 1;
  }
  .stack .top.swipe {
    clip-path: inset(0 0 0 var(--mix));
  }
  .line {
    position: absolute;
    top: 0;
    bottom: 0;
    left: var(--mix);
    width: 2px;
    background: var(--l0);
    pointer-events: none;
  }
  .slider {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .slider input {
    flex: 1;
  }
  .errpad {
    padding: 16px;
  }
</style>
