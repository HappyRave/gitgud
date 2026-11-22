<script lang="ts">
  import { onMount } from "svelte";
  import Dialog from "./Dialog.svelte";

  let {
    branch,
    hasUntracked = false,
    onlyUntracked = false,
    busy = false,
    onsave,
    oncancel,
  }: {
    branch: string;
    hasUntracked?: boolean;
    onlyUntracked?: boolean;
    busy?: boolean;
    onsave: (message: string | null, includeUntracked: boolean) => void;
    oncancel: () => void;
  } = $props();

  let message = $state("");
  let includeUntracked = $state(false);
  let input = $state<HTMLInputElement | null>(null);

  onMount(() => {
    includeUntracked = onlyUntracked;
    requestAnimationFrame(() => input?.focus());
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) {
      return;
    }
    onsave(message.trim() || null, includeUntracked);
  }
</script>

<Dialog title="Stash changes on {branch || 'HEAD'}" width={420} {oncancel}>
  <form onsubmit={submit}>
    <label class="field">
      <span class="ab">Message (optional)</span>
      <input bind:this={input} type="text" bind:value={message} placeholder="WIP on {branch || 'HEAD'}" aria-label="Stash message" />
    </label>
    {#if hasUntracked}
      <label class="chk">
        <input type="checkbox" bind:checked={includeUntracked} />
        <span>Include untracked files (<code>-u</code>)</span>
      </label>
    {/if}
    <div class="bt">
      <span class="ab">Undoable with Ctrl+Z</span>
      <span class="sp"></span>
      <button type="button" onclick={oncancel}>Cancel <span class="k">Esc</span></button>
      <button type="submit" class="pri" disabled={busy}>Stash changes <span class="k">↵</span></button>
    </div>
  </form>
</Dialog>

<style>
  form {
    display: grid;
    gap: 10px;
  }
  .field {
    display: grid;
    gap: 4px;
  }
  .chk {
    display: flex;
    gap: 6px;
    align-items: center;
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
