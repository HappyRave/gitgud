<script lang="ts">
  import { untrack } from "svelte";
  import { type Sprite, type SpriteSheet, spriteSheet } from "$lib/api";
  import { imageUrl } from "$lib/images";
  import type { ModelSide } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";
  import { setViewerChoice, viewerChoice } from "$lib/prefs.svelte";
  import { SPRITE_LAYERS, type SpriteLayer, spriteBox, spriteCaption, spriteStates } from "$lib/viewers/sprites";
  import LoadingBar from "./LoadingBar.svelte";

  let {
    repoId,
    path,
    before = null,
    after,
    bust = 0,
  }: {
    repoId: number;
    /** The texture (not its `.meta`). */
    path: string;
    before?: ModelSide | null;
    after: ModelSide;
    bust?: number;
  } = $props();

  type Mode = "side" | "swipe" | "onion";
  const LABEL: Record<SpriteLayer, string> = { slices: "Slices", pixels: "Pixels", pivots: "Pivots" };

  let sheet = $state<SpriteSheet | null>(null);
  let error = $state<string | null>(null);
  let layer = $state<SpriteLayer>(untrack(() => viewerChoice(path, "sprites", SPRITE_LAYERS)));
  let mode = $state<Mode>("side");
  let split = $state(0.5);
  let onion = $state(0.5);
  let expanded = $state(false);
  let hover = $state<string | null>(null);
  let sizes = $state<{ before: [number, number] | null; after: [number, number] | null }>({ before: null, after: null });
  let seq = 0;

  $effect(() => {
    const token = ++seq;
    void [repoId, path, bust, JSON.stringify(before?.at), JSON.stringify(after.at)];
    sheet = null;
    error = null;
    sizes = { before: null, after: null };
    spriteSheet(repoId, path, before ? toVersion(before.at) : null, toVersion(after.at)).then(
      (s) => {
        if (token === seq) {
          sheet = s;
        }
      },
      (e) => {
        if (token === seq) {
          error = e instanceof Error ? e.message : String(e);
        }
      },
    );
  });

  const states = $derived(sheet ? spriteStates(sheet) : new Map());
  const both = $derived(!!before);
  const sides = $derived([...(before ? [{ key: "before" as const, side: before }] : []), { key: "after" as const, side: after }]);
  const spritesOf = (k: "before" | "after") => (k === "before" ? (sheet?.before ?? sheet?.after ?? []) : (sheet?.after ?? []));
  const borderChanged = $derived(new Set(sheet?.changes.filter((c) => c.kinds.includes("border")).map((c) => c.name) ?? []));

  function pick(l: SpriteLayer) {
    layer = l;
    setViewerChoice(path, "sprites", l);
  }

  function shownIn(k: "before" | "after", s: Sprite) {
    const st = states.get(s.name);
    return (k === "before" && st === "added") || (k === "after" && st === "removed") ? null : (st ?? "same");
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

  const title = $derived(
    !sheet ? "" : sheet.mode === "multiple" ? `Sprite sheet · ${sheet.after.length} slices` : sheet.mode === "single" ? "Sprite" : "Texture",
  );
</script>

<div class="sprites" class:expanded>
  <div class="bar">
    {#if sheet}<span class="pill">{title}</span>{/if}
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
    <p class="state err" role="alert">{error}</p>
  {:else if !sheet}
    <p class="state"><LoadingBar label="Reading the slicing…" /></p>
  {:else}
    <div class="view">
      {#if sheet.mode !== "none"}
        <div class="chrome">
          <div class="seg" role="radiogroup" aria-label="Show">
            {#each SPRITE_LAYERS as l (l)}
              <button type="button" class="tbtn" class:on={layer === l} role="radio" aria-checked={layer === l} onclick={() => pick(l)}
                >{LABEL[l]}</button
              >
            {/each}
          </div>
        </div>
      {/if}
      <div class="pages {both ? mode : 'side'}" style:--split={split} style:--onion={onion}>
        {#each sides as { key, side } (key)}
          {@const size = sizes[key]}
          <figure class="sheet {key}">
            <figcaption>{side.label}</figcaption>
            <div class="canvas">
              <img
                src={imageUrl(repoId, path, side.at, bust)}
                alt="{side.label} texture"
                onload={(e) => {
                  const img = e.currentTarget as HTMLImageElement;
                  sizes[key] = [img.naturalWidth, img.naturalHeight];
                }}
              />
              {#if size && layer !== "pixels"}
                <svg viewBox="0 0 {size[0]} {size[1]}" preserveAspectRatio="none" aria-label="{side.label} slices">
                  {#each spritesOf(key) as s, si (si)}
                    {@const st = shownIn(key, s)}
                    {#if st}
                      {@const b = spriteBox(s, size[0], size[1])}
                      <g
                        class="sprite {st}"
                        role="img"
                        aria-label="{s.name || 'sprite'} {st}"
                        onmouseenter={() => (hover = s.name)}
                        onmouseleave={() => (hover = null)}
                      >
                        <rect x={b.left} y={b.top} width={b.width} height={b.height} vector-effect="non-scaling-stroke" />
                        {#if hover === s.name || borderChanged.has(s.name)}
                          {@const [l, bo, r, t] = s.border}
                          {#if l}<line class="nine" x1={b.left + l} x2={b.left + l} y1={b.top} y2={b.top + b.height} vector-effect="non-scaling-stroke" />{/if}
                          {#if r}<line class="nine" x1={b.left + b.width - r} x2={b.left + b.width - r} y1={b.top} y2={b.top + b.height} vector-effect="non-scaling-stroke" />{/if}
                          {#if t}<line class="nine" x1={b.left} x2={b.left + b.width} y1={b.top + t} y2={b.top + t} vector-effect="non-scaling-stroke" />{/if}
                          {#if bo}<line class="nine" x1={b.left} x2={b.left + b.width} y1={b.top + b.height - bo} y2={b.top + b.height - bo} vector-effect="non-scaling-stroke" />{/if}
                        {/if}
                        {#if layer === "pivots"}
                          <circle class="pivot" cx={b.left + s.pivot[0] * b.width} cy={b.top + (1 - s.pivot[1]) * b.height} r={Math.max(1.5, size[0] / 200)} />
                        {/if}
                      </g>
                    {/if}
                  {/each}
                </svg>
              {/if}
            </div>
          </figure>
        {/each}
        {#if both && mode === "swipe"}
          <div class="swipe" style:left="{split * 100}%">
            <button type="button" class="handle" aria-label="Drag to swipe" onpointerdown={dragSwipe}>⇔</button>
          </div>
        {/if}
      </div>
      {#if hover}<div class="hover" role="status">{hover}</div>{/if}
    </div>
    <div class="cap">
      {#each spriteCaption(sheet) as p, i (i)}
        {#if i > 0}<span>·</span>{/if}
        {#if i === 0}<b>{p.text}</b>{:else}<span class={p.tone ? `d-${p.tone}` : ""}>{p.text}</span>{/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .sprites {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    min-height: 320px;
    height: 100%;
    background: var(--panel);
  }
  .sprites.expanded {
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
  .view {
    position: relative;
    overflow: auto;
    min-height: 0;
    padding: 34px 12px 12px;
    background: repeating-conic-gradient(var(--hover) 0 25%, var(--panel) 0 50%) 0 0 / 16px 16px;
  }
  .chrome {
    position: absolute;
    top: 6px;
    right: 8px;
    z-index: 3;
    padding: 3px;
    border-radius: 7px;
    background: var(--panel);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.15);
  }
  .pages {
    position: relative;
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .pages.side > .sheet {
    flex: 1;
    min-width: 0;
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
    margin: 0;
  }
  figcaption {
    font: 10px var(--mono);
    color: var(--muted);
    margin-bottom: 3px;
  }
  .pages.swipe figcaption,
  .pages.onion figcaption {
    visibility: hidden;
  }
  .canvas {
    position: relative;
  }
  img {
    display: block;
    width: 100%;
    image-rendering: pixelated;
  }
  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .sprite rect {
    fill: transparent;
    stroke: rgb(86 180 233 / 0.85);
    stroke-width: 1;
  }
  .sprite:hover rect {
    fill: rgb(86 180 233 / 0.15);
    stroke-width: 2;
  }
  .sprite.changed rect {
    stroke: var(--warn-ink);
    stroke-width: 2;
  }
  .sprite.added rect {
    stroke: var(--add-ink);
    stroke-width: 2;
    fill: rgb(46 160 90 / 0.15);
  }
  .sprite.removed rect {
    stroke: var(--bad-ink);
    stroke-width: 2;
    stroke-dasharray: 4 3;
  }
  .nine {
    stroke: #e040fb;
    stroke-width: 1;
    stroke-dasharray: 3 2;
  }
  .pivot {
    fill: #fff;
    stroke: #111;
    stroke-width: 0.5;
  }
  .hover {
    position: sticky;
    bottom: 0;
    display: inline-block;
    margin-top: 6px;
    padding: 1px 7px;
    border-radius: 4px;
    font: 10.5px var(--mono);
    background: rgb(10 14 20 / 0.7);
    color: #f2f4f7;
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
  .tbtn:focus-visible {
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
  .d-mod {
    color: var(--warn-ink);
  }
  .d-add {
    color: var(--add-ink);
  }
  .d-del {
    color: var(--bad-ink);
  }
</style>
