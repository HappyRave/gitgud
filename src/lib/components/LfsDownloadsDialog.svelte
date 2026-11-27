<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    lfsPartialApply,
    lfsPartialDownload,
    lfsPartialPlan,
    onTaskProgress,
    type LfsPlan,
    type PartialState,
    type Progress,
  } from "$lib/api";
  import { formatBytes } from "$lib/appStats.svelte";
  import { SvelteSet } from "svelte/reactivity";
  import {
    applyEffect,
    byFolder,
    childrenOf,
    sameState,
    segments,
    under,
    withScene,
    withoutKept,
    withoutScene,
    type Segment,
  } from "$lib/lfs/partial";
  import type { SkippedFile } from "$lib/api";
  import Dialog from "./Dialog.svelte";

  let {
    repoId,
    repoName,
    oncancel,
  }: {
    repoId: number;
    repoName: string;
    oncancel: () => void;
  } = $props();

  let plan = $state<LfsPlan | null>(null);
  /** The state being edited; `null` until the first plan arrives. */
  let draft = $state<PartialState | null>(null);
  let planning = $state(true);
  let working = $state<string | null>(null);
  let progress = $state<Progress | null>(null);
  let error = $state<string | null>(null);
  let adding = $state("");
  let seq = 0;

  const effect = $derived(plan ? applyEffect(plan) : null);
  const bar = $derived(plan ? segments(plan, formatBytes) : []);
  /** The bar segment under the pointer, for its hover box. */
  let hovered = $state<{ seg: Segment; at: number } | null>(null);
  const folderFiles = $derived(plan ? byFolder(plan.skippedFiles) : new Map<string, SkippedFile[]>());
  /** Expanded rows of the not-used list, by path. */
  const open = new SvelteSet<string>();
  const toggle = (path: string) => (open.has(path) ? open.delete(path) : open.add(path));
  const changed = $derived(!!plan && !!draft && (!plan.partial || !sameState(plan.state, draft)));
  const sceneChoices = $derived(
    plan && draft ? plan.allScenes.filter((s) => !draft!.scenes.includes(s) && !plan!.buildScenes.includes(s)) : [],
  );

  function replan(state: PartialState | null) {
    const mine = ++seq;
    planning = true;
    error = null;
    lfsPartialPlan(repoId, state).then(
      (p) => {
        if (mine !== seq) {
          return;
        }
        plan = p;
        draft ??= p.state;
        planning = false;
      },
      (e) => {
        if (mine === seq) {
          error = String(e);
          planning = false;
        }
      },
    );
  }
  replan(null);

  function edit(next: PartialState) {
    draft = next;
    replan(next);
  }

  let stop: (() => void) | null = null;
  let destroyed = false;
  onTaskProgress((id, op, p) => {
    if (id === repoId && op === "lfs" && working) {
      progress = p;
    }
  }).then((unlisten) => (destroyed ? unlisten() : (stop = unlisten)));
  onDestroy(() => {
    destroyed = true;
    stop?.();
  });

  async function run(label: string, job: () => Promise<LfsPlan>) {
    working = label;
    progress = null;
    error = null;
    try {
      const p = await job();
      plan = p;
      draft = p.state;
    } catch (e) {
      error = String(e);
    } finally {
      working = null;
    }
  }

  const apply = () => draft && run("Downloading what the scenes use…", () => lfsPartialApply(repoId, draft));
  const whole = () => run("Downloading every LFS file…", () => lfsPartialApply(repoId, null));
  const download = (folder: string) => run(`Downloading ${folder}…`, () => lfsPartialDownload(repoId, [folder]));

  function addScene(event: SubmitEvent) {
    event.preventDefault();
    if (draft && adding.trim()) {
      edit(withScene(draft, adding));
      adding = "";
    }
  }
</script>

{#snippet tree(files: SkippedFile[], base: string, depth: number)}
  {#each childrenOf(files, base) as n (n.path)}
    <div class="li" style:padding-left="{10 + depth * 14}px">
      {#if n.folder}
        <button
          type="button"
          class="caret"
          aria-label="{open.has(n.path) ? 'Collapse' : 'Expand'} {n.path}"
          aria-expanded={open.has(n.path)}
          onclick={() => toggle(n.path)}
        >
          {open.has(n.path) ? "▾" : "▸"}
        </button>
      {:else}
        <span class="cloud" title={n.onDiskBytes ? "On disk now" : "Not downloaded"}>{n.onDiskBytes ? "" : "☁"}</span>
      {/if}
      <span class="nm" class:mono={!n.folder} title={n.path}>{n.name}{n.folder ? "/" : ""}</span>
      <span class="sz">{formatBytes(n.bytes)}{n.folder ? ` · ${n.files.toLocaleString()} files` : ""}</span>
      {#if plan?.partial}
        <button type="button" class="sm" disabled={!!working} aria-label="Download {n.path}" onclick={() => download(n.path)}>Download</button>
      {/if}
    </div>
    {#if n.folder && open.has(n.path)}
      {@render tree(under(files, n.path), n.path, depth + 1)}
    {/if}
  {/each}
{/snippet}

<Dialog title="LFS downloads · {repoName}" width={860} oncancel={() => !working && oncancel()}>
  {#if !plan}
    <p class="muted">{error ?? "Reading the scenes and what they use…"}</p>
  {:else if draft}
    <section class="sum" aria-label="Summary">
      <div class="big">
        <b>{formatBytes(plan.used.bytes + plan.always.bytes + plan.kept.bytes)}</b>
        <span class="muted">
          of {formatBytes(plan.total.bytes)} on disk{changed ? " after Apply" : ""} · {formatBytes(plan.skipped.bytes)} not used by these scenes
        </span>
        {#if planning}<span class="muted">· updating…</span>{/if}
      </div>
      <div class="barwrap">
        <div class="bar">
          {#each bar as seg, i (seg.kind)}
            {#if seg.pct > 0}
              {@const start = bar.slice(0, i).reduce((n, s) => n + s.pct, 0)}
              <i
                class={seg.kind}
                style:width="{seg.pct}%"
                role="img"
                aria-label={seg.label}
                onmouseenter={() => (hovered = { seg, at: Math.min(85, Math.max(15, start + seg.pct / 2)) })}
                onmouseleave={() => (hovered = null)}
              ></i>
            {/if}
          {/each}
        </div>
        {#if hovered}
          <div class="tip" role="tooltip" style:left="{hovered.at}%">
            <b><i class={hovered.seg.kind}></i>{hovered.seg.label}</b>
            <span class="mono">{formatBytes(hovered.seg.bytes)} · {hovered.seg.files.toLocaleString()} files</span>
            <p>{hovered.seg.about}</p>
          </div>
        {/if}
      </div>
      <div class="legend">
        <span><i class="used"></i>used by the scenes · {formatBytes(plan.used.bytes)}</span>
        <span><i class="always"></i>always kept · {formatBytes(plan.always.bytes)}</span>
        {#if plan.kept.files}<span><i class="kept"></i>kept by you · {formatBytes(plan.kept.bytes)}</span>{/if}
        <span>
          <i class="skipped"></i>not used · {formatBytes(plan.skipped.bytes)}{#if plan.skippedOnDisk.bytes}, {formatBytes(plan.skippedOnDisk.bytes)} of it on disk now: Apply removes it{:else}, not downloaded{/if}
        </span>
        <span class="muted">now on disk: {formatBytes(plan.onDisk.bytes)}</span>
      </div>
    </section>

    <div class="cols">
      <div class="col">
        <h3>Scenes and prefabs to keep working</h3>
        <label class="li">
          <input type="checkbox" checked={draft.buildScenes} onchange={(e) => edit({ ...draft!, buildScenes: e.currentTarget.checked })} />
          <span class="nm">Every scene in Build Settings <span class="chip">{plan.buildScenes.length}</span></span>
        </label>
        {#each draft.scenes as scene (scene)}
          <div class="li">
            <span class="dot" aria-hidden="true">✓</span>
            <span class="nm mono" title={scene}>{scene}</span>
            <button type="button" class="x" aria-label="Remove {scene}" onclick={() => edit(withoutScene(draft!, scene))}>×</button>
          </div>
        {/each}
        <form class="add" onsubmit={addScene}>
          <input list="lfs-scenes" bind:value={adding} placeholder="Add a scene or prefab path…" aria-label="Add a scene or prefab" />
          <datalist id="lfs-scenes">
            {#each sceneChoices as s (s)}<option value={s}></option>{/each}
          </datalist>
          <button type="submit" disabled={!adding.trim()}>Add</button>
        </form>
        {#if draft.kept.length}
          <h3>Kept by you</h3>
          {#each draft.kept as path (path)}
            <div class="li">
              <span class="dot" aria-hidden="true">✓</span>
              <span class="nm mono" title={path}>{path}</span>
              <button type="button" class="x" aria-label="Stop keeping {path}" onclick={() => edit(withoutKept(draft!, path))}>×</button>
            </div>
          {/each}
        {/if}
        <h3>Always kept</h3>
        <div class="li">
          <span class="dot" aria-hidden="true">✓</span>
          <span class="nm" title="Code, DLLs, Resources, Editor, StreamingAssets, and everything outside Assets/">Code, DLLs, Resources, Editor, StreamingAssets, outside Assets/</span>
          <span class="sz">{formatBytes(plan.always.bytes)}</span>
        </div>
        {#if plan.missing.length}
          <p class="warn">Not in the repo: {plan.missing.join(", ")}</p>
        {/if}
      </div>

      <div class="col">
        <h3>Not used by these scenes, by folder · {plan.skipped.files.toLocaleString()} files</h3>
        <div class="folders">
          {#each plan.skippedFolders as f (f.folder)}
            {@const files = folderFiles.get(f.folder) ?? []}
            <div class="li">
              <button type="button" class="caret" aria-label="{open.has(f.folder) ? 'Collapse' : 'Expand'} {f.folder}" aria-expanded={open.has(f.folder)} onclick={() => toggle(f.folder)}>
                {open.has(f.folder) ? "▾" : "▸"}
              </button>
              <span class="nm" title={f.folder}>{f.folder}</span>
              <span class="sz" title="Not used, of every LFS file in the folder">{formatBytes(f.skipped.bytes)} of {formatBytes(f.total.bytes)}</span>
              {#if plan.partial}
                <button type="button" class="sm" disabled={!!working} onclick={() => download(f.folder)}>Download</button>
              {/if}
            </div>
            {#if open.has(f.folder)}
              {@render tree(files, f.folder, 1)}
            {/if}
          {:else}
            <p class="muted">Everything is used.</p>
          {/each}
        </div>
        {#if !plan.partial}
          <p class="muted hint">Apply first; then any folder here can be downloaded and kept.</p>
        {/if}
      </div>
    </div>

    {#if working}
      <p class="status" role="status">{working}{progress ? ` ${progress.phase} ${progress.percent}% (${progress.done}/${progress.total})` : ""}</p>
    {/if}
    {#if error}
      <p class="err" role="alert">{error}</p>
    {/if}
  {/if}

  {#snippet footer()}
    <span class="muted foot">Saved as <code>lfs.fetchinclude</code> in .git/config; Sourcetree and the terminal follow it.</span>
    <span class="sp"></span>
    {#if plan?.partial}
      <button type="button" disabled={!!working} onclick={whole}>Download everything</button>
    {/if}
    <button type="button" disabled={!!working} onclick={oncancel}>Close</button>
    <button type="button" class="pri" disabled={!plan || !changed || planning || !!working} onclick={apply}>
      {#if !changed}Apply{:else if effect?.kind === "free"}Apply · free {formatBytes(effect.bytes)}{:else if effect?.kind === "download"}Apply · download {formatBytes(effect.bytes)}{:else}Apply{/if}
    </button>
  {/snippet}
</Dialog>

<style>
  .muted {
    color: var(--muted);
  }
  .sum {
    display: grid;
    gap: 6px;
  }
  .big b {
    font-size: 18px;
  }
  .bar {
    height: 8px;
    border-radius: 4px;
    background: var(--hover);
    overflow: hidden;
    display: flex;
  }
  .bar i,
  .legend i {
    display: block;
    height: 100%;
  }
  .used {
    background: var(--l0);
  }
  .always {
    background: var(--l2);
  }
  .kept {
    background: var(--l4);
  }
  .bar .skipped {
    background: transparent;
  }
  .bar i:hover {
    filter: brightness(1.25);
  }
  .bar .skipped:hover {
    background: var(--line);
  }
  .barwrap {
    position: relative;
  }
  .tip {
    position: absolute;
    top: 14px;
    z-index: 5;
    transform: translateX(-50%);
    width: 280px;
    display: grid;
    gap: 2px;
    padding: 7px 9px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--panel);
    box-shadow: 0 6px 20px #0006;
    font-size: 11.5px;
    pointer-events: none;
  }
  .tip b i {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    margin-right: 5px;
  }
  .tip .mono {
    color: var(--muted);
  }
  .tip p {
    margin: 2px 0 0;
    color: var(--muted);
  }
  .caret {
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .caret:hover {
    color: var(--fg);
  }
  .tip .skipped {
    background: var(--hover);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 11.5px;
  }
  .legend i {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    margin-right: 4px;
    background: var(--hover);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .col {
    min-width: 0;
    padding-bottom: 6px;
  }
  .col + .col {
    border-left: 1px solid var(--line);
  }
  h3 {
    margin: 0;
    padding: 8px 10px 3px;
    font: 600 10px var(--mono);
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .li {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto auto;
    gap: 6px;
    align-items: center;
    padding: 2px 10px;
  }
  .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mono {
    font: 11.5px var(--mono);
  }
  .sz {
    font: 11px var(--mono);
    color: var(--muted);
    white-space: nowrap;
  }
  .dot {
    color: var(--muted);
    text-align: center;
  }
  .cloud {
    color: var(--l0);
    text-align: center;
  }
  .chip {
    font: 600 10px var(--mono);
    padding: 0 5px;
    border-radius: 8px;
    background: var(--hover);
    color: var(--muted);
  }
  .x {
    padding: 0 6px;
  }
  .sm {
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
  }
  .folders {
    max-height: 300px;
    overflow: auto;
  }
  .add {
    display: flex;
    gap: 6px;
    padding: 4px 10px;
  }
  .add input {
    flex: 1;
    min-width: 0;
  }
  .hint {
    padding: 4px 10px;
    margin: 0;
    font-size: 11.5px;
  }
  .warn {
    margin: 6px 10px 0;
    color: var(--warn-ink);
    font-size: 11.5px;
  }
  .status,
  .err {
    margin: 0;
    font-size: 12px;
  }
  .err {
    color: var(--bad-ink);
  }
  .foot {
    font-size: 11px;
  }
  .sp {
    flex: 1;
  }
</style>
