<script lang="ts">
  import { onMount } from "svelte";
  import Dialog from "./Dialog.svelte";

  let {
    targetLabel = null,
    busy = false,
    onsubmit,
    oncancel,
  }: {
    targetLabel?: string | null;
    busy?: boolean;
    onsubmit: (name: string, message: string | null) => void;
    oncancel: () => void;
  } = $props();

  let name = $state("");
  let message = $state("");
  let input = $state<HTMLInputElement | null>(null);

  const valid = $derived(name.trim().length > 0 && !/\s/.test(name.trim()));

  onMount(() => {
    requestAnimationFrame(() => input?.focus());
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!valid || busy) {
      return;
    }
    const trimmed = message.trim();
    onsubmit(name.trim(), trimmed ? trimmed : null);
  }
</script>

<Dialog title="New tag" width={420} {oncancel}>
  <form onsubmit={submit}>
    {#if targetLabel}
      <p class="ab">Points to {targetLabel}</p>
    {/if}
    <label class="field">
      <span class="ab">Tag name</span>
      <input bind:this={input} type="text" bind:value={name} placeholder="v1.0.0" aria-label="Tag name" />
    </label>
    <label class="field">
      <span class="ab">Annotation message (optional)</span>
      <textarea
        rows="3"
        bind:value={message}
        placeholder="Leave empty for a lightweight tag"
        aria-label="Annotation message"
      ></textarea>
    </label>
    <div class="bt">
      <span class="ab">{message.trim() ? "Annotated tag" : "Lightweight tag"}</span>
      <span class="sp"></span>
      <button type="button" onclick={oncancel}>Cancel <span class="k">Esc</span></button>
      <button type="submit" class="pri" disabled={!valid || busy}>Create tag <span class="k">↵</span></button>
    </div>
  </form>
</Dialog>

<style>
  form {
    display: grid;
    gap: 10px;
  }
  p {
    margin: 0;
  }
  .field {
    display: grid;
    gap: 4px;
  }
  textarea {
    resize: vertical;
    font: inherit;
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
