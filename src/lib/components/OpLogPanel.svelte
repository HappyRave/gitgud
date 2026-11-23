<script lang="ts">
  import { opLog, type OpEntry, type OpLogStats, type ReflogEntry } from "$lib/api";

  let {
    repoId,
    revision,
    busy = false,
    stats = null,
    reflog = [],
    onundo,
    onrestoreto,
    onrestorefiles,
    onprune,
    onrestorereflog,
  }: {
    repoId: number;
    /** Changes after every write, so the list reloads. */
    revision: number;
    busy?: boolean;
    stats?: OpLogStats | null;
    reflog?: ReflogEntry[];
    onundo: () => void;
    /** Undo everything back to before this entry. */
    onrestoreto: (entry: OpEntry) => void;
    /** Put back the files a discard changed, as a new action. */
    onrestorefiles: (entry: OpEntry) => void;
    onprune?: () => void;
    onrestorereflog?: (entry: ReflogEntry) => void;
  } = $props();

  let entries = $state<OpEntry[] | null>(null);
  let error = $state<string | null>(null);
  let selectedId = $state<number | null>(null);

  const selected = $derived(entries?.find((e) => e.id === selectedId) ?? null);
  const newest = $derived(entries?.find((e) => !e.undone) ?? null);

  $effect(() => {
    const key = `${repoId}:${revision}`;
    opLog(repoId).then(
      (list) => {
        if (key === `${repoId}:${revision}`) {
          entries = list;
          error = null;
        }
      },
      (err) => (error = String(err)),
    );
  });

  function time(seconds: number) {
    const d = new Date(seconds * 1000);
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) {
      return `${bytes} B`;
    }
    if (bytes < 1024 * 1024) {
      return `${(bytes / 1024).toFixed(1)} KB`;
    }
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<section class="oplog" aria-label="Op log">
  <h2 class="ph">Op log <span class="ct">· every action, discards included</span><span class="r">Ctrl+3</span></h2>

  {#if error}
    <p role="alert">{error}</p>
  {:else if entries && entries.length === 0}
    <p class="note">Nothing yet. Every stage, commit and discard lands here, and Ctrl+Z undoes it.</p>
  {:else if entries}
    <ul aria-label="Actions">
      {#each entries as entry (entry.id)}
        <li class:undone={entry.undone} class:selected={entry.id === selectedId}>
          <button class="row" aria-pressed={entry.id === selectedId} onclick={() => (selectedId = entry.id)}>
            <span class="tm">{time(entry.time)}</span>
            <span class="t">{entry.action}</span>
          </button>
          {#if entry.undone}
            <span class="state">undone</span>
          {:else if entry.id === newest?.id}
            <button class="lk" disabled={busy} onclick={onundo}>Undo</button>
          {:else}
            <button class="lk" disabled={busy} onclick={() => onrestoreto(entry)}>Restore</button>
          {/if}
        </li>
      {/each}
    </ul>

    {#if selected}
      <div class="card" aria-label="Entry {time(selected.time)}">
        <b>{time(selected.time)} · {selected.action}</b>
        {#if selected.paths.length > 0}
          <p class="ab">Snapshot before: {selected.paths.join(", ")}</p>
        {/if}
        {#if !selected.undone}
          <div class="buttons">
            {#if selected.paths.length > 0}
              <button class="pri" disabled={busy} onclick={() => onrestorefiles(selected)}>Restore these files</button>
            {/if}
            <button disabled={busy} onclick={() => onrestoreto(selected)}>Restore whole repo to {time(selected.time)}</button>
          </div>
        {/if}
      </div>
    {/if}
  {/if}

  {#if stats}
    <div class="statsbar" aria-label="Op log storage">
      <span>{stats.entryCount} {stats.entryCount === 1 ? "entry" : "entries"} · {formatBytes(stats.totalBytes)}</span>
      {#if stats.prunableCount > 0 && onprune}
        <button type="button" class="lk" disabled={busy} onclick={onprune}>
          Prune old ({stats.prunableCount} · {formatBytes(stats.reclaimableBytes)})
        </button>
      {/if}
    </div>
  {/if}

  {#if reflog.length > 0}
    <details class="reflog">
      <summary>Outside gitgud (reflog) <span class="ct">· {reflog.length}</span></summary>
      <ul class="reflist" aria-label="Git reflog">
        {#each reflog as r (`${r.selector}-${r.id}`)}
          <li>
            <span class="selid mono">{r.selector}</span>
            <span class="rsha mono">{r.shortId}</span>
            <span class="raction" title={r.action}>{r.action}</span>
            {#if onrestorereflog}
              <button type="button" class="lk" disabled={busy} onclick={() => onrestorereflog(r)}>Inspect</button>
            {/if}
          </li>
        {/each}
      </ul>
    </details>
  {/if}
</section>

<style>
  .oplog {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  h2 {
    margin: 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 2px 0;
  }
  .oplog > ul {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    height: var(--row-h, 24px);
    padding-right: 6px;
  }
  li:hover {
    background: var(--hover);
  }
  li.selected {
    background: var(--sel);
  }
  li.undone .t,
  li.undone .tm {
    color: var(--muted);
    text-decoration: line-through;
  }
  .row {
    all: unset;
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr);
    gap: 6px;
    padding: 0 10px;
    cursor: default;
    white-space: nowrap;
  }
  .row:focus-visible {
    outline: 2px solid var(--l0);
    outline-offset: -2px;
  }
  .tm {
    color: var(--muted);
    font: 10.5px/var(--row-h, 24px) var(--mono);
  }
  .t {
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: var(--row-h, 24px);
  }
  .lk {
    all: unset;
    color: var(--l0);
    font-size: 11px;
    padding: 0 6px;
    cursor: default;
  }
  .lk:hover:not(:disabled) {
    text-decoration: underline;
  }
  .lk:disabled {
    color: var(--muted);
  }
  .state {
    color: var(--muted);
    font-size: 11px;
    padding: 0 6px;
  }
  .card {
    display: grid;
    gap: 6px;
    padding: 8px 10px;
  }
  .card .ab {
    margin: 0;
  }
  .buttons {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .note {
    color: var(--muted);
    padding: 10px;
    margin: 0;
  }
  .statsbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 10px;
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 11px;
  }
  .reflog {
    border-top: 1px solid var(--line);
    font-size: 11.5px;
  }
  .reflog summary {
    padding: 6px 10px;
    cursor: default;
    font-weight: 600;
    color: var(--muted);
  }
  .reflist {
    list-style: none;
    margin: 0;
    padding: 0 0 6px;
    max-height: 180px;
    overflow: auto;
  }
  .reflist li {
    display: grid;
    grid-template-columns: auto auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px;
    padding: 2px 10px;
  }
  .selid {
    color: var(--muted);
    font-size: 10.5px;
  }
  .rsha {
    color: var(--l0);
    font-size: 11px;
  }
  .raction {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mono {
    font-family: var(--mono);
  }
</style>
