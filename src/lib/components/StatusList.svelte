<script lang="ts">
  import type { Change, Entry, Head, Side, Status } from "$lib/api";

  let {
    status,
    busy = false,
    selected = null,
    onstage,
    onunstage,
    onselectfile,
    ondiscard,
  }: {
    status: Status;
    busy?: boolean;
    /** The file whose diff is shown, and on which side. */
    selected?: { path: string; side: Side } | null;
    onstage: (paths: string[]) => void;
    onunstage: (paths: string[]) => void;
    onselectfile?: (path: string, side: Side) => void;
    /** Throws away unstaged changes; undoable with Ctrl+Z. */
    ondiscard?: (paths: string[]) => void;
  } = $props();

  const LETTER: Record<Change, string> = { added: "A", modified: "M", deleted: "D", typeChange: "T" };

  const groups = $derived(
    [
      {
        title: "Conflicts",
        rows: status.entries.filter((e) => e.kind === "conflicted"),
        change: () => "!",
        verb: "Mark resolved",
        all: null,
        act: onstage,
        side: null,
      },
      {
        title: "Staged",
        rows: status.entries.filter((e) => e.index),
        change: (e: Entry) => LETTER[e.index!],
        verb: "Unstage",
        all: "Unstage all",
        act: onunstage,
        side: "staged" as Side,
      },
      {
        title: "Changes",
        rows: status.entries.filter((e) => e.worktree),
        change: (e: Entry) => LETTER[e.worktree!],
        verb: "Stage",
        all: "Stage all",
        act: onstage,
        side: "unstaged" as Side,
      },
      {
        title: "Untracked",
        rows: status.entries.filter((e) => e.kind === "untracked"),
        change: () => "?",
        verb: "Stage",
        all: "Stage all",
        act: onstage,
        side: "unstaged" as Side,
      },
    ].filter((g) => g.rows.length > 0),
  );

  function headText(head: Head) {
    switch (head.kind) {
      case "branch":
        return head.name;
      case "unborn":
        return `${head.name} (no commits yet)`;
      case "detached":
        return `detached at ${head.id}`;
    }
  }
</script>

<section aria-label="Working copy">
  <p class="head" data-testid="head" class:detached={status.head.kind === "detached"}>{headText(status.head)}</p>
  <p class="root">{status.root}</p>

  {#if groups.length === 0}
    <p class="clean">Nothing to commit, working tree clean.</p>
  {/if}

  {#each groups as group (group.title)}
    <h2>
      {group.title} <span class="count">{group.rows.length}</span>
      {#if group.all}
        <button
          class="all"
          disabled={busy}
          aria-label="{group.all} ({group.title})"
          onclick={() => group.act(group.rows.map((e) => e.path))}>{group.all}</button
        >
      {/if}
    </h2>
    <ul aria-label={group.title}>
      {#each group.rows as entry (entry.path)}
        <li>
          <span class="badge">{group.change(entry)}</span>
          {#if group.side && onselectfile}
            {@const side = group.side}
            <button
              class="path"
              class:current={selected?.path === entry.path && selected.side === side}
              aria-pressed={selected?.path === entry.path && selected.side === side}
              onclick={() => onselectfile(entry.path, side)}>{entry.path}</button
            >
          {:else}
            <span class="path">{entry.path}</span>
          {/if}
          {#if group.side === "unstaged" && ondiscard}
            <button class="act" disabled={busy} aria-label="Discard {entry.path}" onclick={() => ondiscard([entry.path])}
              >Discard</button
            >
          {/if}
          <button class="act" disabled={busy} aria-label="{group.verb} {entry.path}" onclick={() => group.act([entry.path])}
            >{group.verb}</button
          >
        </li>
      {/each}
    </ul>
  {/each}
</section>

<style>
  .head {
    font-weight: 600;
    margin: 0;
  }
  .detached {
    color: #b3261e;
  }
  .root {
    color: #666;
    font-size: 0.85em;
    margin: 0 0 1rem;
  }
  h2 {
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin: 1rem 0 0.25rem;
  }
  .count {
    color: #888;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    font-family: ui-monospace, monospace;
    font-size: 0.85rem;
  }
  li {
    line-height: 24px;
    display: flex;
    align-items: center;
  }
  li:hover {
    background: #f4f4f4;
  }
  .badge {
    display: inline-block;
    width: 1.5em;
    flex: none;
    color: #888;
  }
  .path {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button.path {
    all: unset;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }
  button.path.current {
    background: #e3ecfd;
  }
  button.path:focus-visible {
    outline: 2px solid #4c8bf5;
  }
  button {
    font: inherit;
    font-family: system-ui, sans-serif;
    font-size: 0.75rem;
    text-transform: none;
    letter-spacing: 0;
    padding: 0 0.4rem;
  }
  /* Hidden until hover, but still focusable and announced: opacity, not visibility. */
  .act {
    opacity: 0;
  }
  li:hover .act,
  .act:focus-visible {
    opacity: 1;
  }
  .all {
    margin-left: 0.5rem;
  }
</style>
