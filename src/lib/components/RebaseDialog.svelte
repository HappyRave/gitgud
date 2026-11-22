<script lang="ts">
  import { onMount } from "svelte";
  import { predictRebase, type RebasePlan } from "$lib/api";

  let {
    repoId,
    onto,
    branch,
    dirty = false,
    onrebase,
    oncancel,
  }: {
    repoId: number;
    /** The branch to rebase onto. */
    onto: string;
    /** The current branch, which gets rewritten. */
    branch: string;
    dirty?: boolean;
    onrebase: () => void;
    oncancel: () => void;
  } = $props();

  let plan = $state<RebasePlan | null>(null);
  let error = $state<string | null>(null);
  let goButton = $state<HTMLButtonElement | null>(null);

  const nothing = $derived(!!plan && plan.behind === 0);

  onMount(() => {
    goButton?.focus();
    predictRebase(repoId, onto).then(
      (p) => (plan = p),
      (err) => (error = String(err)),
    );
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      oncancel();
    }
  }

  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
</script>

<svelte:window onkeydown={onKeydown} />

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && oncancel()}>
  <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="rebase-title">
    <h2 id="rebase-title">Rebase <b>{branch}</b> onto <b>{onto}</b></h2>
    <div class="pred" aria-live="polite">
      {#if error}
        <p role="alert">{error}</p>
      {:else if !plan}
        <p class="ab">Checking each step for conflicts…</p>
      {:else if nothing}
        <p class="ok">✓ {branch} already contains everything on {onto}: nothing to do.</p>
      {:else if plan.commits === 0}
        <p class="ok">✓ No commits of its own: {branch} moves forward to {onto}.</p>
      {:else}
        <p>Replays {plural(plan.commits, "commit")} of {branch} on top of {plural(plan.behind, "new commit")} from {onto}.</p>
        {#if plan.conflict}
          <p class="flag">
            Step {plan.conflict.step} ({plan.conflict.shortId} "{plan.conflict.subject}") will conflict; the rebase stops
            there for you to resolve.
          </p>
          <ul>
            {#each plan.conflict.paths.slice(0, 8) as path (path)}<li>{path}</li>{/each}
            {#if plan.conflict.paths.length > 8}<li class="ab">and {plan.conflict.paths.length - 8} more</li>{/if}
          </ul>
        {:else}
          <p class="ok">✓ No conflicts expected.</p>
        {/if}
      {/if}
      {#if dirty}
        <p class="ab">You have uncommitted changes. git stops if the rebase would overwrite them.</p>
      {/if}
    </div>
    <div class="bt">
      <span class="ab">Rewrites {branch}; undoable with Ctrl+Z</span>
      <span class="sp"></span>
      <button onclick={oncancel}>Cancel <span class="k">Esc</span></button>
      <button class="pri" bind:this={goButton} disabled={!!error || nothing} onclick={onrebase}>Rebase <span class="k">↵</span></button>
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
    width: min(480px, 92vw);
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
