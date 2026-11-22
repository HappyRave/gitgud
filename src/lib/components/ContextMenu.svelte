<script lang="ts" module>
  export interface MenuItem {
    label: string;
    hint?: string;
    disabled?: boolean;
    run: () => void;
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";

  let {
    x,
    y,
    items,
    onclose,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  } = $props();

  let menu = $state<HTMLElement | null>(null);

  onMount(() => {
    menu?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }
    event.preventDefault();
    const buttons = [...(menu?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = (at + (event.key === "ArrowDown" ? 1 : buttons.length - 1)) % buttons.length;
    buttons[next]?.focus();
  }

  function onPointerDown(event: PointerEvent) {
    if (menu && !menu.contains(event.target as Node)) {
      onclose();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} onpointerdown={onPointerDown} onblur={onclose} />

<div class="menu" role="menu" bind:this={menu} style:left="{x}px" style:top="{y}px">
  {#each items as item (item.label)}
    <button
      class="bare"
      role="menuitem"
      disabled={item.disabled}
      onclick={() => {
        onclose();
        item.run();
      }}
    >
      {item.label}
      {#if item.hint}<span class="k">{item.hint}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 30;
    min-width: 200px;
    padding: 4px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 7px;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.18);
    display: grid;
  }
  button {
    all: unset;
    display: flex;
    gap: 12px;
    align-items: center;
    height: 26px;
    padding: 0 10px;
    border-radius: 4px;
    white-space: nowrap;
    cursor: default;
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--sel);
    outline: none;
  }
  button:disabled {
    color: var(--muted);
  }
  .k {
    margin-left: auto;
    font: 10.5px var(--mono);
    color: var(--muted);
  }
</style>
