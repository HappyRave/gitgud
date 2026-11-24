<script lang="ts">
  import { setViewMode, viewModeFor } from "$lib/prefs.svelte";
  import { type ViewMode, viewModes } from "$lib/viewers/route";

  let { path }: { path: string } = $props();

  const LABEL: Record<ViewMode, string> = { visual: "Visual", structure: "Structure", source: "Source" };
  const TITLE: Record<ViewMode, string> = {
    visual: "Show the file in its viewer",
    structure: "Unity's objects and properties, compared",
    source: "The text diff",
  };
  const modes = $derived(viewModes(path));
  const current = $derived(viewModeFor(path));
</script>

{#if modes.length > 1}
  <span class="switch" role="radiogroup" aria-label="View">
    {#each modes as m (m)}
      <button
        type="button"
        role="radio"
        aria-checked={current === m}
        class:on={current === m}
        title={TITLE[m]}
        onclick={() => setViewMode(path, m)}>{LABEL[m]}</button
      >
    {/each}
  </span>
{/if}

<style>
  .switch {
    display: inline-flex;
    flex: none;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
    font: 500 10.5px var(--body);
  }
  button {
    all: unset;
    padding: 1px 8px;
    color: var(--muted);
    border-right: 1px solid var(--line);
    cursor: default;
  }
  button:last-child {
    border-right: 0;
  }
  button:hover {
    background: var(--hover);
    color: var(--fg);
  }
  button.on {
    background: var(--sel);
    color: var(--fg);
    font-weight: 600;
  }
  button:focus-visible {
    outline: 2px solid var(--l0);
    outline-offset: -2px;
  }
</style>
