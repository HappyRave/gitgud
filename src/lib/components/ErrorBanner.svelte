<script lang="ts">
  import { explainError, type FixId } from "$lib/errors";

  let {
    message,
    onfix,
    ondismiss,
  }: {
    message: string;
    onfix?: (fix: FixId) => void;
    ondismiss?: () => void;
  } = $props();

  const explained = $derived(explainError(message));
</script>

<div class="errbar" role="alert">
  <div class="main">
    <div class="head">
      <strong class="title">{explained.title}</strong>
      {#if explained.concept}
        <span class="concept">{explained.concept}</span>
      {/if}
      <span class="exp">{explained.explanation}</span>
      {#if explained.fix && onfix}
        <button type="button" class="fix" onclick={() => onfix(explained.fix!.id)}>{explained.fix.label}</button>
      {/if}
    </div>
    {#if explained.raw.trim() && explained.raw.trim() !== explained.explanation.trim()}
      <details class="raw">
        <summary>Details</summary>
        <pre>{explained.raw}</pre>
      </details>
    {/if}
  </div>
  {#if ondismiss}
    <button type="button" class="bare x" aria-label="Dismiss" title="Dismiss" onclick={ondismiss}>✕</button>
  {/if}
</div>

<style>
  .errbar {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 6px 12px;
    background: var(--bad);
    color: var(--bad-ink);
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .main {
    flex: 1;
    min-width: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
    font: 12px/1.35 var(--body);
  }
  .concept {
    font: 600 10px var(--mono);
    padding: 1px 5px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--bad-ink) 14%, transparent);
  }
  .exp {
    color: var(--fg);
  }
  .fix {
    height: 20px;
    padding: 0 8px;
    font-size: 11px;
    border-color: color-mix(in srgb, var(--bad-ink) 40%, var(--line));
  }
  .raw {
    margin-top: 3px;
  }
  .raw summary {
    cursor: pointer;
    font: 11px var(--mono);
    opacity: 0.85;
  }
  .raw pre {
    margin: 4px 0 0;
    padding: 6px 8px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--panel) 65%, transparent);
    color: var(--fg);
    font: 11px/1.4 var(--mono);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 160px;
    overflow: auto;
  }
  .x {
    padding: 2px 6px;
    border-radius: 4px;
    flex: none;
  }
  .x:hover {
    background: color-mix(in srgb, var(--bad-ink) 14%, transparent);
  }
</style>
