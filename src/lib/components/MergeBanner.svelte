<script lang="ts">
  let {
    theirs,
    branch,
    left,
    total,
    busy = false,
    oncommit,
    onabort,
  }: {
    /** The branch being merged in; `null` when the merge was started outside gitgud. */
    theirs: string | null;
    branch: string;
    left: number;
    total: number;
    busy?: boolean;
    oncommit: () => void;
    onabort: () => void;
  } = $props();

  const done = $derived(left === 0);
</script>

<div class="banner" class:done role="region" aria-label="Merge in progress">
  <span class="ic">⇄</span>
  <span class="what">
    Merging <b>{theirs ?? "another branch"}</b> into <b>{branch}</b>
    ·
    {#if done}
      {total ? `all ${total} ${total === 1 ? "file" : "files"} resolved` : "ready to commit"}
    {:else}
      {left} of {total} {total === 1 ? "file" : "files"} left
    {/if}
  </span>
  <span class="sp"></span>
  <button class:pri={done} disabled={!done || busy} title={done ? "Ctrl+Enter in the message" : "Resolve every file first"} onclick={oncommit}>Commit merge</button>
  <button class="bad" disabled={busy} title="Undoable with Ctrl+Z" onclick={onabort}>Abort, back to before</button>
</div>

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 12px;
    background: var(--warn);
    color: var(--warn-ink);
    border-bottom: 1px solid var(--line);
    font-weight: 500;
    white-space: nowrap;
  }
  .banner.done {
    background: var(--add);
    color: var(--add-ink);
  }
  .what {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sp {
    flex: 1;
  }
  button {
    height: 22px;
  }
</style>
