<script lang="ts">
  import type { Change, Entry, Head, Side, Status } from "$lib/api";

  let {
    status,
    busy = false,
    selected = null,
    lastAction = null,
    onstage,
    onunstage,
    onselectfile,
    ondiscard,
  }: {
    status: Status;
    busy?: boolean;
    /** The file whose diff is shown, and on which side. */
    selected?: { path: string; side: Side } | null;
    /** What Ctrl+Z would undo, named on the clean state (R1 state 5). */
    lastAction?: string | null;
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

<section class="wc" aria-label="Working copy">
  <div class="ph" title={status.root}>
    Working copy <span class="ct">· <span data-testid="head" class:detached={status.head.kind === "detached"}>{headText(status.head)}</span></span>
    <span class="r">Ctrl+1</span>
  </div>

  {#if groups.length === 0}
    <div class="empty">
      <b>Nothing to commit, working tree clean.</b>
      <span>on {headText(status.head)}</span>
      {#if lastAction}
        <span class="ab">Last action: {lastAction} · Ctrl+Z undoes it</span>
      {/if}
    </div>
  {/if}

  {#each groups as group (group.title)}
    <h2 class="grp">
      {group.title} <span class="count">{group.rows.length}</span>
      {#if group.all}
        <button
          class="bare all"
          disabled={busy}
          aria-label="{group.all} ({group.title})"
          onclick={() => group.act(group.rows.map((e) => e.path))}>{group.all}</button
        >
      {/if}
    </h2>
    <ul aria-label={group.title}>
      {#each group.rows as entry (entry.path)}
        {@const letter = group.change(entry)}
        {@const cut = entry.path.lastIndexOf("/") + 1}
        <li>
          <span class="badge l-{letter === '?' ? 'q' : letter === '!' ? 'x' : letter}">{letter}</span>
          {#if group.side && onselectfile}
            {@const side = group.side}
            <button
              class="bare path"
              class:current={selected?.path === entry.path && selected.side === side}
              aria-pressed={selected?.path === entry.path && selected.side === side}
              title={entry.path}
              onclick={() => onselectfile(entry.path, side)}><span class="dir">{entry.path.slice(0, cut)}</span>{entry.path.slice(cut)}</button
            >
          {:else}
            <span class="path" title={entry.path}><span class="dir">{entry.path.slice(0, cut)}</span>{entry.path.slice(cut)}</span>
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
  .wc {
    display: flex;
    flex-direction: column;
  }
  .detached {
    color: var(--bad-ink);
    font-weight: 600;
  }
  h2 {
    margin: 0;
  }
  .grp:first-of-type {
    border-top: 0;
  }
  .count {
    font-weight: 400;
  }
  .all {
    all: unset;
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
    font: 500 11px var(--body);
    color: var(--l0);
    cursor: default;
  }
  .all:hover:not(:disabled) {
    text-decoration: underline;
  }
  .all:disabled {
    color: var(--muted);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 2px 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 6px 0 10px;
  }
  li:hover {
    background: var(--hover);
  }
  li:has(.current) {
    background: var(--sel);
  }
  .badge {
    all: unset;
    font: 600 10.5px var(--mono);
    width: 14px;
    text-align: center;
    flex: none;
    color: var(--muted);
  }
  .l-M,
  .l-T {
    color: var(--l4);
  }
  .l-A {
    color: var(--l2);
  }
  .l-D,
  .l-x {
    color: var(--bad-ink);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button.path {
    all: unset;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: default;
  }
  button.path:focus-visible {
    outline: 2px solid var(--l0);
    outline-offset: -2px;
  }
  .dir {
    color: var(--muted);
  }
  /* Hidden until hover, but still focusable and announced: opacity, not visibility. */
  .act {
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
    opacity: 0;
  }
  li:hover .act,
  .act:focus-visible {
    opacity: 1;
  }
</style>
