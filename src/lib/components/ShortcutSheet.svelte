<script lang="ts">
  import { onMount } from "svelte";
  import { SHORTCUT_GROUPS } from "$lib/keymap";

  let { onclose }: { onclose: () => void } = $props();

  let box = $state<HTMLDivElement | null>(null);

  onMount(() => {
    requestAnimationFrame(() => box?.focus());
  });
</script>

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div
    bind:this={box}
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-label="Keyboard shortcuts"
    tabindex="-1"
    onkeydown={(e) => e.key === "Escape" && (e.preventDefault(), onclose())}
  >
    <div class="hdr">
      <h2>Keyboard shortcuts</h2>
      <button type="button" class="bare x" aria-label="Close" onclick={onclose}>✕</button>
    </div>
    <div class="grid">
      {#each SHORTCUT_GROUPS as group (group.category)}
        <section class="grp">
          <h3>{group.category}</h3>
          <dl>
            {#each group.items as item (item.keys + item.label)}
              <dt><kbd>{item.keys}</kbd></dt>
              <dd>{item.label}</dd>
            {/each}
          </dl>
        </section>
      {/each}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(15, 20, 28, 0.42);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 40;
  }
  .sheet {
    width: min(760px, 94vw);
    max-height: 85vh;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 16px 18px;
    box-shadow: 0 18px 46px rgba(0, 0, 0, 0.28);
  }
  .hdr {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  h2 {
    margin: 0;
    font: 600 15px var(--display);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(290px, 1fr));
    gap: 14px 22px;
  }
  .grp h3 {
    margin: 0 0 6px;
    font: 600 11.5px var(--body);
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 5px 12px;
    margin: 0;
    align-items: center;
  }
  dt {
    margin: 0;
  }
  dd {
    margin: 0;
    font-size: 12px;
  }
  kbd {
    display: inline-block;
    padding: 1px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--bg);
    font: 11px var(--mono);
    white-space: nowrap;
  }
  .x {
    padding: 2px 6px;
  }
</style>
