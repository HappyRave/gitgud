<script lang="ts">
  import { openVersion } from "$lib/api";
  import type { ModelSide } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";

  let {
    repoId,
    path,
    title,
    lines = [],
    sides = [],
  }: {
    repoId: number;
    path: string;
    title: string;
    lines?: string[];
    /** Versions offered under "Open in default app". */
    sides?: ModelSide[];
  } = $props();

  let error = $state<string | null>(null);

  async function open(side: ModelSide) {
    error = null;
    try {
      await openVersion(repoId, path, toVersion(side.at), side.label);
    } catch (err) {
      error = String(err);
    }
  }
</script>

<div class="wrap">
  <div class="card" role="status">
    <span class="t">{title}</span>
    {#each lines as line, i (i)}
      <span class="m">{line}</span>
    {/each}
    {#if sides.length > 0}
      <div class="row">
        {#each sides as side (side.label)}
          <button type="button" onclick={() => open(side)}>Open {side.label} in default app</button>
        {/each}
      </div>
    {/if}
    {#if error}<span class="err" role="alert">{error}</span>{/if}
  </div>
</div>

<style>
  .wrap {
    display: grid;
    place-items: center;
    padding: 24px 12px;
    min-height: 160px;
    background: var(--bg);
  }
  .card {
    display: grid;
    gap: 6px;
    justify-items: start;
    max-width: 440px;
    padding: 14px 16px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--panel);
  }
  .t {
    font: 600 13px var(--body);
  }
  .m {
    font: 11px var(--mono);
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }
  .err {
    color: var(--bad-ink);
    font-size: 11.5px;
  }
</style>
