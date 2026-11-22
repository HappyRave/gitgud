<script lang="ts">
  import { onMount } from "svelte";
  import Dialog from "./Dialog.svelte";

  let {
    mode,
    currentName = "",
    atLabel = null,
    remoteBranch = null,
    busy = false,
    onsubmit,
    oncancel,
  }: {
    mode: "create" | "rename" | "checkoutRemote";
    currentName?: string;
    atLabel?: string | null;
    remoteBranch?: string | null;
    busy?: boolean;
    onsubmit: (name: string, checkout: boolean) => void;
    oncancel: () => void;
  } = $props();

  let name = $state("");
  let checkout = $state(true);
  let input = $state<HTMLInputElement | null>(null);

  const title = $derived(
    mode === "rename"
      ? `Rename branch ${currentName}`
      : mode === "checkoutRemote"
        ? `Check out ${remoteBranch ?? ""}`
        : "New branch",
  );
  const valid = $derived(name.trim().length > 0 && !/\s/.test(name.trim()));

  onMount(() => {
    name = mode === "rename" ? currentName : mode === "checkoutRemote" ? (remoteBranch?.split("/").slice(1).join("/") ?? "") : "";
    requestAnimationFrame(() => {
      input?.focus();
      input?.select();
    });
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!valid || busy) {
      return;
    }
    onsubmit(name.trim(), checkout);
  }
</script>

<Dialog {title} width={420} {oncancel}>
  <form onsubmit={submit}>
    {#if atLabel}
      <p class="ab">Starts at {atLabel}</p>
    {/if}
    <label class="field">
      <span class="ab">{mode === "rename" ? "New branch name" : "Branch name"}</span>
      <input bind:this={input} type="text" bind:value={name} placeholder="feature/my-change" aria-label="Branch name" />
    </label>
    {#if mode === "create"}
      <label class="chk">
        <input type="checkbox" bind:checked={checkout} />
        <span>Check out immediately</span>
      </label>
    {/if}
    <div class="bt">
      <span class="ab">Undoable with Ctrl+Z</span>
      <span class="sp"></span>
      <button type="button" onclick={oncancel}>Cancel <span class="k">Esc</span></button>
      <button type="submit" class="pri" disabled={!valid || busy}>
        {mode === "rename" ? "Rename" : mode === "checkoutRemote" ? "Check out" : "Create branch"} <span class="k">↵</span>
      </button>
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
