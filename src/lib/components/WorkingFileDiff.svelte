<script lang="ts">
  import { worktreeFileDiff, type FileDiff, type Hunk, type Side } from "$lib/api";
  import { imageUrl, isImage } from "$lib/images";
  import { prefs, toggleSplit } from "$lib/prefs.svelte";
  import DiffView from "./DiffView.svelte";

  let {
    repoId,
    path,
    side,
    revision,
    perHunk,
    busy = false,
    onhunk,
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
  } = $props();

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

  const images = $derived(
    !isImage(path)
      ? null
      : side === "staged"
        ? {
            before: { label: "HEAD", url: imageUrl(repoId, path, { at: "head" }, revision) },
            after: { label: "Staged", url: imageUrl(repoId, path, { at: "index" }, revision) },
          }
        : {
            before: { label: "Index", url: imageUrl(repoId, path, { at: "index" }, revision) },
            after: { label: "On disk", url: imageUrl(repoId, path, { at: "worktree" }, revision) },
          },
  );

  let diff = $state<FileDiff | null>(null);
  let error = $state<string | null>(null);
  /** The hunk Alt+↑↓ moved to; S / U / ⌫ act on it. */
  let focused = $state(0);

  $effect(() => {
    const key = `${repoId}:${path}:${side}:${revision}`;
    error = null;
    worktreeFileDiff(repoId, path, side).then(
      (d) => {
        if (key === `${repoId}:${path}:${side}:${revision}`) {
          diff = d;
          focused = Math.min(focused, Math.max(0, d.hunks.length - 1));
        }
      },
      (err) => (error = String(err)),
    );
  });

  $effect(() => {
    void path;
    void side;
    focused = 0;
  });

  /** Runs a hunk key; false when it doesn't apply here (wrong side, no hunks). */
  export function hunkKey(action: "next" | "prev" | "stage" | "unstage" | "discard"): boolean {
    const hunks = diff?.hunks ?? [];
    if (!perHunk || hunks.length === 0) {
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

<section class="working-diff" aria-label="{side === 'staged' ? 'Staged' : 'Unstaged'} changes in {path}">
  <h3 class="dh">
    <span class="name">{path}</span> <span class="side">{side === "staged" ? "staged" : "unstaged"}</span>
    <span class="keys">
      {#if perHunk && (diff?.hunks.length ?? 0) > 1}Alt+↑↓ hunks ·{/if}
      <button class="bare mode" title="Ctrl+D" onclick={toggleSplit}>{prefs.split ? "split" : "unified"} · Ctrl+D</button>
    </span>
  </h3>
  {#if error}
    <p role="alert">{error}</p>
  {:else if diff}
    <DiffView {diff} {busy} hunkActions={actions} focused={perHunk ? focused : null} {images} split={prefs.split} />
  {/if}
</section>

<style>
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
  p {
    padding: 0 10px;
  }
</style>
