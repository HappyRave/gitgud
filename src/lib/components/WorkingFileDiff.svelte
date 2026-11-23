<script lang="ts">
  import { onMount } from "svelte";
  import {
    lfsFetchObject,
    worktreeFileDiff,
    worktreeUnityDiff,
    type BinarySide,
    type FileDiff,
    type Hunk,
    type LineSelect,
    type Side,
    type UnityYamlDiff,
  } from "$lib/api";
  import { imageUrl, isImage } from "$lib/images";
  import { prefs, setDiffLayout, toggleIgnoreWhitespace } from "$lib/prefs.svelte";
  import { isUnityYaml } from "$lib/unity";
  import DiffView, { type LineAction } from "./DiffView.svelte";
  import LoadingBar from "./LoadingBar.svelte";
  import UnityDiffView from "./UnityDiffView.svelte";

  let {
    repoId,
    path,
    side,
    revision,
    perHunk,
    busy = false,
    onhunk,
    onlines,
    onhistory,
    onblame,
  }: {
    repoId: number;
    path: string;
    side: Side;
    /** Changes whenever the status was re-read, so the diff is too. */
    revision: number;
    /** Whether hunks can be staged one by one (not for new, deleted or binary files). */
    perHunk: boolean;
    busy?: boolean;
    onhunk: (action: "stage" | "unstage" | "discard", hunk: Hunk) => void;
    onlines?: (action: "stage" | "unstage" | "discard", select: LineSelect) => void;
    onhistory?: (path: string) => void;
    onblame?: (path: string) => void;
  } = $props();

  let container = $state<HTMLElement | null>(null);
  let width = $state(0);

  const effectiveSplit = $derived(prefs.diffLayout === "auto" ? width >= 1400 : prefs.split);

  const actions = $derived(
    !perHunk
      ? []
      : side === "staged"
        ? [{ label: "Unstage hunk", key: "U", run: (h: Hunk) => onhunk("unstage", h) }]
        : [
            { label: "Discard hunk", key: "⌫", run: (h: Hunk) => onhunk("discard", h) },
            { label: "Stage hunk", key: "S", run: (h: Hunk) => onhunk("stage", h) },
          ],
  );

  const pluralLines = (verb: string) => (n: number) => `${verb} ${n} ${n === 1 ? "line" : "lines"}`;

  const lineActions = $derived<LineAction[]>(
    !perHunk || !onlines
      ? []
      : side === "staged"
        ? [{ label: pluralLines("Unstage"), run: (s: LineSelect) => onlines("unstage", s) }]
        : [
            { label: pluralLines("Discard"), run: (s: LineSelect) => onlines("discard", s) },
            { label: pluralLines("Stage"), run: (s: LineSelect) => onlines("stage", s) },
          ],
  );

  let fetchRev = $state(0);

  const images = $derived(
    !isImage(path)
      ? null
      : side === "staged"
        ? {
            before: { label: "HEAD", url: imageUrl(repoId, path, { at: "head" }, revision + fetchRev) },
            after: { label: "Staged", url: imageUrl(repoId, path, { at: "index" }, revision + fetchRev) },
          }
        : {
            before: { label: "Index", url: imageUrl(repoId, path, { at: "index" }, revision + fetchRev) },
            after: { label: "On disk", url: imageUrl(repoId, path, { at: "worktree" }, revision + fetchRev) },
          },
  );

  async function handleFetchLfs(_which: "before" | "after", meta: BinarySide) {
    if (!meta.oid) {
      return;
    }
    const currentRepo = repoId;
    const currentPath = path;
    const currentSide = side;
    const ws = prefs.ignoreWhitespace;
    await lfsFetchObject(currentRepo, meta.oid, meta.size, currentPath);
    if (repoId !== currentRepo || path !== currentPath || side !== currentSide) {
      return;
    }
    fetchRev += 1;
    diff = await worktreeFileDiff(currentRepo, currentPath, currentSide, ws);
  }

  let diff = $state<FileDiff | null>(null);
  let unityDiff = $state<UnityYamlDiff | null>(null);
  let preferStructural = $state(true);
  let error = $state<string | null>(null);
  /** The hunk Alt+↑↓ moved to; S / U / ⌫ act on it. */
  let focused = $state(0);
  const isUnityFile = $derived(isUnityYaml(path));
  // A refresh of the same view keeps the old diff up; a different view clears it so it shows loading.
  let shownView = "";

  onMount(() => {
    if (!container || typeof ResizeObserver === "undefined") {
      return;
    }
    width = container.clientWidth;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        width = entry.contentRect.width;
      }
    });
    ro.observe(container);
    return () => ro.disconnect();
  });

  $effect(() => {
    const ws = prefs.ignoreWhitespace;
    const structural = isUnityFile && preferStructural;
    const key = `${repoId}:${path}:${side}:${revision}:${ws}:${structural}`;
    const stale = () =>
      key !== `${repoId}:${path}:${side}:${revision}:${prefs.ignoreWhitespace}:${isUnityYaml(path) && preferStructural}`;
    error = null;
    const view = `${repoId}:${path}:${side}:${ws}:${structural}`;
    if (view !== shownView) {
      shownView = view;
      diff = null;
      unityDiff = null;
    }
    const loadRaw = () => {
      unityDiff = null;
      worktreeFileDiff(repoId, path, side, ws).then(
        (d) => {
          if (!stale()) {
            diff = d;
            focused = Math.min(focused, Math.max(0, d.hunks.length - 1));
          }
        },
        (err) => {
          if (!stale()) {
            error = String(err);
          }
        },
      );
    };
    if (structural) {
      worktreeUnityDiff(repoId, path, side).then(
        (u) => {
          if (!stale()) {
            unityDiff = u;
            diff = null;
          }
        },
        () => {
          if (!stale()) {
            loadRaw();
          }
        },
      );
    } else {
      loadRaw();
    }
  });

  $effect(() => {
    void repoId;
    void path;
    void side;
    focused = 0;
    fetchRev = 0;
  });

  /** Runs a hunk key; false when it doesn't apply here (wrong side, no hunks). */
  export function hunkKey(action: "next" | "prev" | "stage" | "unstage" | "discard"): boolean {
    const hunks = diff?.hunks ?? [];
    if (!perHunk || (isUnityFile && preferStructural && unityDiff) || hunks.length === 0) {
      return false;
    }
    if (action === "next" || action === "prev") {
      focused = Math.max(0, Math.min(hunks.length - 1, focused + (action === "next" ? 1 : -1)));
      return true;
    }
    const allowed = side === "staged" ? action === "unstage" : action !== "unstage";
    if (!allowed || busy) {
      return false;
    }
    onhunk(action, hunks[focused]);
    return true;
  }
</script>

<section
  bind:this={container}
  class="working-diff"
  aria-label="{side === 'staged' ? 'Staged' : 'Unstaged'} changes in {path}"
>
  <h3 class="dh">
    <span class="name">{path}</span> <span class="side">{side === "staged" ? "staged" : "unstaged"}</span>
    <span class="keys">
      {#if perHunk && (diff?.hunks.length ?? 0) > 1}Alt+↑↓ hunks ·{/if}
      {#if isUnityFile}
        <button
          type="button"
          class="bare mode"
          class:active={preferStructural}
          aria-pressed={preferStructural}
          title="Toggle Unity structural property diff vs raw YAML diff"
          onclick={() => (preferStructural = !preferStructural)}>{preferStructural ? "Structural" : "Raw"}</button
        > ·
      {/if}
      {#if onhistory}
        <button type="button" class="bare mode" title="File history" onclick={() => onhistory(path)}>History</button> ·
      {/if}
      {#if onblame}
        <button type="button" class="bare mode" title="Annotate (git blame)" onclick={() => onblame(path)}>Blame</button> ·
      {/if}
      <button
        type="button"
        class="bare mode"
        class:active={prefs.ignoreWhitespace}
        aria-pressed={prefs.ignoreWhitespace}
        title="Ignore whitespace changes"
        onclick={toggleIgnoreWhitespace}>±ws</button
      >
      ·
      <button type="button" class="bare mode" title="Ctrl+D" onclick={() => setDiffLayout(effectiveSplit ? "unified" : "split")}
        >{effectiveSplit ? "split" : "unified"} · Ctrl+D</button
      >
    </span>
  </h3>
  {#if error}
    <p role="alert">{error}</p>
  {:else if isUnityFile && preferStructural && unityDiff}
    <UnityDiffView diff={unityDiff} />
  {:else if diff}
    <DiffView
      {diff}
      {busy}
      hunkActions={actions}
      {lineActions}
      focused={perHunk ? focused : null}
      {images}
      split={effectiveSplit}
      onfetchlfs={handleFetchLfs}
    />
  {:else}
    <p class="loading-note"><LoadingBar label={isUnityFile && preferStructural ? "Loading structural diff…" : "Loading diff…"} /></p>
  {/if}
</section>

<style>
  .loading-note {
    margin: 12px 10px;
    font-size: 11.5px;
  }
  .working-diff {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-top: 1px solid var(--line);
  }
  .dh {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    margin: 0;
    padding: 0 10px;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
    font: 600 12px var(--body);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .side,
  .keys {
    color: var(--muted);
    font: 11px var(--mono);
    white-space: nowrap;
  }
  .keys {
    margin-left: auto;
  }
  .mode {
    all: unset;
    cursor: default;
  }
  .mode:hover {
    color: var(--fg);
    text-decoration: underline;
  }
  .mode.active {
    color: var(--l0);
    font-weight: 700;
  }
  p {
    padding: 0 10px;
  }
</style>
