<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { type AudioInfo, audioInfo, type BinaryMeta, type BinarySide } from "$lib/api";
  import { protocolUrl } from "$lib/images";
  import type { ModelSide } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";
  import { prefs, setViewerChoice, setVolume, viewerChoice } from "$lib/prefs.svelte";
  import { audioCaption, clock, drawWave, needsWav } from "$lib/viewers/audio";
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

  type Key = "before" | "after";
  type Side = { key: Key; side: ModelSide; info: AudioInfo; el: HTMLAudioElement; url: string };
  const AB = ["b", "a"] as const;

  let sides = $state<Side[] | null>(null);
  let error = $state<string | null>(null);
  let ab = $state<"a" | "b">(untrack(() => viewerChoice(path, "ab", AB)));
  let playing = $state(false);
  let time = $state(0);
  let expanded = $state(false);
  let lanes = $state<HTMLDivElement>();
  let seq = 0;
  let frame = 0;

  function mediaUrl(side: ModelSide) {
    if (!needsWav(path)) {
      return fileUrl(repoId, path, side.at, bust);
    }
    const q = new URLSearchParams({ repo: String(repoId), path, at: side.at.at });
    if ("commit" in side.at) {
      q.set("commit", side.at.commit);
    }
    q.set("v", String(bust));
    return protocolUrl("audio", q);
  }

  async function open(key: Key, side: ModelSide): Promise<Side> {
    const info = await audioInfo(repoId, path, toVersion(side.at));
    const res = await fetch(mediaUrl(side));
    if (!res.ok) {
      throw new Error(await res.text());
    }
    const url = URL.createObjectURL(await res.blob());
    const el = document.createElement("audio");
    el.preload = "auto";
    el.src = url;
    el.onended = () => {
      if (current()?.el === el) {
        stop();
        time = info.duration;
      }
    };
    return { key, side, info, el, url };
  }

  function release(list: Side[] | null) {
    for (const s of list ?? []) {
      s.el.pause();
      s.el.removeAttribute("src");
      URL.revokeObjectURL(s.url);
    }
  }

  $effect(() => {
    const token = ++seq;
    void [repoId, path, bust, JSON.stringify(before?.at), JSON.stringify(after.at)];
    untrack(() => {
      stop();
      release(sides);
    });
    sides = null;
    error = null;
    time = 0;
    const msg = (e: unknown) => (e instanceof Error ? e.message : String(e));
    Promise.all([
      before ? open("before", before).catch(() => null) : Promise.resolve(null),
      open("after", after).catch((e) => msg(e)),
    ]).then(([b, a]) => {
      if (token !== seq) {
        release([b, a].filter((s): s is Side => !!s && typeof s !== "string"));
        return;
      }
      if (typeof a === "string") {
        release(b ? [b] : []);
        error = a;
        sides = [];
        return;
      }
      sides = b ? [b, a] : [a];
    });
  });

  onDestroy(() => {
    seq++;
    stop();
    release(sides);
  });

  const span = $derived(Math.max(0, ...(sides ?? []).map((s) => s.info.duration)));
  const compare = $derived((sides?.length ?? 0) === 2);
  const current = () => (sides ? (compare && ab === "a" ? sides[0] : sides[sides.length - 1]) : undefined);
  const bInfo = $derived(compare && sides ? sides[0].info : null);
  const aInfo = $derived(sides?.length ? sides[sides.length - 1].info : null);

  $effect(() => {
    const v = prefs.volume;
    for (const s of sides ?? []) {
      s.el.volume = v;
    }
  });

  function tick() {
    const s = current();
    if (s && playing) {
      time = s.el.currentTime;
      frame = requestAnimationFrame(tick);
    }
  }

  function start() {
    const s = current();
    if (!s) {
      return;
    }
    if (time >= s.info.duration - 0.01) {
      time = 0;
    }
    s.el.currentTime = time;
    s.el.volume = prefs.volume;
    playing = true;
    void s.el.play()?.catch?.((e: unknown) => {
      playing = false;
      error = `Can't play this version: ${e instanceof Error ? e.message : String(e)}`;
    });
    frame = requestAnimationFrame(tick);
  }

  function stop() {
    playing = false;
    cancelAnimationFrame(frame);
    for (const s of sides ?? []) {
      s.el.pause();
    }
  }

  function toggle() {
    if (playing) {
      stop();
    } else {
      start();
    }
  }

  function hear(which: "a" | "b") {
    if (which === ab) {
      return;
    }
    const was = playing;
    if (was) {
      time = current()?.el.currentTime ?? time;
      stop();
    }
    ab = which;
    setViewerChoice(path, "ab", which);
    if (was) {
      start();
    }
  }

  function seek(t: number) {
    time = Math.min(span, Math.max(0, t));
    const s = current();
    if (s) {
      s.el.currentTime = Math.min(time, s.info.duration);
    }
  }

  function clickLanes(e: MouseEvent) {
    if (!lanes || span === 0) {
      return;
    }
    const r = lanes.getBoundingClientRect();
    seek(((e.clientX - r.left) / r.width) * span);
  }

  function onkey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input, select")) {
      return;
    }
    if (e.key === " ") {
      e.preventDefault();
      toggle();
    } else if ((e.key === "a" || e.key === "b") && compare) {
      hear(e.key);
    } else if (e.key === "Escape" && expanded) {
      expanded = false;
    }
  }

  function wave(node: HTMLCanvasElement, args: { peaks: [number, number][]; duration: number; span: number }) {
    let current = args;
    const draw = () => {
      const dpr = window.devicePixelRatio || 1;
      const w = node.clientWidth;
      const h = node.clientHeight;
      node.width = Math.round(w * dpr);
      node.height = Math.round(h * dpr);
      const g = node.getContext("2d");
      if (!g || w === 0) {
        return;
      }
      g.setTransform(dpr, 0, 0, dpr, 0, 0);
      drawWave(g, current.peaks, w, h, current.duration, current.span, getComputedStyle(node).color);
    };
    const ro = new ResizeObserver(draw);
    ro.observe(node);
    draw();
    return {
      update(next: typeof args) {
        current = next;
        draw();
      },
      destroy: () => ro.disconnect(),
    };
  }

  const ext = $derived(path.slice(path.lastIndexOf(".") + 1).toUpperCase());
  const lfs = $derived(!!(meta?.after?.lfs || meta?.before?.lfs));
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="audio" class:expanded role="application" aria-label="Sound preview of {path}" tabindex="0" onkeydown={onkey}>
  <div class="bar">
    <span class="pill">{ext}{lfs ? " · LFS" : ""}</span>
    <button type="button" class="tbtn solo" onclick={() => (expanded = !expanded)} aria-pressed={expanded}
      >{expanded ? "Close" : "Expand"}</button
    >
  </div>

  {#if !sides}
    <p class="state"><LoadingBar label="Decoding the sound…" /></p>
  {:else if error && !aInfo}
    {#if meta?.after?.lfs && /isn't downloaded/.test(error)}
      <div class="state"><LfsBinaryCard {path} {meta} {onfetch} /></div>
    {:else}
      <p class="state err" role="alert">{error}</p>
    {/if}
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="lanes" bind:this={lanes} onclick={clickLanes} role="presentation">
      {#each sides as s (s.key)}
        <div class="version" class:heard={!compare || (ab === "a") === (s.key === "before")}>
          <span class="ln">{s.side.label} · {s.info.duration.toFixed(2)} s</span>
          {#each s.info.peaks as peaks, c (c)}
            <canvas
              class={s.key}
              class:thin={s.info.peaks.length > 1}
              use:wave={{ peaks, duration: s.info.duration, span }}
              aria-label="{s.side.label} waveform{s.info.peaks.length > 1 ? `, channel ${c + 1}` : ''}"
            ></canvas>
          {/each}
        </div>
      {/each}
      <div class="playhead" style:left="{span ? (time / span) * 100 : 0}%"></div>
    </div>

    <div class="transport">
      <button type="button" class="play" aria-label={playing ? "Pause" : "Play"} onclick={toggle}>{playing ? "❚❚" : "▶"}</button>
      {#if compare}
        <div class="seg" role="radiogroup" aria-label="Version heard">
          <button type="button" class="tbtn" class:on={ab === "a"} role="radio" aria-checked={ab === "a"} title="Hear {sides[0].side.label} (A)" onclick={() => hear("a")}>A</button>
          <button type="button" class="tbtn" class:on={ab === "b"} role="radio" aria-checked={ab === "b"} title="Hear {sides[1].side.label} (B)" onclick={() => hear("b")}>B</button>
        </div>
      {/if}
      <input
        type="range"
        min="0"
        max="1000"
        value={span ? Math.round((time / span) * 1000) : 0}
        aria-label="Position"
        oninput={(e) => seek((+e.currentTarget.value / 1000) * span)}
      />
      <span class="time">{clock(time, span)}</span>
      <label class="vol"
        >Volume<input
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={prefs.volume}
          aria-label="Volume"
          oninput={(e) => setVolume(+e.currentTarget.value)}
        /></label
      >
    </div>

    {#if error}<p class="err note" role="alert">{error}</p>{/if}

    {#if aInfo}
      <div class="cap">
        {#each audioCaption(bInfo, aInfo) as p, i (i)}
          {#if i > 0}<span>·</span>{/if}
          {#if i === 0}<b>{p.text}</b>{:else}<span class:d-mod={p.changed}>{p.text}</span>{/if}
        {/each}
        <span>· {aInfo.codec}</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .audio {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto auto auto;
    min-height: 260px;
    height: 100%;
    outline: none;
    background: var(--panel);
  }
  .audio.expanded {
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
  .state {
    margin: 12px 10px;
    font-size: 11.5px;
  }
  .err {
    color: var(--bad-ink);
    font-size: 11.5px;
  }
  .note {
    margin: 4px 10px;
  }
  .lanes {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 6px 0;
    cursor: text;
  }
  .version {
    position: relative;
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 60px;
    opacity: 0.55;
  }
  .version.heard {
    opacity: 1;
  }
  .version + .version {
    border-top: 1px solid var(--line);
  }
  .ln {
    position: absolute;
    top: 3px;
    left: 8px;
    z-index: 1;
    font: 10.5px var(--mono);
    padding: 1px 6px;
    border-radius: 4px;
    background: rgb(10 14 20 / 0.55);
    color: #f2f4f7;
    pointer-events: none;
  }
  canvas {
    flex: 1;
    display: block;
    width: 100%;
    min-height: 0;
  }
  canvas.before {
    color: var(--l1);
  }
  canvas.after {
    color: var(--l0);
  }
  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--fg);
    pointer-events: none;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-top: 1px solid var(--line);
  }
  .transport input[aria-label="Position"] {
    flex: 1;
  }
  .play {
    all: unset;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--l0);
    color: #fff;
    font-size: 10px;
    cursor: default;
  }
  .play:focus-visible {
    outline: 2px solid var(--fg);
  }
  .time,
  .vol {
    font: 10.5px var(--mono);
    color: var(--muted);
    white-space: nowrap;
  }
  .vol {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .vol input {
    width: 70px;
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
    padding: 1px 8px;
    color: var(--muted);
    border-right: 1px solid var(--line);
    cursor: default;
  }
  .tbtn:last-child {
    border-right: 0;
  }
  .tbtn.solo {
    border: 1px solid var(--line);
    border-radius: 5px;
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
</style>
