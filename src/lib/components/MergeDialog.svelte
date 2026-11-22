<script lang="ts">
  import { onMount } from "svelte";
  import { predictMerge } from "$lib/api";

  let {
    repoId,
    branch,
    into,
    dirty = false,
    onmerge,
    oncancel,
  }: {
    repoId: number;
    /** The branch to merge in. */
    branch: string;
    /** The current branch. */
    into: string;
    /** Uncommitted changes in the working copy. */
    dirty?: boolean;
    onmerge: () => void;
    oncancel: () => void;
  } = $props();

  let conflicts = $state<string[] | null>(null);
  let error = $state<string | null>(null);
  let mergeButton = $state<HTMLButtonElement | null>(null);

  onMount(() => {
    mergeButton?.focus();
    predictMerge(repoId, branch).then(
      (paths) => (conflicts = paths),
      (err) => (error = String(err)),
    );
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      oncancel();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && oncancel()}>
  <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="merge-title">
    <h2 id="merge-title">Merge <b>{branch}</b> into <b>{into}</b></h2>
    <div class="pred" aria-live="polite">
      {#if error}
        <p role="alert">{error}</p>
      {:else if conflicts === null}
        <p class="ab">Checking for conflicts…</p>
      {:else if conflicts.length === 0}
        <p class="ok">✓ No conflicts expected: the merge commits right away.</p>
      {:else}
        <p class="flag">
          {conflicts.length}
          {conflicts.length === 1 ? "file" : "files"} will conflict; you resolve them before the merge is committed.
        </p>
        <ul>
          {#each conflicts.slice(0, 8) as path (path)}<li>{path}</li>{/each}
          {#if conflicts.length > 8}<li class="ab">and {conflicts.length - 8} more</li>{/if}
        </ul>
      {/if}
      {#if dirty}
        <p class="ab">You have uncommitted changes. git stops if the merge would overwrite them.</p>
      {/if}
    </div>
    <div class="bt">
      <span class="ab">Undoable with Ctrl+Z</span>
      <span class="sp"></span>
      <button onclick={oncancel}>Cancel <span class="k">Esc</span></button>
      <button class="pri" bind:this={mergeButton} disabled={!!error} onclick={onmerge}>Merge <span class="k">↵</span></button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: grid;
    place-items: start center;
    padding-top: 14vh;
    z-index: 20;
  }
  .dlg {
    width: min(460px, 92vw);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 18px 60px rgba(0, 0, 0, 0.3);
    padding: 14px 16px 12px;
    display: grid;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font: 500 15px var(--display);
  }
  h2 b {
    font-weight: 700;
  }
  .pred p {
    margin: 0 0 6px;
  }
  ul {
    margin: 0 0 6px;
    padding-left: 18px;
    font: 11.5px/1.6 var(--mono);
  }
  .bt {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .sp {
    flex: 1;
  }
</style>
