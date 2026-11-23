<script lang="ts">
  import { onMount } from "svelte";
  import Dialog from "./Dialog.svelte";

  let {
    kind,
    branch,
    remote = null,
    busy = false,
    onconfirm,
    oncancel,
  }: {
    /** `"remote"` deletes on the remote (irreversible); `"forceLocal"` force-deletes an unmerged local branch. */
    kind: "remote" | "forceLocal";
    branch: string;
    remote?: string | null;
    busy?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();

  let cancelButton = $state<HTMLButtonElement | null>(null);

  onMount(() => {
    cancelButton?.focus();
  });

  const title = $derived(
    kind === "remote" ? `Delete remote branch ${remote}/${branch}?` : `Force-delete unmerged branch ${branch}?`,
  );
</script>

<Dialog {title} width={440} {oncancel}>
  {#if kind === "remote"}
    <p class="flag bad">
      Deletes <b>{branch}</b> on <b>{remote}</b>. This removes the branch from the remote immediately and cannot be undone with Ctrl+Z.
    </p>
  {:else}
    <p class="flag">
      <b>{branch}</b> has commits that aren't merged into your current branch. Force-deleting removes the branch pointer (undoable with Ctrl+Z).
    </p>
  {/if}
  {#snippet footer()}
    <span class="ab">{kind === "remote" ? "Irreversible remotely" : "Undoable with Ctrl+Z"}</span>
    <span class="sp"></span>
    <button bind:this={cancelButton} onclick={oncancel}>Cancel <span class="k">Esc</span></button>
    <button class="pri bad" disabled={busy} onclick={onconfirm}>
      {kind === "remote" ? "Delete remote branch" : "Force delete"}
    </button>
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0;
  }
  .sp {
    flex: 1;
  }
</style>
