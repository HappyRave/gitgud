<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    width = 460,
    oncancel,
    children,
    footer,
  }: {
    title: string;
    width?: number;
    oncancel: () => void;
    children: Snippet;
    footer?: Snippet;
  } = $props();

  const headingId = `dlg-${Math.random().toString(36).slice(2, 8)}`;

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      oncancel();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && oncancel()}>
  <div class="dlg" style:--w="{width}px" role="dialog" aria-modal="true" aria-labelledby={headingId}>
    <h2 id={headingId}>{title}</h2>
    <div class="body">{@render children()}</div>
    {#if footer}
      <div class="bt">{@render footer()}</div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: grid;
    place-items: start center;
    padding-top: 12vh;
    z-index: 20;
  }
  .dlg {
    width: min(var(--w, 460px), 92vw);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 18px 60px rgba(0, 0, 0, 0.3);
    padding: 14px 16px 12px;
    display: grid;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font: 600 15px var(--display);
  }
  .body {
    display: grid;
    gap: 8px;
  }
  .bt {
    display: flex;
    gap: 6px;
    align-items: center;
  }
</style>
