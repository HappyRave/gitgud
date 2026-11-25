<script lang="ts">
  import { onDestroy, type Snippet } from "svelte";
  import type { Object3D } from "three";
  import type { BinaryMeta, BinarySide } from "$lib/api";
  import { type Change, type ModelStats, stats, TINT } from "$lib/model/compare";
  import type { ModelSide, Opened } from "$lib/model/open";
  import type { Summary } from "$lib/model/progress";
  import type { CompareMode, ModelStage } from "$lib/model/stage";
  import { prefs, setModelPlaying, setModelShading, setShaderBase, shaderBase } from "$lib/prefs.svelte";
  import { type BaseSlots, mergeBases } from "$lib/model/approximations";
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
    opener,
    chrome,
    startClip = null,
    merged = false,
  }: {
    repoId: number;
    path: string;
    before?: ModelSide | null;
    after: ModelSide;
    bust?: number;
    meta?: BinaryMeta | null;
    onfetch?: (side: "before" | "after", meta: BinarySide) => Promise<void>;
    /** Builds each side's model instead of loading `path` (the material viewer's ball). */
    opener?: (side: ModelSide) => Promise<Opened>;
    /** Replaces the view's own controls; shading stays Textured and the counts are hidden. */
    chrome?: Snippet;
    /** The clip selected once the model loads, when it has one by that name. */
    startClip?: string | null;
    /** Offers the Merged mode next to Side by side (scenes, R7 decision 10). */
    merged?: boolean;
  } = $props();

  let canvas = $state<HTMLCanvasElement>();
  let viewport = $state<HTMLDivElement>();
  let stage: ModelStage | null = null;
  let loading = $state(true);
  /** Shown, with meshes still arriving. */
  let filling = $state(false);
  let status = $state<Summary>({ label: "Loading model…", value: null });
  let errors = $state<{ before: string | null; after: string | null }>({ before: null, after: null });
  let lfsMissing = $state(false);
  let single = $state(true);
  let changes = $state<Change[]>([]);
  let counts = $state<{ before: ModelStats | null; after: ModelStats | null }>({ before: null, after: null });
  let notes = $state<string[]>([]);
  /** Custom shaders shown as standard lit, with the textures a base map can come from. */
  let approx = $state<string[]>([]);
  let bases = $state<Record<string, BaseSlots>>({});
  let notesOpen = $state(false);
  let clips = $state<{ name: string; duration: number }[]>([]);
  let clip = $state<string | null>(null);
  let time = $state(0);
  let mode = $state<CompareMode>("side");
  let wire = $state(false);
  let split = $state(0.5);
  let onion = $state(0.45);
  let expanded = $state(false);
  let changedOnly = $state(false);
  let filter = $state("");
  let seq = 0;

  const BIG_MESH = 1_000_000;
  const duration = $derived(clips.find((c) => c.name === clip)?.duration ?? 0);
  const big = $derived((counts.after?.triangles ?? 0) > BIG_MESH);
  const fmt = (n: number) => n.toLocaleString("en-US");

  function delta(a: number | undefined, b: number): string {
    if (a === undefined || a === b) {
      return "";
    }
    return b > a ? `+${fmt(b - a)}` : `−${fmt(a - b)}`;
  }

  async function load(token: number) {
    // Read now: props are live, and after an await they may already be the next file's.
    const at = { repoId, path, before, after, bust, opener };
    const [{ openSide, needsLfsFetch }, { LoadProgress }] = await Promise.all([
      import("$lib/model/open"),
      import("$lib/model/progress"),
    ]);
    const progress = new LoadProgress();
    progress.onchange = (s) => {
      if (token === seq) {
        status = s;
      }
    };
    const piece = (o: Object3D) => {
      if (token === seq) {
        stage?.added(o);
      }
    };
    // The stage gets ready while the sides' structure is read: both are waited on before anything shows.
    const staged = stage ? Promise.resolve() : import("$lib/model/stage");
    const [b, a]: [Opened | null, Opened] = await Promise.all([
      at.before
        ? at.opener
          ? at.opener(at.before)
          : openSide(at.repoId, at.path, at.before, at.bust, progress, piece)
        : Promise.resolve(null),
      at.opener ? at.opener(at.after) : openSide(at.repoId, at.path, at.after, at.bust, progress, piece),
    ]);
    const stageModule = await staged;
    if (token !== seq) {
      return;
    }
    const absent = (o: Opened | null) => !!o?.error && /not in that version/.test(o.error);
    errors = { before: absent(b) ? null : (b?.error ?? null), after: a.error };
    lfsMissing = needsLfsFetch(errors.before) || needsLfsFetch(errors.after);
    if (!a.model || !canvas) {
      loading = false;
      return;
    }
    if (!stage && stageModule) {
      stage = new stageModule.ModelStage(canvas);
      stage.ontime = (t) => (time = t);
      resize();
    }
    if (!stage) {
      return;
    }
    stage.setModels(b?.model ?? null, a.model);
    single = stage.single;
    // A scene shows at once and fills in; what depends on all of it waits until then.
    changes = [];
    counts = { before: null, after: null };
    notes = [];
    approx = [];
    bases = {};
    clips = [];
    clip = null;
    loading = false;
    const growing = !!(b?.load || a.load);
    filling = growing;
    sync();
    try {
      if (growing) {
        // Let the placeholders paint before hundreds of requests take the main thread.
        await new Promise((resolve) => requestAnimationFrame(() => setTimeout(resolve)));
        if (token !== seq) {
          return;
        }
        await Promise.all([b?.load?.(), a.load?.()]);
      }
    } catch (e) {
      if (token === seq) {
        errors = { ...errors, after: e instanceof Error ? e.message : String(e) };
        filling = false;
      }
      return;
    }
    if (token !== seq) {
      return;
    }
    if (growing) {
      stage.refresh();
      // It was framed on the placeholders, which only mark where meshes sit, not their size.
      if (!stage.cameraMoved) {
        stage.frameAll();
      }
    }
    filling = false;
    changes = stage.comparison.changes;
    counts = { before: b?.model ? stats(b.model.root) : null, after: stats(a.model.root) };
    approx = [...new Set([...(b?.model?.approximated ?? []), ...a.model.approximated])];
    bases = mergeBases(b?.model?.bases, a.model.bases);
    const found = [...a.model.missing];
    const disk = Math.max(a.model.fromDisk ?? 0, b?.model?.fromDisk ?? 0);
    if (disk > 0) {
      found.push(`${fmt(disk)} gitignored ${disk === 1 ? "file comes" : "files come"} from your disk copy, the same in both versions`);
    }
    if (counts.after && counts.after.triangles > BIG_MESH) {
      found.push(`Large model: ${fmt(counts.after.triangles)} triangles, so orbiting may be slow. Wire is off-limits above this size.`);
    }
    notes = [...new Set(found)];
    clips = stage.clips().map((c) => ({ name: c.name, duration: c.duration }));
    clip = stage.clip;
    if (startClip && startClip !== clip && clips.some((c) => c.name === startClip)) {
      stage.setClip(startClip);
      clip = startClip;
    }
    time = 0;
    sync();
    loading = false;
  }

  const inputs = $derived(JSON.stringify([repoId, path, before, after, bust]));

  function reload() {
    loading = true;
    filling = false;
    status = { label: "Loading model…", value: null };
    void load(++seq);
  }

  $effect(() => {
    void inputs;
    reload();
  });

  /** The base map shown for `shader`: the one picked for it, else the automatic one. */
  function baseOf(shader: string): string {
    const b = bases[shader];
    const picked = shaderBase(shader);
    return picked && b.slots.includes(picked) ? picked : (b.auto ?? b.slots[0]);
  }

  function pickBase(shader: string, property: string) {
    setShaderBase(shader, property);
    reload();
  }

  function sync() {
    if (!stage) {
      return;
    }
    stage.mode = single ? "side" : mode;
    stage.wire = wire && !big;
    stage.split = split;
    stage.playing = prefs.modelPlaying && duration > 0;
    stage.setOnion(onion);
    stage.setDimUnchanged(changedOnly && !single && !chrome && prefs.modelShading === "changes");
    stage.setShading(single || chrome ? "textured" : prefs.modelShading);
    stage.invalidate();
  }

  $effect(() => {
    void [mode, wire, big, split, onion, prefs.modelShading, prefs.modelPlaying, single, duration, changedOnly];
    sync();
  });

  function resize() {
    if (stage && viewport) {
      stage.resize(viewport.clientWidth, viewport.clientHeight);
    }
  }

  $effect(() => {
    if (!viewport) {
      return;
    }
    const ro = new ResizeObserver(resize);
    ro.observe(viewport);
    const io = new IntersectionObserver(([e]) => stage?.setActive(e.isIntersecting));
    io.observe(viewport);
    return () => {
      ro.disconnect();
      io.disconnect();
    };
  });

  onDestroy(() => {
    seq++;
    stage?.dispose();
    stage = null;
  });

  function pickClip(name: string) {
    clip = name;
    stage?.setClip(name);
    time = 0;
  }

  function scrub(e: Event) {
    setModelPlaying(false);
    time = (+(e.currentTarget as HTMLInputElement).value / 1000) * duration;
    stage?.seek(time);
  }

  function onkey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input, select")) {
      return;
    }
    if (e.key === "f" || e.key === "F") {
      e.preventDefault();
      stage?.frameAll();
    } else if (e.key === " " && duration > 0) {
      e.preventDefault();
      setModelPlaying(!prefs.modelPlaying);
    } else if (e.key === "Escape" && expanded) {
      e.preventDefault();
      expanded = false;
    }
  }

  function dragSwipe(e: PointerEvent) {
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      if (!viewport) {
        return;
      }
      const r = viewport.getBoundingClientRect();
      split = Math.min(0.97, Math.max(0.03, (ev.clientX - r.left) / r.width));
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", () => handle.removeEventListener("pointermove", move), { once: true });
  }

  function dblclick(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    stage?.pick(e.clientX - r.left, e.clientY - r.top);
  }

  const shown = $derived(single || chrome ? "textured" : prefs.modelShading);
  /** The change list gets a filter box past this many entries (a scene's hierarchy). */
  const FILTER_FROM = 8;
  const filtered = $derived(filter.trim() ? changes.filter((c) => c.name.toLowerCase().includes(filter.trim().toLowerCase())) : changes);
  const sym = { added: "+", removed: "−", modified: "~" } as const;
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="model" class:expanded role="application" aria-label="3D preview of {path}" tabindex="0" onkeydown={onkey}>
  <div class="bar">
    <div class="seg" role="radiogroup" aria-label="Compare mode">
      {#each [["side", "Side by side"], ["swipe", "Swipe"], ["onion", "Onion"], ...(merged ? [["merged", "Merged"]] : [])] as [m, label] (m)}
        <button
          type="button"
          class="tbtn"
          class:on={!single && mode === m}
          role="radio"
          aria-checked={!single && mode === m}
          disabled={single}
          onclick={() => (mode = m as CompareMode)}>{label}</button
        >
      {/each}
    </div>
    {#if !single && mode === "onion"}
      <label class="onion">
        {before?.label} ghost
        <input type="range" min="0.1" max="0.9" step="0.05" bind:value={onion} aria-label="Ghost opacity" />
      </label>
    {/if}
    <button type="button" class="tbtn solo" onclick={() => (expanded = !expanded)} aria-pressed={expanded}
      >{expanded ? "Close" : "Expand"}</button
    >
  </div>

  <div class="vp" bind:this={viewport}>
    <canvas bind:this={canvas} ondblclick={dblclick} aria-label="3D view; drag to orbit, right-drag to pan, wheel to zoom, F to frame"
    ></canvas>

    {#if !loading && !errors.after}
      {#if !single && mode === "side"}<div class="divider"></div>{/if}
      {#if !single && mode === "swipe"}
        <div class="swipe" style:left="{split * 100}%">
          <button type="button" class="handle" aria-label="Drag to swipe" onpointerdown={dragSwipe}>⇔</button>
        </div>
      {/if}
      {#if !single}
        <span class="lbl" class:ghost={mode === "onion"} style:left="8px">{mode === "onion" ? `ghost: ${before?.label}` : before?.label}</span>
        <span
          class="lbl"
          style:left={mode === "side" ? "calc(50% + 8px)" : mode === "onion" ? "8px" : undefined}
          style:right={mode === "swipe" ? "8px" : undefined}
          style:top={mode === "onion" ? "32px" : undefined}>{after.label}</span
        >
      {:else}
        <span class="lbl" style:left="8px">{after.label}</span>
      {/if}

      {#if shown === "changes" && changes.length > 0}
        <div class="legend" aria-label="Changes">
          {#if changes.length > FILTER_FROM}
            <input class="filter" type="search" placeholder="Filter {changes.length} changes" aria-label="Filter changes" bind:value={filter} />
          {/if}
          {#each filtered as c, i (i)}
            <button type="button" title="Fly to {c.name}" onclick={() => stage?.frame(c.obj)}>
              <span class="dot" style:background={TINT[c.kind]}></span><b>{sym[c.kind]}</b>{c.name}<span class="why">{c.note}</span>
            </button>
          {/each}
        </div>
      {/if}

      <div class="chrome">
        {#if chrome}
          {@render chrome()}
        {:else}
        <div class="seg" role="radiogroup" aria-label="Shading">
          <button
            type="button"
            class="tbtn"
            class:on={shown === "textured"}
            role="radio"
            aria-checked={shown === "textured"}
            onclick={() => setModelShading("textured")}>Textured</button
          >
          <button
            type="button"
            class="tbtn"
            class:on={shown === "changes"}
            role="radio"
            aria-checked={shown === "changes"}
            disabled={single}
            onclick={() => setModelShading("changes")}>Changes</button
          >
        </div>
        <button
          type="button"
          class="tbtn solo"
          class:on={wire && !big}
          aria-pressed={wire && !big}
          disabled={big}
          title={big ? "Wireframe is off for models this large" : undefined}
          onclick={() => (wire = !wire)}>Wire</button
        >
        {#if shown === "changes"}
          <button
            type="button"
            class="tbtn solo"
            class:on={changedOnly}
            aria-pressed={changedOnly}
            title="Fade everything that didn't change"
            onclick={() => (changedOnly = !changedOnly)}>Changed only</button
          >
        {/if}
        {/if}
      </div>

      {#if counts.after && !chrome}
        <div class="stats">
          <b>{fmt(counts.after.triangles)}</b> tris
          {#if counts.before}<span class="d">{delta(counts.before.triangles, counts.after.triangles)}</span>{/if}
          · {fmt(counts.after.vertices)} verts · {counts.after.meshes} meshes · {counts.after.materials} materials · {counts.after.textures}
          textures
        </div>
      {/if}

      {#if notes.length > 0 || approx.length > 0 || errors.before}
        {@const count = notes.length + approx.length + (errors.before ? 1 : 0)}
        <button
          type="button"
          class="warn-btn"
          class:on={notesOpen}
          aria-expanded={notesOpen}
          aria-label={`${count} ${count === 1 ? "warning" : "warnings"}`}
          title={notesOpen ? "Hide warnings" : "Show warnings"}
          onclick={() => (notesOpen = !notesOpen)}
          ><svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"
            ><path d="M8 1.5 15 14H1z" fill="currentColor" /><path d="M8 6v4M8 11.6v.4" stroke="#1d1606" stroke-width="1.6" stroke-linecap="round" /></svg
          >{count}</button
        >
        {#if notesOpen}
          <div class="notes" role="note">
            {#if errors.before}<span>{before?.label}: {errors.before}</span>{/if}
            {#each approx as s (s)}
              <span
                >{s} is shown as a standard lit material{#if (bases[s]?.slots.length ?? 0) > 1}
                  <label class="base"
                    >· base map <select value={baseOf(s)} onchange={(e) => pickBase(s, e.currentTarget.value)}
                      >{#each bases[s].slots as p (p)}<option value={p}>{p}</option>{/each}</select
                    ></label
                  >{/if}</span
              >
            {/each}
            {#each notes as n (n)}<span>{n}</span>{/each}
          </div>
        {/if}
      {/if}

      {#if clips.length > 0}
        <div class="transport">
          <button
            type="button"
            class="play"
            aria-label={prefs.modelPlaying ? "Pause" : "Play"}
            onclick={() => setModelPlaying(!prefs.modelPlaying)}>{prefs.modelPlaying ? "❚❚" : "▶"}</button
          >
          <select aria-label="Clip" value={clip} onchange={(e) => pickClip(e.currentTarget.value)}>
            {#each clips as c, i (i)}<option value={c.name}>{c.name} · {c.duration.toFixed(1)} s</option>{/each}
          </select>
          <input
            type="range"
            min="0"
            max="1000"
            value={duration ? Math.round((time / duration) * 1000) : 0}
            aria-label="Animation time"
            oninput={scrub}
          />
          <span class="time">{time.toFixed(2)} / {duration.toFixed(2)} s</span>
        </div>
      {/if}
    {/if}

    {#if filling && !errors.after}
      <p class="filling" aria-live="polite"><LoadingBar label={status.label} value={status.value} /></p>
    {/if}
    {#if loading}
      <p class="state loading"><LoadingBar label={status.label} value={status.value} /></p>
    {:else if lfsMissing}
      <div class="state card"><LfsBinaryCard {path} {meta} {onfetch} /></div>
    {:else if errors.after}
      <p class="state err" role="alert">{errors.after}</p>
    {/if}
  </div>
</div>

<style>
  .model {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 420px;
    height: 100%;
    outline: none;
    background: var(--panel);
  }
  .model.expanded {
    position: fixed;
    inset: 0;
    z-index: 50;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
  }
  .bar .solo {
    margin-left: auto;
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
  .play:focus-visible {
    outline: 2px solid var(--l0);
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
  .vp {
    position: relative;
    min-height: 0;
    overflow: hidden;
    background: radial-gradient(120% 95% at 50% 30%, var(--panel), var(--bg));
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    touch-action: none;
  }
  .lbl {
    position: absolute;
    top: 8px;
    font: 10.5px var(--mono);
    padding: 2px 7px;
    border-radius: 4px;
    background: rgb(10 14 20 / 0.55);
    color: #f2f4f7;
    pointer-events: none;
    white-space: nowrap;
  }
  .lbl.ghost {
    background: var(--l1);
    color: #fff;
  }
  .divider {
    position: absolute;
    inset: 0 auto 0 50%;
    width: 1px;
    background: var(--line);
    pointer-events: none;
  }
  .swipe {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 0;
  }
  .swipe::before {
    content: "";
    position: absolute;
    inset: 0 auto 0 -1px;
    width: 2px;
    background: #fff;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.25);
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
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.35);
    touch-action: none;
  }
  .chrome {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    gap: 6px;
    padding: 4px;
    border-radius: 8px;
    background: rgb(16 19 24 / 0.55);
    backdrop-filter: blur(6px);
  }
  .chrome .seg,
  .chrome .tbtn.solo {
    border-color: rgb(255 255 255 / 0.16);
  }
  .chrome .tbtn {
    color: #c9d0da;
    border-color: rgb(255 255 255 / 0.16);
  }
  .chrome .tbtn.on {
    background: rgb(255 255 255 / 0.18);
    color: #fff;
  }
  .chrome .tbtn:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.1);
    color: #fff;
  }
  .legend {
    position: absolute;
    top: 34px;
    left: 10px;
    display: grid;
    gap: 4px;
    justify-items: start;
    max-width: calc(50% - 20px);
    max-height: calc(100% - 80px);
    overflow-y: auto;
  }
  .legend .filter {
    width: 180px;
    font-size: 10.5px;
    padding: 2px 6px;
    border: 1px solid var(--line);
    border-radius: 5px;
    background: color-mix(in srgb, var(--panel) 92%, transparent);
    color: var(--fg);
  }
  .legend button {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    padding: 2px 9px 2px 7px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: color-mix(in srgb, var(--panel) 88%, transparent);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .legend button:hover,
  .legend button:focus-visible {
    border-color: var(--muted);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
  }
  .why {
    color: var(--muted);
    font: 10px var(--mono);
  }
  .stats {
    position: absolute;
    left: 10px;
    bottom: 46px;
    max-width: calc(100% - 96px);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font: 10.5px var(--mono);
    color: #dfe4ea;
    background: rgb(10 12 16 / 0.5);
    padding: 3px 8px;
    border-radius: 4px;
  }
  .stats .d {
    color: #ffc533;
  }
  .warn-btn {
    all: unset;
    position: absolute;
    right: 10px;
    bottom: 46px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 7px 2px 5px;
    border-radius: 999px;
    font: 600 10.5px var(--mono);
    color: #ffb020;
    background: rgb(10 12 16 / 0.55);
    cursor: pointer;
  }
  .warn-btn:hover,
  .warn-btn:focus-visible,
  .warn-btn.on {
    background: rgb(10 12 16 / 0.8);
    outline: 1px solid #ffb020;
  }
  .notes {
    position: absolute;
    right: 10px;
    bottom: 72px;
    max-width: min(460px, calc(100% - 20px));
    max-height: calc(100% - 130px);
    overflow-y: auto;
    display: grid;
    align-content: start;
    gap: 2px;
    padding: 6px 10px;
    border-radius: 6px;
    background: var(--warn);
    color: var(--warn-ink);
    font-size: 11px;
  }
  .notes .base {
    margin-left: 4px;
    white-space: nowrap;
  }
  .notes select {
    margin-left: 3px;
    padding: 0 2px;
    font: 10.5px var(--mono);
    color: inherit;
    background: rgb(255 255 255 / 0.35);
    border: 1px solid currentColor;
    border-radius: 3px;
  }
  .transport {
    position: absolute;
    inset: auto 0 0 0;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 18px 12px 9px;
    font: 10.5px var(--mono);
    color: #e8ecf2;
    background: linear-gradient(transparent, rgb(8 10 14 / 0.7));
  }
  .play {
    all: unset;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 9px;
    background: #fff;
    color: #1b1f25;
    cursor: pointer;
    flex: none;
  }
  .transport select {
    max-width: 240px;
    font: 10.5px var(--mono);
    color: #e8ecf2;
    background: rgb(255 255 255 / 0.1);
    border: 1px solid rgb(255 255 255 / 0.2);
    border-radius: 4px;
  }
  .transport select option {
    color: #111;
  }
  .transport input {
    flex: 1;
    min-width: 60px;
  }
  .time {
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .state {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .filling {
    position: absolute;
    left: 10px;
    bottom: 46px;
    width: min(320px, calc(100% - 96px));
    margin: 0;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 11px;
    color: var(--muted);
    background: color-mix(in srgb, var(--panel) 80%, transparent);
    pointer-events: none;
  }
  .state.loading {
    background: color-mix(in srgb, var(--panel) 70%, transparent);
  }
  .state.err {
    color: var(--bad-ink);
    padding: 20px;
    text-align: center;
  }
  .state.card {
    background: var(--panel);
  }
</style>
