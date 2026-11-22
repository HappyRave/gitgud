<script lang="ts">
  import { onMount } from "svelte";
  import { predictCherryPick, predictRevert } from "$lib/api";
  import Dialog from "./Dialog.svelte";

  let {
    repoId,
    commit,
    shortId,
    subject,
    branch,
    kind = "cherryPick",
    busy = false,
    onconfirm,
    oncancel,
  }: {
    repoId: number;
    commit: string;
    shortId: string;
    subject: string;
    branch: string;
    kind?: "cherryPick" | "revert";
    busy?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();

  let conflicts = $state<string[] | null>(null);
  let error = $state<string | null>(null);
  let goButton = $state<HTMLButtonElement | null>(null);

  const verb = $derived(kind === "cherryPick" ? "Cherry-pick" : "Revert");

  onMount(() => {
    goButton?.focus();
    const predict = kind === "cherryPick" ? predictCherryPick : predictRevert;
    predict(repoId, commit).then(
      (paths) => (conflicts = paths),
      (err) => (error = String(err)),
    );
  });
</script>

<Dialog title="{verb} {shortId} onto {branch}" width={460} {oncancel}>
  <p class="ab"><code>{shortId}</code> {subject}</p>
  {#if error}
    <p role="alert" class="flag bad">{error}</p>
  {:else if conflicts === null}
    <p class="ab">Checking for conflicts…</p>
  {:else if conflicts.length === 0}
    <p class="ok">✓ No conflicts expected: commits onto {branch} right away.</p>
  {:else}
    <p class="flag">
      {conflicts.length} {conflicts.length === 1 ? "file" : "files"} will conflict; you resolve them before finishing.
    </p>
    <ul>
      {#each conflicts.slice(0, 8) as path (path)}<li>{path}</li>{/each}
      {#if conflicts.length > 8}<li class="ab">and {conflicts.length - 8} more</li>{/if}
    </ul>
  {/if}
  {#snippet footer()}
    <span class="ab">Undoable with Ctrl+Z</span>
    <span class="sp"></span>
    <button onclick={oncancel}>Cancel <span class="k">Esc</span></button>
    <button class="pri" bind:this={goButton} disabled={!!error || busy} onclick={onconfirm}>
      {verb} <span class="k">↵</span>
    </button>
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0;
  }
  ul {
    margin: 0;
    padding-left: 18px;
    font: 11.5px/1.6 var(--mono);
  }
  .sp {
    flex: 1;
  }
</style>
