<script lang="ts">
  import type { CommitOptions, Hunk, LfsLock, LineSelect, MetaIssue, Status } from "$lib/api";
  import { workingKey } from "$lib/keys";
  import { matchesLfsPattern } from "$lib/unity";
  import { fileOrder, stepFile, type FileRef } from "$lib/working";
  import CommitBox from "./CommitBox.svelte";
  import StatusList from "./StatusList.svelte";
  import WorkingFileDiff from "./WorkingFileDiff.svelte";

  let {
    repoId,
    status,
    revision,
    branch,
    busy = false,
    suspended = false,
    lastAction = null,
    metaIssues = [],
    locks = [],
    lfsPatterns = [],
    locksSupported = false,
    selected = $bindable(null),
    onstage,
    onunstage,
    ondiscard,
    onhunk,
    onlines,
    onhistory,
    onblame,
    onlock,
    onunlock,
    oncommit,
    ondraft,
    onstash,
  }: {
    repoId: number;
    status: Status;
    revision: number;
    branch: string;
    busy?: boolean;
    /** A dialog is open over the page: the single-key shortcuts stay off. */
    suspended?: boolean;
    lastAction?: string | null;
    metaIssues?: MetaIssue[];
    locks?: LfsLock[];
    lfsPatterns?: string[];
    locksSupported?: boolean;
    /** The file whose diff is shown. */
    selected?: FileRef | null;
    onstage: (paths: string[]) => void;
    onunstage: (paths: string[]) => void;
    ondiscard: (paths: string[]) => void;
    onhunk: (path: string, action: "stage" | "unstage" | "discard", hunk: Hunk) => void;
    onlines?: (path: string, action: "stage" | "unstage" | "discard", select: LineSelect) => void;
    onhistory?: (path: string) => void;
    onblame?: (path: string) => void;
    onlock?: (path: string) => void;
    onunlock?: (path: string, force: boolean) => void;
    oncommit: (message: string, options?: CommitOptions) => Promise<void>;
    ondraft?: () => Promise<string | null>;
    onstash?: () => void;
  } = $props();

  let pane = $state<HTMLElement | null>(null);
  let diffView = $state<ReturnType<typeof WorkingFileDiff> | null>(null);
  let commitBox = $state<ReturnType<typeof CommitBox> | null>(null);

  const stagedCount = $derived(status.entries.filter((e) => e.index).length);

  /** Whether the selected file's hunks can be staged one at a time. */
  const perHunk = $derived.by(() => {
    const entry = status.entries.find((e) => e.path === selected?.path);
    if (!entry || entry.kind !== "tracked") {
      return false;
    }
    return selected?.side === "staged" ? entry.index === "modified" : entry.worktree === "modified";
  });

  /** Moves keyboard focus here (Ctrl+1), so the working-copy keys apply. */
  export function focus() {
    pane?.focus();
  }

  function onKeydown(event: KeyboardEvent) {
    // Only with nothing focused or focus in this pane: the graph and sidebar have their own arrows.
    const target = event.target;
    if (suspended || (target instanceof Node && target !== document.body && !pane?.contains(target))) {
      return;
    }
    const action = workingKey(event);
    if (!action || (action === "toggle" && target instanceof HTMLButtonElement)) {
      return;
    }
    const order = fileOrder(status);
    const file = selected;
    let handled = true;
    switch (action) {
      case "down":
      case "up":
        selected = stepFile(order, file, action === "down" ? 1 : -1);
        break;
      case "toggle":
        if (file && !busy) {
          (file.side === "staged" ? onunstage : onstage)([file.path]);
        }
        break;
      case "discardFile":
        if (file?.side === "unstaged" && !busy) {
          ondiscard([file.path]);
        }
        break;
      case "stageAll": {
        const paths = status.entries.filter((e) => e.worktree || e.kind === "untracked").map((e) => e.path);
        if (paths.length && !busy) {
          onstage(paths);
        }
        break;
      }
      case "message":
        commitBox?.focus();
        break;
      default:
        handled =
          diffView?.hunkKey(
            ({ nextHunk: "next", prevHunk: "prev", stageHunk: "stage", unstageHunk: "unstage", discardHunk: "discard" } as const)[
              action
            ],
          ) ?? false;
    }
    if (handled) {
      event.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="wc" bind:this={pane} tabindex="-1" role="group" aria-label="Working copy pane">
  <div class="files">
    <StatusList
      {status}
      {busy}
      {selected}
      {lastAction}
      {metaIssues}
      {locks}
      {lfsPatterns}
      {locksSupported}
      {onstage}
      {onunstage}
      onselectfile={(path, side) => (selected = { path, side })}
      {ondiscard}
      {onlock}
      {onunlock}
    />
  </div>
  {#if selected}
    {@const file = selected}
    {@const suggestLock =
      locksSupported &&
      file.side === "unstaged" &&
      !!onlock &&
      lfsPatterns.some((pat) => matchesLfsPattern(file.path, pat)) &&
      !locks.some((l) => l.path === file.path)}
    <div class="diffbox">
      {#if suggestLock}
        <div class="lock-suggest" role="status">
          <span>This LFS asset isn't locked yet.</span>
          <button type="button" class="lsbtn" disabled={busy} onclick={() => onlock?.(file.path)}>Lock file</button>
        </div>
      {/if}
      <WorkingFileDiff
        bind:this={diffView}
        {repoId}
        path={file.path}
        side={file.side}
        {revision}
        {perHunk}
        {busy}
        onhunk={(action, hunk) => onhunk(file.path, action, hunk)}
        onlines={onlines ? (action, select) => onlines(file.path, action, select) : undefined}
        {onhistory}
        {onblame}
      />
    </div>
  {:else if status.entries.length}
    <p class="hint ab">
      ↑↓ files · Space stage / unstage · A stage all · Shift+⌫ discard · C message
      {#if onstash}
        <button class="stashbtn" disabled={busy} onclick={onstash}>Stash…</button>
      {/if}
    </p>
  {/if}
  <CommitBox bind:this={commitBox} {repoId} staged={stagedCount} {branch} {busy} {oncommit} {ondraft} />
</div>

<style>
  .wc {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .wc:focus {
    outline: none;
  }
  .files {
    flex: 0 1 auto;
    max-height: 45%;
    overflow-y: auto;
  }
  .files:has(+ :global(form)),
  .files:has(+ .hint) {
    max-height: none;
    flex: 1;
  }
  .diffbox {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .diffbox > :global(section) {
    flex: 1;
  }
  .diffbox :global(.diff) {
    flex: 1;
  }
  .lock-suggest {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 5px 10px;
    background: var(--warn);
    color: var(--warn-ink);
    border-bottom: 1px solid var(--line);
    font-size: 11.5px;
  }
  .lsbtn {
    height: 20px;
    padding: 0 8px;
    font-size: 11px;
  }
  .hint {
    margin: 0;
    padding: 8px 10px;
    border-top: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .stashbtn {
    margin-left: auto;
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
  }
  /* Ultrawide (R1, decided): file lists and commit box | diff. */
  @media (min-width: 2400px) {
    .wc {
      display: grid;
      grid-template-columns: minmax(320px, 2fr) 3fr;
      grid-template-rows: minmax(0, 1fr) auto;
    }
    .files {
      grid-area: 1 / 1;
      max-height: none;
    }
    .hint {
      grid-area: 1 / 2;
    }
    .diffbox {
      grid-area: 1 / 2 / 3 / 3;
      border-left: 1px solid var(--line);
    }
    .wc > :global(form) {
      grid-area: 2 / 1;
    }
  }
</style>
