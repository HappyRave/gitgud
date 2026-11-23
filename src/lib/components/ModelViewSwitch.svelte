<script lang="ts">
  import { prefs, setModelView } from "$lib/prefs.svelte";

  let { textLabel = "Text" }: { textLabel?: string } = $props();
</script>

<div class="switch" role="radiogroup" aria-label="View">
  <button type="button" role="radio" aria-checked={prefs.modelView === "3d"} class:on={prefs.modelView === "3d"} onclick={() => setModelView("3d")}
    >3D</button
  ><button
    type="button"
    role="radio"
    aria-checked={prefs.modelView === "source"}
    class:on={prefs.modelView === "source"}
    onclick={() => setModelView("source")}>{textLabel}</button
  >
</div>

<style>
  .switch {
    display: inline-flex;
    margin: 6px 10px 0;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
  }
  button {
    all: unset;
    font-size: 10.5px;
    padding: 1px 9px;
    color: var(--muted);
    border-right: 1px solid var(--line);
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
  }
</style>
