<script lang="ts">
  import { untrack } from "svelte";
  import { type ClipCurves, clipCurves, clipUsers, type CurveTrack } from "$lib/api";
  import type { ModelSide } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";
  import { setViewerChoice, viewerChoice } from "$lib/prefs.svelte";
  import { byObject, CURVE_MODES, type CurveMode, curveCaption, curvePath, initialSelection, range, trackId } from "$lib/viewers/curves";
  import LoadingBar from "./LoadingBar.svelte";
  import ModelViewer from "./ModelViewer.svelte";
  import SideError from "./SideError.svelte";

  let {
    repoId,
    path,
    before = null,
    after,
    bust = 0,
  }: {
    repoId: number;
    path: string;
    before?: ModelSide | null;
    after: ModelSide;
    bust?: number;
  } = $props();

  /** The lane colours, one per plotted track. */
  const COLORS = ["#0072b2", "#d55e00", "#009e73", "#cc79a7", "#e69f00", "#56b4e9", "#8c6bb1", "#999933"];
  const W = 1000;
  const H = 300;
  const PAD = 14;

  // Raw: every key of every track, never changed in place.
  let clip = $state.raw<ClipCurves | null>(null);
  let error = $state<string | null>(null);
  let users = $state<string[] | null>(null);
  let user = $state<string | null>(null);
  let selected = $state<Set<string>>(new Set());
  let mode = $state<CurveMode>(untrack(() => viewerChoice(path, "curves", CURVE_MODES)));
  let expanded = $state(false);
  let seq = 0;

  $effect(() => {
    const token = ++seq;
    void [repoId, path, bust, JSON.stringify(before?.at), JSON.stringify(after.at)];
    clip = null;
    error = null;
    users = null;
    clipCurves(repoId, path, before ? toVersion(before.at) : null, toVersion(after.at)).then(
      (c) => {
        if (token === seq) {
          clip = c;
          selected = initialSelection(c.tracks);
        }
      },
      (e) => {
        if (token === seq) {
          error = e instanceof Error ? e.message : String(e);
        }
      },
    );
    clipUsers(repoId, path, toVersion(after.at)).then(
      (u) => {
        if (token === seq) {
          users = u;
          user = u[0] ?? null;
        }
      },
      () => {
        if (token === seq) {
          users = [];
        }
      },
    );
  });

  const compare = $derived(!!clip && clip.keysBefore !== null);
  const plotted = $derived(clip ? clip.tracks.filter((t) => selected.has(trackId(t))) : []);
  const color = (t: CurveTrack) => COLORS[plotted.indexOf(t) % COLORS.length];
  const bounds = $derived.by(() => {
    let lo = Infinity;
    let hi = -Infinity;
    for (const t of plotted) {
      for (const keys of [t.before, t.after]) {
        if (keys?.length) {
          const [a, b] = range(keys, clip?.duration ?? 1, t.approximated);
          lo = Math.min(lo, a);
          hi = Math.max(hi, b);
        }
      }
    }
    if (!Number.isFinite(lo)) {
      return [0, 1] as const;
    }
    const pad = hi - lo < 1e-6 ? 1 : (hi - lo) * 0.08;
    return [lo - pad, hi + pad] as const;
  });
  const y = (v: number) => PAD + (1 - (v - bounds[0]) / (bounds[1] - bounds[0])) * (H - 2 * PAD);
  const x = (t: number) => (clip && clip.duration > 0 ? (t / clip.duration) * W : 0);
  const ticks = $derived(clip ? Array.from({ length: 5 }, (_, i) => (i / 4) * clip!.duration) : []);
  const mark = { same: "", changed: "~", added: "+", removed: "−" } as const;

  function toggle(t: CurveTrack) {
    const id = trackId(t);
    const next = new Set(selected);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selected = next;
  }

  function pick(m: CurveMode) {
    mode = m;
    setViewerChoice(path, "curves", m);
  }

  const fmt = (v: number) => (Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(2));
</script>

<div class="curves" class:expanded>
  <div class="bar">
    {#if clip}<span class="pill">Clip · {clip.duration.toFixed(2)} s{clip.looping ? " · loop" : ""}</span>{/if}
    <button type="button" class="tbtn solo" onclick={() => (expanded = !expanded)} aria-pressed={expanded}
      >{expanded ? "Close" : "Expand"}</button
    >
  </div>

  {#if error}
    <p class="state err" role="alert">{error}</p>
  {:else if !clip}
    <p class="state"><LoadingBar label="Reading the curves…" /></p>
  {:else}
    <div class="view">
      <div class="chrome">
        <div class="seg" role="radiogroup" aria-label="Show">
          {#each CURVE_MODES as m (m)}
            <button
              type="button"
              class="tbtn"
              class:on={mode === m}
              role="radio"
              aria-checked={mode === m}
              disabled={m === "3d" && users !== null && users.length === 0}
              title={m === "3d" && users?.length === 0 ? "No prefab's Animator plays this clip" : undefined}
              onclick={() => pick(m)}>{m === "3d" ? "3D" : "Curves"}</button
            >
          {/each}
        </div>
        {#if mode === "3d" && users && users.length > 1}
          <select aria-label="Prefab that plays this clip" bind:value={user}>
            {#each users as u (u)}<option value={u}>{u.slice(u.lastIndexOf("/") + 1)}</option>{/each}
          </select>
        {/if}
      </div>

      {#if mode === "3d" && user}
        {#key user}
          <ModelViewer {repoId} path={user} {before} {after} {bust} startClip={clip.name} />
        {/key}
      {:else}
        {#if before && clip.beforeError}
          <SideError label={before.label} error={clip.beforeError} />
        {/if}
        <div class="editor">
          <ul class="tracks" aria-label="Tracks">
            {#each byObject(clip.tracks) as [obj, tracks] (obj)}
              <li class="obj">{obj || "(root)"}</li>
              {#each tracks as t (trackId(t))}
                <li>
                  <label class="track {t.state}">
                    <input type="checkbox" checked={selected.has(trackId(t))} onchange={() => toggle(t)} />
                    {#if selected.has(trackId(t))}<span class="sw" style:background={color(t)}></span>{/if}
                    <span class="name">{t.label}</span>
                    {#if t.state !== "same"}<b class="st">{mark[t.state]}</b>{/if}
                  </label>
                </li>
              {/each}
            {/each}
            {#each byObject(clip.flipbooks) as [obj, books] (obj)}
              <li class="obj">{obj || "(root)"}</li>
              {#each books as f (trackId(f))}
                <li class="track {f.state} flip"><span class="name">{f.label}</span>{#if f.state !== "same"}<b class="st">{mark[f.state]}</b>{/if}</li>
              {/each}
            {/each}
          </ul>
          <div class="plot">
            <div class="canvas">
              <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" aria-label="Curves">
                {#each ticks as t, i (i)}
                  <line class="grid" x1={x(t)} x2={x(t)} y1="0" y2={H} />
                {/each}
                {#each plotted as t (trackId(t))}
                  {#if compare && t.before && t.state !== "same"}
                    <path class="before" d={curvePath(t.before, clip.duration, W, y, t.approximated)} stroke={color(t)} />
                  {/if}
                  {#if t.after}
                    <path class="after" d={curvePath(t.after, clip.duration, W, y, t.approximated)} stroke={color(t)} />
                  {:else if t.before}
                    <path class="before" d={curvePath(t.before, clip.duration, W, y, t.approximated)} stroke={color(t)} />
                  {/if}
                {/each}
              </svg>
              {#each plotted as t (trackId(t))}
                {#each t.after ?? [] as k, i (i)}
                  <span
                    class="key"
                    style:left="{(x(k.time) / W) * 100}%"
                    style:top="{(y(k.value) / H) * 100}%"
                    style:background={color(t)}
                    title="{t.label} {fmt(k.value)} at {k.time.toFixed(2)} s"
                  ></span>
                {/each}
              {/each}
            </div>
            <div class="axis">
              {#each ticks as t, i (i)}<span>{t.toFixed(2)} s</span>{/each}
            </div>
            <div class="vals"><span>{fmt(bounds[1])}</span><span>{fmt(bounds[0])}</span></div>
            {#each clip.flipbooks as f (trackId(f))}
              <div class="flip {f.state}" aria-label="{f.path} {f.label}">
                {#each f.after ?? f.before ?? [] as [t, name], i (i)}
                  <span class="frame" style:left="{(x(t) / W) * 100}%" title="{t.toFixed(2)} s">{name}</span>
                {/each}
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
    <div class="cap">
      {#if compare}<span>dashed = {before?.label} · solid = {after.label} ·</span>{/if}
      {#each curveCaption(clip, compare) as p, i (i)}
        {#if i > 0}<span>·</span>{/if}
        <span class={p.tone ? `d-${p.tone}` : ""}>{p.text}</span>
      {/each}
      {#if clip.tracks.some((t) => t.approximated)}<span>· quaternion rotations shown as Euler angles</span>{/if}
    </div>
  {/if}
</div>

<style>
  .curves {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    min-height: 360px;
    height: 100%;
    background: var(--panel);
  }
  .curves.expanded {
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
  }
  .view {
    position: relative;
    display: grid;
    min-height: 0;
  }
  .view > :global(*) {
    min-height: 0;
  }
  .chrome {
    position: absolute;
    top: 6px;
    right: 8px;
    z-index: 3;
    display: flex;
    gap: 6px;
    padding: 3px;
    border-radius: 7px;
    background: var(--panel);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.15);
  }
  .chrome select {
    font-size: 10.5px;
    max-width: 180px;
  }
  .editor {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr);
    min-height: 0;
  }
  .tracks {
    list-style: none;
    margin: 0;
    padding: 6px 0;
    overflow-y: auto;
    border-right: 1px solid var(--line);
    font-size: 11px;
  }
  .obj {
    padding: 6px 10px 2px;
    font: 600 10px var(--mono);
    color: var(--muted);
    text-transform: uppercase;
  }
  .track {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 1px 10px;
    font-family: var(--mono);
  }
  .track input {
    margin: 0;
  }
  .sw {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .st {
    font-family: var(--mono);
  }
  .track.changed .st {
    color: var(--warn-ink);
  }
  .track.added .st {
    color: var(--add-ink);
  }
  .track.removed {
    text-decoration: line-through;
  }
  .track.removed .st {
    color: var(--bad-ink);
  }
  .flip {
    position: relative;
    height: 20px;
    margin: 4px 0;
    border-top: 1px dashed var(--line);
  }
  .flip.changed {
    border-top-color: var(--warn-ink);
  }
  .frame {
    position: absolute;
    top: 2px;
    font: 9.5px var(--mono);
    padding: 0 4px;
    border-radius: 3px;
    background: var(--hover);
    white-space: nowrap;
  }
  .plot {
    position: relative;
    padding: 36px 44px 8px 10px;
    overflow: auto;
    min-height: 0;
  }
  .canvas {
    position: relative;
  }
  .key {
    position: absolute;
    width: 7px;
    height: 7px;
    margin: -3.5px 0 0 -3.5px;
    border-radius: 50%;
    border: 1px solid var(--panel);
  }
  svg {
    display: block;
    width: 100%;
    height: 260px;
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .grid {
    stroke: var(--line);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  path {
    fill: none;
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }
  path.before {
    stroke-dasharray: 5 4;
    opacity: 0.75;
  }
  .axis {
    display: flex;
    justify-content: space-between;
    font: 9.5px var(--mono);
    color: var(--muted);
  }
  .vals {
    position: absolute;
    top: 36px;
    right: 4px;
    height: 260px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    font: 9.5px var(--mono);
    color: var(--muted);
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
