<script lang="ts">
  import type { Change, Entry, Head, LfsLock, MetaIssue, Side, Status } from "$lib/api";
  import { fileKey, fileOrder, selectedPathsForSide } from "$lib/working";

  let {
    status,
    busy = false,
    selected = null,
    selectedKeys = null,
    lastAction = null,
    metaIssues = [],
    locks = [],
    lfsPatterns = [],
    locksSupported = false,
    onstage,
    onunstage,
    onselectfile,
    onclearselection,
    ondiscard,
    onlock,
    onunlock,
  }: {
    status: Status;
    busy?: boolean;
    /** The file whose diff is shown, and on which side. */
    selected?: { path: string; side: Side } | null;
    /** All selected rows (`${side}:${path}`) when multi-selecting. */
    selectedKeys?: ReadonlySet<string> | null;
    /** What Ctrl+Z would undo, named on the clean state (R1 state 5). */
    lastAction?: string | null;
    metaIssues?: MetaIssue[];
    locks?: LfsLock[];
    lfsPatterns?: string[];
    locksSupported?: boolean;
    onstage: (paths: string[]) => void;
    onunstage: (paths: string[]) => void;
    onselectfile?: (path: string, side: Side, mods?: { ctrl: boolean; shift: boolean }) => void;
    onclearselection?: () => void;
    /** Throws away unstaged changes; undoable with Ctrl+Z. */
    ondiscard?: (paths: string[]) => void;
    onlock?: (path: string) => void;
    onunlock?: (path: string, force: boolean) => void;
  } = $props();

  const LETTER: Record<Change, string> = { added: "A", modified: "M", deleted: "D", typeChange: "T" };

  const lockByPath = $derived(new Map(locks.map((l) => [l.path, l])));

  function isRowSelected(path: string, side: Side | null): boolean {
    if (!side) {
      return false;
    }
    if (selectedKeys && selectedKeys.size > 0) {
      return selectedKeys.has(fileKey(path, side));
    }
    return selected?.path === path && selected.side === side;
  }

  const multiSide = $derived.by<Side | null>(() => {
    if (!selectedKeys || selectedKeys.size < 2) {
      return null;
    }
    const order = fileOrder(status);
    if (selectedPathsForSide(order, selectedKeys, "unstaged").length > 1) {
      return "unstaged";
    }
    if (selectedPathsForSide(order, selectedKeys, "staged").length > 1) {
      return "staged";
    }
    return null;
  });

  const multiPaths = $derived(multiSide && selectedKeys ? selectedPathsForSide(fileOrder(status), selectedKeys, multiSide) : []);

  function batchFor(path: string, side: Side | null): string[] {
    if (side && multiSide === side && multiPaths.length > 1 && selectedKeys?.has(fileKey(path, side))) {
      return multiPaths;
    }
    return [path];
  }

  function isLfsPath(path: string): boolean {
    if (lfsPatterns.length === 0) {
      return false;
    }
    const lower = path.toLowerCase();
    for (const pat of lfsPatterns) {
      const p = pat.toLowerCase();
      if (p.startsWith("*.")) {
        if (lower.endsWith(p.slice(1))) {
          return true;
        }
      } else if (lower === p || lower.endsWith(`/${p}`)) {
        return true;
      }
    }
    return false;
  }

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

  {#if metaIssues.length > 0}
    <div class="meta-warn" role="status" aria-label="Unity .meta warnings">
      {#each metaIssues as issue (issue.kind + issue.path)}
        <div class="meta-item">
          <span class="meta-tag">{issue.kind}</span>
          <span>{issue.detail}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if groups.length === 0}
    <div class="empty">
      <b>Nothing to commit, working tree clean.</b>
      <span>on {headText(status.head)}</span>
      {#if lastAction}
        <span class="ab">Last action: {lastAction} · Ctrl+Z undoes it</span>
      {/if}
    </div>
  {/if}

  {#if multiPaths.length > 1 && multiSide}
    <div class="selbar" role="region" aria-label="Selected files">
      <span>{multiPaths.length} selected</span>
      <span class="sel-acts">
        {#if multiSide === "unstaged"}
          <button type="button" class="pri sm" disabled={busy} onclick={() => onstage(multiPaths)}>
            Stage {multiPaths.length}
          </button>
          {#if ondiscard}
            <button type="button" class="sm" disabled={busy} onclick={() => ondiscard(multiPaths)}>
              Discard {multiPaths.length}
            </button>
          {/if}
        {:else}
          <button type="button" class="pri sm" disabled={busy} onclick={() => onunstage(multiPaths)}>
            Unstage {multiPaths.length}
          </button>
        {/if}
        {#if onclearselection}
          <button type="button" class="sm" disabled={busy} onclick={onclearselection}>Clear</button>
        {/if}
      </span>
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
        {@const lock = lockByPath.get(entry.path)}
        {@const lfs = isLfsPath(entry.path)}
        {@const rowSelected = isRowSelected(entry.path, group.side)}
        <li class:selected={rowSelected}>
          <span class="badge l-{letter === '?' ? 'q' : letter === '!' ? 'x' : letter}">{letter}</span>
          {#if group.side && onselectfile}
            {@const side = group.side}
            <button
              class="bare path"
              data-file-row
              class:current={selected?.path === entry.path && selected.side === side}
              aria-pressed={rowSelected}
              title={entry.path}
              onmousedown={(e) => {
                if (e.shiftKey) {
                  e.preventDefault();
                }
              }}
              onclick={(e) => {
                const ctrl = e.ctrlKey || e.metaKey;
                const shift = e.shiftKey;
                if (ctrl || shift) {
                  onselectfile(entry.path, side, { ctrl, shift });
                } else {
                  onselectfile(entry.path, side);
                }
              }}><span class="dir">{entry.path.slice(0, cut)}</span>{entry.path.slice(cut)}</button
            >
          {:else}
            <span class="path" title={entry.path}><span class="dir">{entry.path.slice(0, cut)}</span>{entry.path.slice(cut)}</span>
          {/if}
          {#if lfs}
            <span class="lfs-tag" title="Tracked by Git LFS">LFS</span>
          {/if}
          {#if lock}
            <span class="lock-tag" class:theirs={!lock.ours} title="Locked by {lock.owner} since {lock.lockedAt}"
              >🔒 {lock.ours ? "Locked (you)" : `Locked by ${lock.owner}`}</span
            >
          {/if}
          {#if locksSupported && (lock || lfs) && (onlock || onunlock)}
            {#if lock && onunlock}
              <button
                type="button"
                class="act"
                disabled={busy}
                aria-label="Unlock {entry.path}"
                onclick={() => onunlock(entry.path, !lock.ours)}>Unlock</button
              >
            {:else if !lock && onlock}
              <button
                type="button"
                class="act"
                disabled={busy}
                aria-label="Lock {entry.path}"
                onclick={() => onlock(entry.path)}>Lock</button
              >
            {/if}
          {/if}
          {#if group.side === "unstaged" && ondiscard}
            <button
              class="act"
              disabled={busy}
              aria-label="Discard {entry.path}"
              onclick={() => ondiscard(batchFor(entry.path, group.side))}>Discard</button
            >
          {/if}
          <button
            class="act"
            disabled={busy}
            aria-label="{group.verb} {entry.path}"
            onclick={() => group.act(batchFor(entry.path, group.side))}>{group.verb}</button
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
  .meta-warn {
    padding: 5px 10px;
    background: var(--warn);
    color: var(--warn-ink);
    border-bottom: 1px solid var(--line);
    display: grid;
    gap: 3px;
    font-size: 11px;
  }
  .meta-item {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .meta-tag {
    font: 600 9.5px var(--mono);
    text-transform: uppercase;
    padding: 0 4px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--warn-ink) 14%, transparent);
    flex: none;
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
    height: var(--row-h, 24px);
    padding: 0 6px 0 10px;
  }
  li:hover {
    background: var(--hover);
  }
  li.selected,
  li:has(.current) {
    background: var(--sel);
  }
  li.selected:has(.current) {
    box-shadow: inset 2px 0 0 var(--l0);
  }
  .selbar {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 10px;
    background: var(--sel);
    border-bottom: 1px solid var(--line);
    font-size: 11px;
  }
  .sel-acts {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sm {
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
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
  .lfs-tag {
    flex: none;
    font: 600 9.5px var(--mono);
    padding: 0 4px;
    border-radius: 3px;
    background: var(--hover);
    color: var(--muted);
  }
  .lock-tag {
    flex: none;
    font: 10.5px var(--body);
    padding: 0 5px;
    border-radius: 3px;
    background: var(--sel);
    color: var(--l0);
  }
  .lock-tag.theirs {
    background: var(--warn);
    color: var(--warn-ink);
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
