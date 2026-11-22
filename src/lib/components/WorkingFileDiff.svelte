<script lang="ts">
  import { worktreeFileDiff, type FileDiff, type Hunk, type Side } from "$lib/api";
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
        ? [{ label: "Unstage hunk", run: (h: Hunk) => onhunk("unstage", h) }]
        : [
            { label: "Discard hunk", run: (h: Hunk) => onhunk("discard", h) },
            { label: "Stage hunk", run: (h: Hunk) => onhunk("stage", h) },
          ],
  );

  let diff = $state<FileDiff | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const key = `${repoId}:${path}:${side}:${revision}`;
    error = null;
    worktreeFileDiff(repoId, path, side).then(
      (d) => {
        if (key === `${repoId}:${path}:${side}:${revision}`) {
          diff = d;
        }
      },
      (err) => (error = String(err)),
    );
  });
</script>

<section class="working-diff" aria-label="{side === 'staged' ? 'Staged' : 'Unstaged'} changes in {path}">
  <h3>{path} <span class="side">{side === "staged" ? "staged" : "unstaged"}</span></h3>
  {#if error}
    <p role="alert">{error}</p>
  {:else if diff}
    <DiffView {diff} {busy} hunkActions={actions} />
  {/if}
</section>

<style>
  .working-diff {
    margin-top: 1rem;
  }
  h3 {
    font-size: 0.85rem;
    font-family: ui-monospace, monospace;
    margin: 0 0 0.3rem;
    overflow-wrap: anywhere;
  }
  .side {
    color: #888;
    font-family: system-ui, sans-serif;
    font-weight: normal;
  }
</style>
