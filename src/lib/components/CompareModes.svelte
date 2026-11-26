<script lang="ts" generics="M extends string">
  let {
    mode = $bindable(),
    onion = $bindable(),
    enabled,
    modes,
    onionLabel,
    onionName = "Onion opacity",
    onionMin = 0,
    onionMax = 1,
  }: {
    mode: M;
    onion: number;
    /** Off while only one version is shown. */
    enabled: boolean;
    modes: [M, string][];
    onionLabel: string;
    onionName?: string;
    onionMin?: number;
    onionMax?: number;
  } = $props();
</script>

<div class="seg" role="radiogroup" aria-label="Compare mode">
  {#each modes as [m, label] (m)}
    <button
      type="button"
      class="tbtn"
      class:on={enabled && mode === m}
      role="radio"
      aria-checked={enabled && mode === m}
      disabled={!enabled}
      onclick={() => (mode = m)}>{label}</button
    >
  {/each}
</div>
{#if enabled && mode === "onion"}
  <label class="onion"
    >{onionLabel}
    <input type="range" min={onionMin} max={onionMax} step="0.05" bind:value={onion} aria-label={onionName} /></label
  >
{/if}

<style>
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
  }
  .tbtn {
    all: unset;
    font-size: 10.5px;
    padding: 1px 7px;
    color: var(--muted);
    border-right: 1px solid var(--line);
    cursor: default;
    white-space: nowrap;
  }
  .tbtn:last-child {
    border-right: 0;
  }
  .tbtn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--fg);
  }
  .tbtn.on {
    background: var(--sel);
    color: var(--fg);
    font-weight: 600;
  }
  .tbtn:disabled {
    opacity: 0.4;
  }
  .tbtn:focus-visible {
    outline: 2px solid var(--l0);
  }
  .onion {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: 10.5px var(--mono);
    color: var(--muted);
  }
  .onion input {
    width: 90px;
  }
</style>
