<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { type BinaryMeta, type BinarySide, type VideoInfo, videoInfo } from "$lib/api";
  import type { ModelSide } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";
  import { prefs, setVolume } from "$lib/prefs.svelte";
  import { fileUrl } from "$lib/viewers/route";
  import {
    diffSpans,
    frameDifference,
    hasBoxes,
    type PlayerMeta,
    quality,
    sampleTimes,
    timecode,
    videoCaption,
  } from "$lib/viewers/video";
  import LfsBinaryCard from "./LfsBinaryCard.svelte";
  import LoadingBar from "./LoadingBar.svelte";
  import PreviewCard from "./PreviewCard.svelte";

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
  type Mode = "side" | "swipe" | "onion";
  type Side = { key: Key; side: ModelSide; url: string; info: VideoInfo | null };
  const SIDES = ["before", "after"] as const;

  let sides = $state<{ before: Side | null; after: Side } | null>(null);
  let error = $state<string | null>(null);
  let unplayable = $state<string | null>(null);
  let players = $state<{ before?: HTMLVideoElement; after?: HTMLVideoElement }>({});
  let metas = $state<{ before: PlayerMeta | null; after: PlayerMeta | null }>({ before: null, after: null });
  let mode = $state<Mode>("side");
  let split = $state(0.5);
  let onion = $state(0.5);
  let playing = $state(false);
  let muted = $state(true);
  let time = $state(0);
  let expanded = $state(false);
  let scan = $state<{ done: number; total: number } | null>(null);
  let spans = $state<[number, number][] | null>(null);
  let seq = 0;
  let frame = 0;

  async function open(key: Key, side: ModelSide): Promise<Side> {
    const info = hasBoxes(path) ? await videoInfo(repoId, path, toVersion(side.at)) : null;
    const res = await fetch(fileUrl(repoId, path, side.at, bust));
    if (!res.ok) {
      throw new Error(await res.text());
    }
    return { key, side, info, url: URL.createObjectURL(await res.blob()) };
  }

  function release(s: typeof sides) {
    for (const x of [s?.before, s?.after]) {
      if (x) {
        URL.revokeObjectURL(x.url);
      }
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
    unplayable = null;
    metas = { before: null, after: null };
    spans = null;
    scan = null;
    time = 0;
    muted = true;
    const msg = (e: unknown) => (e instanceof Error ? e.message : String(e));
    Promise.all([
      before ? open("before", before).catch(() => null) : Promise.resolve(null),
      open("after", after).catch((e) => msg(e)),
    ]).then(([b, a]) => {
      if (token !== seq) {
        release({ before: b, after: typeof a === "string" ? (null as never) : a });
        return;
      }
      if (typeof a === "string") {
        release({ before: b, after: null as never });
        error = a;
        return;
      }
      sides = { before: b, after: a };
    });
  });

  onDestroy(() => {
    seq++;
    stop();
    release(sides);
  });

  const both = $derived(!!sides?.before);
  const fps = $derived(sides?.after.info?.fps ?? sides?.before?.info?.fps ?? 30);
  const duration = $derived(Math.max(metas.after?.duration ?? 0, metas.before?.duration ?? 0));

  $effect(() => {
    const v = prefs.volume;
    if (players.after) {
      players.after.volume = v;
      players.after.muted = muted;
    }
    if (players.before) {
      players.before.muted = true;
    }
  });

  function loaded(k: Key, e: Event) {
    const v = e.currentTarget as HTMLVideoElement;
    metas[k] = { width: v.videoWidth, height: v.videoHeight, duration: Number.isFinite(v.duration) ? v.duration : 0 };
  }

  function failed(k: Key, e: Event) {
    const v = e.currentTarget as HTMLVideoElement;
    if (v.error?.code === 4 || v.error?.code === 3) {
      const codec = sides?.[k]?.info?.codec;
      unplayable = codec ?? path.slice(path.lastIndexOf(".") + 1).toUpperCase();
    }
  }

  function tick() {
    const a = players.after;
    if (!a || !playing) {
      return;
    }
    time = a.currentTime;
    const b = players.before;
    if (b && Math.abs(b.currentTime - Math.min(time, b.duration || time)) > 0.08) {
      b.currentTime = time;
    }
    if (a.ended) {
      stop();
      return;
    }
    frame = requestAnimationFrame(tick);
  }

  function start() {
    const list = [players.after, players.before].filter((v): v is HTMLVideoElement => !!v);
    if (time >= duration - 0.01) {
      time = 0;
    }
    for (const v of list) {
      v.currentTime = time;
      void v.play()?.catch?.(() => {});
    }
    playing = true;
    frame = requestAnimationFrame(tick);
  }

  function stop() {
    playing = false;
    cancelAnimationFrame(frame);
    players.after?.pause();
    players.before?.pause();
  }

  function seek(t: number) {
    time = Math.min(duration, Math.max(0, t));
    for (const v of [players.after, players.before]) {
      if (v) {
        v.currentTime = time;
      }
    }
  }

  function step(dir: 1 | -1) {
    stop();
    seek(time + dir / fps);
  }

  function onkey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input")) {
      return;
    }
    if (e.key === " ") {
      e.preventDefault();
      if (playing) {
        stop();
      } else {
        start();
      }
    } else if (e.key === ",") {
      step(-1);
    } else if (e.key === ".") {
      step(1);
    } else if (e.key === "Escape" && expanded) {
      expanded = false;
    }
  }

  async function findDifferences() {
    if (!sides?.before) {
      return;
    }
    stop();
    const token = seq;
    const make = (url: string) => Object.assign(document.createElement("video"), { src: url, muted: true, preload: "auto" });
    const a = make(sides.before.url);
    const b = make(sides.after.url);
    const ready = (v: HTMLVideoElement) =>
      v.readyState >= 1 ? Promise.resolve() : new Promise<void>((r) => v.addEventListener("loadedmetadata", () => r(), { once: true }));
    try {
      await Promise.all([ready(a), ready(b)]);
      if (token !== seq) {
        return;
      }
      const times = sampleTimes(Math.min(a.duration, b.duration), fps);
      const w = 96;
      const h = Math.max(1, Math.round((w * (b.videoHeight || 9)) / (b.videoWidth || 16)));
      const canvas = Object.assign(document.createElement("canvas"), { width: w, height: h });
      const g = canvas.getContext("2d", { willReadFrequently: true });
      if (!g) {
        throw new Error("No 2D canvas to compare frames on");
      }
      const grab = async (v: HTMLVideoElement, t: number) => {
        await new Promise<void>((r) => {
          v.addEventListener("seeked", () => r(), { once: true });
          v.currentTime = t;
        });
        g.drawImage(v, 0, 0, w, h);
        return g.getImageData(0, 0, w, h).data;
      };
      const scores: number[] = [];
      scan = { done: 0, total: times.length };
      for (const t of times) {
        if (token !== seq) {
          return;
        }
        const fa = await grab(a, t);
        const fb = await grab(b, t);
        scores.push(frameDifference(fa, fb));
        scan = { done: scores.length, total: times.length };
      }
      spans = diffSpans(scores);
    } finally {
      // The hidden players hold decoders until their source goes.
      a.removeAttribute("src");
      b.removeAttribute("src");
      if (token === seq) {
        scan = null;
      }
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

  const ext = $derived(path.slice(path.lastIndexOf(".") + 1).toUpperCase());
  const lfs = $derived(!!(meta?.after?.lfs || meta?.before?.lfs));
  const caption = $derived(
    sides && metas.after
      ? videoCaption(sides.before && metas.before ? { info: sides.before.info, meta: metas.before } : null, {
          info: sides.after.info,
          meta: metas.after,
        })
      : [],
  );
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="video" class:expanded role="application" aria-label="Video preview of {path}" tabindex="0" onkeydown={onkey}>
  <div class="bar">
    <span class="pill">{ext}{lfs ? " · LFS" : ""}{metas.after ? ` · ${quality(metas.after.height)}` : ""}</span>
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
  {:else if !sides}
    <p class="state"><LoadingBar label="Loading the video…" /></p>
  {:else if unplayable}
    <PreviewCard
      {repoId}
      {path}
      title="{unplayable} can't play here"
      lines={["gitgud's webview plays H.264, VP8 and VP9; HEVC and AV1 depend on this machine, ProRes never plays."]}
      sides={[...(before ? [before] : []), after]}
    />
  {:else}
    <div class="stage {both ? mode : 'side'}" style:--split={split} style:--onion={onion}>
      {#each SIDES as k (k)}
        {@const s = sides[k]}
        {#if s}
          <div class="player {k}">
            <span class="lbl">{s.side.label}</span>
            <!-- svelte-ignore a11y_media_has_caption -->
            <video
              bind:this={players[k]}
              src={s.url}
              muted
              playsinline
              preload="auto"
              aria-label="{s.side.label} video"
              onloadedmetadata={(e) => loaded(k, e)}
              onerror={(e) => failed(k, e)}
            ></video>
          </div>
        {/if}
      {/each}
      {#if both && mode === "swipe"}
        <div class="swipe" style:left="{split * 100}%">
          <button type="button" class="handle" aria-label="Drag to swipe" onpointerdown={dragSwipe}>⇔</button>
        </div>
      {/if}
    </div>

    <div class="transport">
      <button type="button" class="play" aria-label={playing ? "Pause" : "Play"} onclick={() => (playing ? stop() : start())}
        >{playing ? "❚❚" : "▶"}</button
      >
      <button type="button" class="tbtn solo" aria-label="Previous frame" title="Previous frame (,)" onclick={() => step(-1)}>◀|</button>
      <button type="button" class="tbtn solo" aria-label="Next frame" title="Next frame (.)" onclick={() => step(1)}>|▶</button>
      <div class="scrub">
        {#each spans ?? [] as [a, b], i (i)}
          <span class="mark" style:left="{a * 100}%" style:width="{Math.max(0.5, (b - a) * 100)}%"></span>
        {/each}
        <input
          type="range"
          min="0"
          max="1000"
          value={duration ? Math.round((time / duration) * 1000) : 0}
          aria-label="Position"
          oninput={(e) => seek((+e.currentTarget.value / 1000) * duration)}
        />
      </div>
      <span class="time">{timecode(time)} / {timecode(duration)}</span>
      <button type="button" class="tbtn solo" aria-pressed={!muted} aria-label={muted ? "Unmute" : "Mute"} onclick={() => (muted = !muted)}
        >{muted ? "🔇" : "🔊"}</button
      >
      <input
        type="range"
        class="vol"
        min="0"
        max="1"
        step="0.01"
        value={prefs.volume}
        aria-label="Volume"
        oninput={(e) => setVolume(+e.currentTarget.value)}
      />
    </div>

    <div class="cap">
      {#each caption as p, i (i)}
        {#if i > 0}<span>·</span>{/if}
        {#if i === 0}<b>{p.text}</b>{:else}<span class:d-mod={p.changed}>{p.text}</span>{/if}
      {/each}
      {#if both}
        <span>·</span>
        {#if scan}
          <span>Comparing frames… {scan.done} / {scan.total}</span>
        {:else if spans}
          <span class:d-mod={spans.length > 0}>{spans.length ? `frames differ in ${spans.length} ${spans.length === 1 ? "stretch" : "stretches"}, marked on the bar` : "every sampled frame matches"}</span>
        {:else}
          <button type="button" class="tbtn solo" onclick={findDifferences}>Find where frames differ</button>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .video {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto auto;
    min-height: 360px;
    height: 100%;
    outline: none;
    background: var(--panel);
  }
  .video.expanded {
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
  .stage {
    position: relative;
    display: flex;
    gap: 8px;
    min-height: 0;
    padding: 8px;
    background: #0d1117;
  }
  .stage.side > .player {
    flex: 1;
  }
  .stage.swipe,
  .stage.onion {
    display: grid;
  }
  .stage.swipe > .player,
  .stage.onion > .player {
    grid-area: 1 / 1;
  }
  .stage.swipe > .player.after {
    clip-path: inset(0 0 0 calc(var(--split) * 100%));
  }
  .stage.onion > .player.before {
    z-index: 1;
    opacity: var(--onion);
  }
  .player {
    position: relative;
    min-width: 0;
    min-height: 0;
    display: grid;
  }
  video {
    width: 100%;
    height: 100%;
    min-height: 0;
    object-fit: contain;
    background: #000;
  }
  .lbl {
    position: absolute;
    top: 6px;
    left: 6px;
    z-index: 1;
    font: 10.5px var(--mono);
    padding: 1px 6px;
    border-radius: 4px;
    background: rgb(10 14 20 / 0.6);
    color: #f2f4f7;
    pointer-events: none;
  }
  .stage.swipe .lbl,
  .stage.onion .lbl {
    display: none;
  }
  .swipe {
    position: absolute;
    top: 8px;
    bottom: 8px;
    width: 0;
    z-index: 2;
  }
  .swipe::before {
    content: "";
    position: absolute;
    inset: 0 auto 0 -1px;
    width: 2px;
    background: #fff;
  }
  .handle {
    all: unset;
    position: absolute;
    top: 50%;
    left: -14px;
    width: 28px;
    height: 28px;
    margin-top: -14px;
    border-radius: 50%;
    background: #fff;
    color: #333;
    display: grid;
    place-items: center;
    font: 700 12px var(--mono);
    cursor: ew-resize;
    touch-action: none;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-top: 1px solid var(--line);
  }
  .scrub {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
  }
  .scrub input {
    width: 100%;
  }
  .mark {
    position: absolute;
    top: 50%;
    height: 8px;
    margin-top: -4px;
    background: var(--warn-ink);
    opacity: 0.6;
    border-radius: 2px;
    pointer-events: none;
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
  .time {
    font: 10.5px var(--mono);
    color: var(--muted);
    white-space: nowrap;
  }
  .vol {
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
    align-items: center;
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
