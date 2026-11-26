<script lang="ts">
  import { onMount } from "svelte";
  import { clearRepoCaches, repoCaches, type RepoCache } from "$lib/api";
  import { HISTORY, cpuLevel, formatBytes, statsFeed, tracePoints } from "$lib/appStats.svelte";
  import { modelCache } from "$lib/model/cache";

  type Repo = { root: string; name: string };
  let {
    repos,
    all = false,
    onopensettings,
  }: { repos: Repo[]; all?: boolean; onopensettings?: () => void } = $props();

  const DISK_CAP = 1024 * 1024 * 1024;
  const TRACE_W = 40;
  const TRACE_H = 12;
  const uid = $props.id();
  const gradId = `cpu-${uid}`;

  let caches = $state<RepoCache[] | null>(null);
  let memBytes = $state(0);
  let cacheError = $state<string | null>(null);
  let open = $state(false);
  let clearing = $state<string | null>(null);
  let freed = $state<number | null>(null);
  let wrap = $state<HTMLElement | null>(null);

  const roots = $derived(repos.map((r) => r.root));
  const diskOf = (c: RepoCache) => c.previewBytes + c.unityBytes;
  const disk = $derived((caches ?? []).reduce((n, c) => n + diskOf(c), 0));
  const total = $derived(disk + memBytes);
  const ranked = $derived([...(caches ?? [])].sort((a, b) => diskOf(b) - diskOf(a)));
  const nameOf = (root: string) => repos.find((r) => r.root === root)?.name ?? root;

  const cpu = $derived(statsFeed.latest?.cpu ?? 0);
  const ram = $derived(statsFeed.latest ? statsFeed.latest.appBytes + statsFeed.latest.viewBytes + statsFeed.latest.gitBytes : 0);
  const ramSplit = $derived(
    statsFeed.latest
      ? `app ${formatBytes(statsFeed.latest.appBytes)} · view ${formatBytes(statsFeed.latest.viewBytes)} · git ${formatBytes(statsFeed.latest.gitBytes)}`
      : "",
  );
  const points = $derived(tracePoints(statsFeed.history, TRACE_W, TRACE_H));
  const firstX = $derived(TRACE_W - (statsFeed.history.length - 1) * (TRACE_W / (HISTORY - 1)));

  async function loadCaches() {
    try {
      caches = await repoCaches(roots);
      memBytes = modelCache.stats().bytes;
      cacheError = null;
    } catch (e) {
      cacheError = String(e);
    }
  }

  $effect(() => {
    void roots;
    void loadCaches();
    const timer = setInterval(() => void loadCaches(), all ? 60_000 : 10_000);
    return () => clearInterval(timer);
  });

  onMount(() => statsFeed.subscribe());

  function toggle() {
    open = !open;
    freed = null;
    if (open) {
      void loadCaches();
    }
  }

  async function clear(which: string[], withMemory: boolean, key: string) {
    const before = (caches ?? []).filter((c) => which.includes(c.root)).reduce((n, c) => n + diskOf(c), 0);
    const mem = withMemory ? memBytes : 0;
    clearing = key;
    freed = null;
    try {
      for (const root of which) {
        await clearRepoCaches(root);
      }
      if (withMemory) {
        modelCache.clear();
      }
      freed = before + mem;
    } catch (e) {
      cacheError = String(e);
    } finally {
      clearing = null;
      await loadCaches();
    }
  }

  function onWindowKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      e.stopPropagation();
      open = false;
    }
  }

  function onWindowPointer(e: PointerEvent) {
    if (open && wrap && !wrap.contains(e.target as Node)) {
      open = false;
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} onpointerdown={onWindowPointer} />

<div class="stats" data-testid="app-stats">
  {#if statsFeed.latest}
    <span class="stat" title="gitgud's CPU over the last 40 s, as a share of the whole machine">
      <span class="k">CPU</span>
      <svg class="trace" width={TRACE_W} height={TRACE_H} viewBox="0 0 {TRACE_W} {TRACE_H}" aria-hidden="true">
        <defs>
          <linearGradient id={gradId} gradientUnits="userSpaceOnUse" x1="0" y1={TRACE_H} x2="0" y2="0">
            <stop offset="0" stop-color="var(--l2)" />
            <stop offset=".5" stop-color="var(--l2)" />
            <stop offset=".5" stop-color="var(--l4)" />
            <stop offset=".86" stop-color="var(--l4)" />
            <stop offset=".86" stop-color="var(--l1)" />
          </linearGradient>
        </defs>
        <path d="M0,{TRACE_H - 0.5}H{TRACE_W}" stroke="var(--line)" />
        {#if statsFeed.history.length > 1}
          <polygon points="{firstX},{TRACE_H} {points} {TRACE_W},{TRACE_H}" fill="url(#{gradId})" opacity=".2" />
          <polyline points={points} fill="none" stroke="url(#{gradId})" stroke-width="1.3" stroke-linejoin="round" />
        {/if}
      </svg>
      <span class="v {cpuLevel(cpu)}">{Math.round(cpu)}%</span>
    </span>
    <span class="stat" title={ramSplit}><span class="k">RAM</span><span class="v">{formatBytes(ram)}</span></span>
  {:else if statsFeed.error}
    <span class="bad" role="alert" title={statsFeed.error}>CPU / RAM unavailable</span>
  {/if}

  <span class="cachewrap" bind:this={wrap}>
    <button
      type="button"
      class="cache"
      class:on={open}
      aria-haspopup="dialog"
      aria-expanded={open}
      title={all ? "gitgud's caches in every repo" : "gitgud's caches for this repo"}
      onclick={toggle}
    >
      <svg class="drum" width="11" height="11" viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5">
        <ellipse cx="8" cy="3.5" rx="5.5" ry="2" />
        <path d="M2.5 3.5v9c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2v-9" />
        <path d="M2.5 8c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2" />
      </svg>
      {#if all}<span class="k">ALL REPOS</span>{/if}
      <span class="v" class:dim={total === 0}>{caches ? formatBytes(total) : "…"}</span>
    </button>

    {#if open}
      <div class="pop" role="dialog" aria-label="gitgud's caches">
        {#if all}
          <h4>gitgud's caches <span class="sub">{repos.length} {repos.length === 1 ? "repo" : "repos"} · {formatBytes(disk)}</span></h4>
          <div class="rows">
            {#each ranked.slice(0, 5) as c (c.root)}
              <span class="nm" title={c.root}>{nameOf(c.root)}</span>
              <b>{formatBytes(diskOf(c))}</b>
              <button
                type="button"
                class="mini"
                disabled={diskOf(c) === 0 || clearing !== null}
                aria-label="Clear {nameOf(c.root)}"
                onclick={() => clear([c.root], false, c.root)}>{clearing === c.root ? "…" : "Clear"}</button
              >
            {/each}
            {#if ranked.length > 5}
              {@const rest = ranked.slice(5)}
              <span class="muted">{rest.length} more</span><b class="muted">{formatBytes(rest.reduce((n, c) => n + diskOf(c), 0))}</b><span></span>
            {/if}
            <span>Loaded in memory</span><b>{formatBytes(memBytes)}</b><span></span>
          </div>
        {:else}
          {@const c = caches?.[0]}
          <h4>gitgud's caches <span class="sub">{repos[0]?.name}</span></h4>
          <div class="rows">
            <span>3D previews on disk</span><b>{formatBytes(c?.previewBytes ?? 0)}</b>
            <span class="n">{(c?.previewModels ?? 0).toLocaleString("en")} models · {(c?.previewTextures ?? 0).toLocaleString("en")} textures</span>
            <span>Unity references</span><b>{formatBytes(c?.unityBytes ?? 0)}</b>
            <span class="n">GUID index{c?.metaFiles ? `, ${c.metaFiles.toLocaleString("en")} .meta files` : ""}</span>
            <span>Loaded in memory</span><b>{formatBytes(memBytes)}</b>
            <span class="n">models and materials kept for the 3D viewers</span>
          </div>
          {@const share = Math.min(1, (c?.previewBytes ?? 0) / DISK_CAP)}
          <div class="cap" aria-hidden="true"><i style:width="{share * 100}%"></i></div>
          <div class="capl"><span>3D previews: {formatBytes(c?.previewBytes ?? 0)} of the 1 GB cap</span><span>{Math.round(share * 100)}%</span></div>
        {/if}
        {#if cacheError}<p class="err" role="alert">{cacheError}</p>{/if}
        <div class="act">
          {#if clearing === "repo" || clearing === "all"}
            <span class="hint">Clearing…</span>
          {:else if freed !== null}
            <span class="hint ok">✓ Freed {formatBytes(freed)}</span>
          {:else}
            <span class="hint">{all ? "Clears gitgud's own caches in every repo." : "Rebuilt when needed; the next big scene opens slower."}</span>
          {/if}
          <button
            type="button"
            class="danger"
            disabled={total === 0 || clearing !== null}
            onclick={() => clear(roots, true, all ? "all" : "repo")}
          >
            {total === 0 ? "Clear" : `${all ? "Clear all" : "Clear"} ${formatBytes(total)}`}
          </button>
        </div>
        <div class="keep">
          Not touched: LFS objects and git's commit&#8209;graph.
          {#if onopensettings}<button type="button" class="link" onclick={() => { open = false; onopensettings(); }}>More in Settings</button>{/if}
        </div>
      </div>
    {/if}
  </span>
</div>

<style>
  .stats {
    display: flex;
    align-items: center;
    gap: 14px;
    flex: none;
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .k {
    font: 600 10px var(--mono);
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .v {
    font: 11.5px var(--mono);
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }
  .v.dim {
    color: var(--muted);
  }
  .v.hot {
    color: var(--l4);
  }
  .v.burn {
    color: var(--l1);
  }
  .trace,
  .drum {
    display: block;
    flex: none;
  }
  .bad {
    color: var(--bad-ink);
  }
  .cachewrap {
    position: relative;
    display: inline-flex;
  }
  .cache {
    all: unset;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 1px 6px;
    margin: 0 -6px;
    border-radius: 4px;
    color: var(--muted);
    cursor: pointer;
  }
  .cache:hover,
  .cache.on {
    background: var(--hover);
  }
  .cache:focus-visible {
    outline: 2px solid var(--l0);
  }
  .pop {
    position: fixed;
    right: 8px;
    bottom: 30px;
    z-index: 50;
    width: 300px;
    display: grid;
    gap: 8px;
    padding: 10px 12px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.3);
    color: var(--fg);
    font: 12px var(--body);
    white-space: normal;
  }
  h4 {
    margin: 0;
    font: 600 12px var(--body);
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .sub {
    font: 11.5px var(--mono);
    font-weight: 400;
    color: var(--muted);
  }
  .rows {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 3px 10px;
    align-items: baseline;
  }
  .pop:has(.mini) .rows {
    grid-template-columns: minmax(0, 1fr) auto auto;
  }
  .rows b {
    font: 600 11.5px var(--mono);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .rows .n {
    grid-column: 1 / -1;
    margin-top: -3px;
    color: var(--muted);
    font-size: 11px;
  }
  .rows .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    color: var(--muted);
  }
  .cap {
    height: 5px;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
  }
  .cap i {
    display: block;
    height: 100%;
    background: var(--l0);
  }
  .capl {
    display: flex;
    justify-content: space-between;
    margin-top: -4px;
    font: 10.5px var(--mono);
    color: var(--muted);
  }
  .act {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .hint {
    flex: 1;
    color: var(--muted);
    font-size: 11px;
  }
  .hint.ok {
    color: var(--l2);
  }
  .err {
    margin: 0;
    color: var(--bad-ink);
    font-size: 11px;
  }
  .keep {
    border-top: 1px solid var(--line);
    padding-top: 7px;
    color: var(--muted);
    font-size: 11px;
  }
  .pop button {
    font: 600 12px var(--body);
    color: var(--fg);
    background: var(--hover);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 3px 10px;
    cursor: pointer;
  }
  .pop button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .pop button.danger {
    color: var(--bad-ink);
  }
  .pop button.mini {
    font-size: 11px;
    padding: 0 7px;
  }
  .pop button.link {
    all: unset;
    color: var(--l0);
    cursor: pointer;
  }
  .pop button.link:hover {
    text-decoration: underline;
  }
</style>
