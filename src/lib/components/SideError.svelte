<script lang="ts">
  import type { BinarySide } from "$lib/api";
  import { formatSize } from "$lib/merge/resolve";

  let {
    label,
    error,
    which = "before",
    meta = null,
    onfetch,
  }: {
    label: string;
    error: string;
    which?: "before" | "after";
    meta?: BinarySide | null;
    onfetch?: (side: "before" | "after", meta: BinarySide) => Promise<void>;
  } = $props();

  let fetching = $state(false);
  let fetchError = $state<string | null>(null);
  const missing = $derived(!!meta?.lfs && /isn't downloaded/.test(error));

  async function run() {
    if (!onfetch || !meta || fetching) {
      return;
    }
    fetching = true;
    fetchError = null;
    try {
      await onfetch(which, meta);
    } catch (e) {
      fetchError = e instanceof Error ? e.message : String(e);
    } finally {
      fetching = false;
    }
  }
</script>

<div class="gate" role="alert">
  <span><b>{label}</b> can't load: {error}</span>
  {#if missing && onfetch && meta}
    <button type="button" class="fetchbtn" disabled={fetching} onclick={run}>
      {fetching ? "Fetching…" : `Fetch LFS object (${formatSize(meta.size)})`}
    </button>
  {/if}
  {#if fetchError}<span class="ferr">{fetchError}</span>{/if}
</div>

<style>
  .gate {
    display: grid;
    gap: 6px;
    justify-items: center;
    padding: 14px 10px;
    text-align: center;
    color: var(--muted);
    font-size: 11.5px;
  }
  .fetchbtn {
    height: 24px;
    padding: 0 9px;
    font-size: 11.5px;
  }
  .ferr {
    color: var(--bad-ink);
    font-size: 11px;
  }
</style>
