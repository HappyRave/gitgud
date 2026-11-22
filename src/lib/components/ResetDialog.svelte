<script lang="ts">
  import { onMount } from "svelte";
  import type { ResetMode } from "$lib/api";
  import Dialog from "./Dialog.svelte";

  let {
    branch,
    shortId,
    subject,
    dirty = false,
    busy = false,
    onreset,
    oncancel,
  }: {
    branch: string;
    shortId: string;
    subject: string;
    dirty?: boolean;
    busy?: boolean;
    onreset: (mode: ResetMode) => void;
    oncancel: () => void;
  } = $props();

  let mode = $state<ResetMode>("mixed");
  let goButton = $state<HTMLButtonElement | null>(null);

  onMount(() => {
    goButton?.focus();
  });

  const MODES: { id: ResetMode; label: string; desc: string }[] = [
    { id: "soft", label: "Soft", desc: "Move branch here; keep all changes staged in the index." },
    { id: "mixed", label: "Mixed", desc: "Move branch here; keep changes unstaged in the working copy." },
    { id: "hard", label: "Hard", desc: "Move branch here and discard staged and working-copy changes (Ctrl+Z restores them)." },
  ];
</script>

<Dialog title="Reset {branch} to {shortId}" width={480} {oncancel}>
  <p class="ab">Target: <code>{shortId}</code> {subject}</p>
  {#each MODES as m (m.id)}
    <label class="opt" class:sel={mode === m.id} class:warn={m.id === "hard" && mode === "hard"}>
      <input type="radio" name="reset-mode" value={m.id} bind:group={mode} />
      <div>
        <b>{m.label}</b>
        <span class="ab">{m.desc}</span>
      </div>
    </label>
  {/each}
  {#if dirty && mode === "hard"}
    <p class="flag">You have uncommitted changes — Hard reset discards tracked edits (undoable with Ctrl+Z).</p>
  {/if}
  {#snippet footer()}
    <span class="ab">Undoable with Ctrl+Z</span>
    <span class="sp"></span>
    <button onclick={oncancel}>Cancel <span class="k">Esc</span></button>
    <button class="pri" bind:this={goButton} disabled={busy} onclick={() => onreset(mode)}>
      Reset ({mode}) <span class="k">↵</span>
    </button>
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0;
  }
  .opt {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: 7px;
    background: var(--bg);
  }
  .opt.sel {
    border-color: var(--l0);
    background: var(--sel);
  }
  .opt.warn {
    border-color: var(--bad-ink);
    background: var(--bad);
  }
  .opt div {
    display: grid;
    gap: 2px;
  }
  .sp {
    flex: 1;
  }
</style>
