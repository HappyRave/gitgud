<script lang="ts">
  import { opLog, type OpEntry } from "$lib/api";

  let {
    repoId,
    revision,
    busy = false,
    onundo,
    onrestoreto,
    onrestorefiles,
  }: {
    repoId: number;
    /** Changes after every write, so the list reloads. */
    revision: number;
    busy?: boolean;
    onundo: () => void;
    /** Undo everything back to before this entry. */
    onrestoreto: (entry: OpEntry) => void;
    /** Put back the files a discard changed, as a new action. */
    onrestorefiles: (entry: OpEntry) => void;
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
</script>

<section class="oplog" aria-label="Op log">
  <h2>Op log <span class="ct">· every action, discards included</span><span class="key">Ctrl+3</span></h2>

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
</section>

<style>
  h2 {
    font-size: 0.85rem;
    margin: 0 0 0.5rem;
    display: flex;
    gap: 0.3rem;
  }
  .ct {
    color: #888;
    font-weight: normal;
  }
  .key {
    margin-left: auto;
    color: #aaa;
    font-weight: normal;
    font-size: 0.75rem;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    height: 24px;
  }
  li.selected {
    background: #e3ecfd;
  }
  li.undone .t,
  li.undone .tm {
    color: #aaa;
    text-decoration: line-through;
  }
  .row {
    all: unset;
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr);
    gap: 6px;
    padding: 0 6px;
    cursor: pointer;
    white-space: nowrap;
    font-size: 0.85rem;
  }
  .row:focus-visible {
    outline: 2px solid #4c8bf5;
  }
  .tm {
    color: #888;
    font-family: ui-monospace, monospace;
    font-size: 0.75rem;
  }
  .t {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lk {
    all: unset;
    color: #3b6fd8;
    font-size: 0.75rem;
    padding: 0 6px;
    cursor: pointer;
  }
  .lk:disabled {
    color: #aaa;
    cursor: default;
  }
  .state {
    color: #aaa;
    font-size: 0.75rem;
    padding: 0 6px;
  }
  .card {
    margin-top: 0.75rem;
    border: 1px solid #ddd;
    border-radius: 4px;
    padding: 0.5rem;
    font-size: 0.85rem;
  }
  .ab {
    color: #666;
    margin: 0.3rem 0;
  }
  .buttons {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .note {
    color: #888;
    font-size: 0.85rem;
  }
</style>
