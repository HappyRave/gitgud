<script lang="ts">
  import type { BinaryMeta, BinarySide } from "$lib/api";
  import { formatSizeDelta } from "$lib/images";
  import { formatSize } from "$lib/merge/resolve";

  let {
    path,
    meta = null,
    busy = false,
    onfetch,
  }: {
    path: string;
    meta?: BinaryMeta | null;
    busy?: boolean;
    onfetch?: (side: "before" | "after", sideMeta: BinarySide) => Promise<void>;
  } = $props();

  let fetching = $state<{ before: boolean; after: boolean }>({ before: false, after: false });
  let fetchError = $state<{ before: string | null; after: string | null }>({ before: null, after: null });

  $effect(() => {
    void path;
    void meta;
    fetching = { before: false, after: false };
    fetchError = { before: null, after: null };
  });

  const extLabel = $derived.by(() => {
    const m = /\.([^./\\]+)$/.exec(path);
    return m ? m[1].toUpperCase() : "BIN";
  });

  const anyLfs = $derived(Boolean(meta?.before?.lfs || meta?.after?.lfs));

  const deltaLabel = $derived.by(() => {
    if (!meta) return null;
    if (!meta.before && meta.after) return `Added · ${formatSize(meta.after.size)}`;
    if (meta.before && !meta.after) return `Removed · ${formatSize(meta.before.size)}`;
    return formatSizeDelta(meta.before?.size, meta.after?.size);
  });

  async function runFetch(which: "before" | "after", side: BinarySide) {
    if (!onfetch || fetching[which] || busy) return;
    fetching = { ...fetching, [which]: true };
    fetchError = { ...fetchError, [which]: null };
    try {
      await onfetch(which, side);
    } catch (err) {
      fetchError = { ...fetchError, [which]: String(err) };
    } finally {
      fetching = { ...fetching, [which]: false };
    }
  }
</script>

{#if !meta}
  <p class="note">Binary file, no text diff.</p>
{:else}
  <section class="bincard" aria-label="Binary details for {path}">
    <header class="bhead">
      <span class="tag ext">{extLabel}</span>
      {#if anyLfs}
        <span class="tag lfs">LFS</span>
      {/if}
      <span class="bsub">Binary asset</span>
      {#if deltaLabel}
        <span class="delta">{deltaLabel}</span>
      {/if}
    </header>
    <div class="cols">
      {#each [{ key: "before" as const, label: "Before", side: meta.before, empty: "Added" }, { key: "after" as const, label: "After", side: meta.after, empty: "Deleted" }] as col (col.key)}
        <div class="col">
          <div class="clabel">{col.label}</div>
          {#if col.side}
            {@const s = col.side}
            {@const hydrated = s.hydrated !== false}
            <div class="cval">
              <strong>{formatSize(s.size)}</strong>
              {#if s.lfs}
                <span class="pill" class:ok={hydrated} class:ptr={!hydrated}>
                  {hydrated ? "Hydrated" : "Pointer only"}
                </span>
              {:else}
                <span class="pill">Git blob</span>
              {/if}
            </div>
            {#if s.oid}
              <div class="oid mono" title="sha256:{s.oid}">OID {s.oid.slice(0, 12)}</div>
            {/if}
            {#if s.lfs && !hydrated && onfetch}
              <button
                type="button"
                class="fetchbtn"
                disabled={busy || fetching[col.key]}
                onclick={() => runFetch(col.key, s)}
              >
                {fetching[col.key] ? "Fetching…" : `Fetch LFS object (${formatSize(s.size)})`}
              </button>
            {/if}
            {#if fetchError[col.key]}
              <p class="ferr" role="alert">{fetchError[col.key]}</p>
            {/if}
          {:else}
            <div class="cempty">{col.empty}</div>
          {/if}
        </div>
      {/each}
    </div>
  </section>
{/if}

<style>
  .note {
    padding: 0 10px;
    color: var(--muted);
  }
  .bincard {
    margin: 10px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--bg);
    overflow: hidden;
  }
  .bhead {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    font-size: 11.5px;
  }
  .tag {
    font: 700 10px var(--mono);
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--hover);
    color: var(--fg);
  }
  .tag.lfs {
    background: var(--sel);
    color: var(--l0);
  }
  .bsub {
    color: var(--muted);
  }
  .delta {
    margin-left: auto;
    font: 600 11px var(--mono);
    color: var(--fg);
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1px;
    background: var(--line);
  }
  .col {
    padding: 10px;
    background: var(--panel);
    display: grid;
    gap: 6px;
    align-content: start;
  }
  .clabel {
    font: 600 11px var(--body);
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .cval {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    font-family: var(--mono);
    font-size: 13px;
  }
  .pill {
    font: 600 10px var(--mono);
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--hover);
    color: var(--muted);
  }
  .pill.ok {
    background: var(--add);
    color: var(--add-ink);
  }
  .pill.ptr {
    background: var(--warn);
    color: var(--warn-ink);
  }
  .oid {
    font-size: 10.5px;
    color: var(--muted);
  }
  .mono {
    font-family: var(--mono);
  }
  .cempty {
    font-style: italic;
    color: var(--muted);
    font-size: 12px;
  }
  .fetchbtn {
    justify-self: start;
    height: 24px;
    padding: 0 9px;
    font-size: 11.5px;
  }
  .ferr {
    margin: 0;
    color: var(--bad-ink);
    font-size: 11px;
  }
</style>
