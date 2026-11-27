<script lang="ts">
  import type { CommitOptions, Hunk, LfsLock, LineSelect, MetaIssue, Overlap, Side, Status } from "$lib/api";
  import { workingKey } from "$lib/keys";
  import { prefs, setPaneSize } from "$lib/prefs.svelte";
  import { matchesLfsPattern } from "$lib/unity";
  import {
    applyFileClick,
    extendFileSelection,
    fileKey,
    fileOrder,
    selectedPathsForSide,
    singleFileSelection,
    stepFile,
    type FileRef,
    type FileSelection,
  } from "$lib/working";
  import CommitBox from "./CommitBox.svelte";
  import Splitter from "./Splitter.svelte";
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
    overlap = null,
    pullAfterCommit = false,
    onpull,
    oncommitpull,
    onmerge,
    onlaunchfail,
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
    /** Files also changed on the upstream (R11 overlap radar). */
    overlap?: Overlap | null;
    /** "Commit, then pull" is armed: the next commit pulls. */
    pullAfterCommit?: boolean;
    onpull?: () => void;
    /** Arms a pull after the next commit and focuses the message. */
    oncommitpull?: () => void;
    /** Opens the merge dialog for the main branch, e.g. `origin/main`. */
    onmerge?: (branch: string) => void;
    /** A file menu's Explorer / terminal / editor action failed. */
    onlaunchfail?: (message: string) => void;
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
  let filesEl = $state<HTMLElement | null>(null);
  let dragStartFilesH = 200;

  function measuredFilesH(): number {
    return prefs.panes.workingFilesHeight ?? (Math.round(filesEl?.getBoundingClientRect().height || 0) || 200);
  }

  function clampFilesHeight(h: number): number {
    const maxH = Math.max(120, (pane?.clientHeight || 600) - 180);
    return Math.max(96, Math.min(maxH, Math.round(h)));
  }
  let diffView = $state<ReturnType<typeof WorkingFileDiff> | null>(null);
  let commitBox = $state<ReturnType<typeof CommitBox> | null>(null);

  let anchor = $state<FileRef | null>(null);
  let multiSide = $state<Side | null>(null);
  let multiKeys = $state<Set<string>>(new Set());

  const selectedKeys = $derived.by(() => {
    const order = fileOrder(status);
    const valid = new Set(order.map((f) => fileKey(f.path, f.side)));
    if (!selected || !valid.has(fileKey(selected.path, selected.side))) {
      return new Set<string>();
    }
    const out = new Set<string>();
    if (multiSide === selected.side) {
      for (const k of multiKeys) {
        if (valid.has(k)) {
          out.add(k);
        }
      }
    }
    out.add(fileKey(selected.path, selected.side));
    return out;
  });

  function applySelection(next: FileSelection) {
    selected = next.active;
    anchor = next.anchor;
    multiSide = next.active?.side ?? null;
    multiKeys = next.keys;
  }

  function currentSelection(): FileSelection {
    return { active: selected, anchor, keys: new Set(selectedKeys) };
  }

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

  /** Focuses the commit message ("Commit, then pull"). */
  export function focusCommit() {
    commitBox?.focus();
  }

  /** Overlaps worth pulling for: the ones git may not merge by itself. */
  const risky = $derived((overlap?.files ?? []).filter((f) => f.risk !== "merges" && !f.trunk));
  const riskyDirty = $derived(risky.some((f) => f.dirty));
  /** The same on the main branch: a pull won't bring these, a merge will. */
  const riskyTrunk = $derived((overlap?.files ?? []).filter((f) => f.risk !== "merges" && f.trunk));
  const plural = (n: number) => (n === 1 ? "file" : "files");

  function onKeydown(event: KeyboardEvent) {
    // Only with nothing focused or focus in this pane: the graph and sidebar have their own arrows.
    const target = event.target;
    if (suspended || (target instanceof Node && target !== document.body && !pane?.contains(target))) {
      return;
    }
    if (event.key === "Escape" && selectedKeys.size > 1 && selected) {
      event.preventDefault();
      applySelection(singleFileSelection(selected));
      return;
    }
    const action = workingKey(event);
    if (!action || (action === "toggle" && target instanceof HTMLButtonElement && !target.hasAttribute("data-file-row"))) {
      return;
    }
    const order = fileOrder(status);
    const file = selected;
    let handled = true;
    switch (action) {
      case "down":
      case "up":
        applySelection(singleFileSelection(stepFile(order, file, action === "down" ? 1 : -1)));
        break;
      case "extendDown":
      case "extendUp":
        applySelection(extendFileSelection(order, currentSelection(), action === "extendDown" ? 1 : -1));
        break;
      case "toggle":
        if (file && !busy) {
          const paths = selectedPathsForSide(order, selectedKeys, file.side);
          (file.side === "staged" ? onunstage : onstage)(paths.length ? paths : [file.path]);
        }
        break;
      case "discardFile":
        if (file?.side === "unstaged" && !busy) {
          const paths = selectedPathsForSide(order, selectedKeys, "unstaged");
          ondiscard(paths.length ? paths : [file.path]);
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
  <div
    class="files"
    bind:this={filesEl}
    style:height={selected && prefs.panes.workingFilesHeight != null ? `${prefs.panes.workingFilesHeight}px` : undefined}
    style:flex={selected && prefs.panes.workingFilesHeight != null ? "none" : undefined}
    style:max-height={selected && prefs.panes.workingFilesHeight != null ? "none" : undefined}
  >
    <StatusList
      {status}
      {busy}
      {selected}
      {selectedKeys}
      {lastAction}
      {metaIssues}
      {locks}
      {lfsPatterns}
      {locksSupported}
      {overlap}
      {onlaunchfail}
      {onstage}
      {onunstage}
      onselectfile={(path, side, mods) => applySelection(applyFileClick(fileOrder(status), currentSelection(), { path, side }, mods))}
      onclearselection={() => applySelection(singleFileSelection(null))}
      {ondiscard}
      {onlock}
      {onunlock}
    />
  </div>
  {#if selected}
    <Splitter
      orientation="horizontal"
      label="Resize file list"
      class="wc-split"
      onstart={() => (dragStartFilesH = measuredFilesH())}
      ondrag={(d) => setPaneSize("workingFilesHeight", clampFilesHeight(dragStartFilesH + d))}
      onstep={(d) => setPaneSize("workingFilesHeight", clampFilesHeight(measuredFilesH() + d))}
      onreset={() => setPaneSize("workingFilesHeight", null)}
    />
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
  {#if risky.length && overlap && onpull}
    <div class="overlap" role="status">
      <span>
        {#if pullAfterCommit}
          <b>Pulls after this commit</b> to merge {risky.length === 1 ? risky[0].path.slice(risky[0].path.lastIndexOf("/") + 1) : `${risky.length} files`}
          with {risky[0].on}.
        {:else}
          <b>{risky.length} {plural(risky.length)} changed on {risky[0].on} since you started.</b>
          Pull first, while the merge is small.
        {/if}
      </span>
      {#if !pullAfterCommit}
        {#if riskyDirty && oncommitpull}
          <button class="sm pri" disabled={busy} onclick={oncommitpull}>Commit, then pull</button>
        {:else}
          <button class="sm pri" disabled={busy} onclick={onpull}>Pull now</button>
        {/if}
      {/if}
    </div>
  {/if}
  {#if riskyTrunk.length && onmerge}
    {@const trunk = riskyTrunk[0].on}
    <div class="overlap" role="status" aria-label="Changed on {trunk}">
      <span>
        <b>{riskyTrunk.length} {plural(riskyTrunk.length)} changed on {trunk} this week.</b>
        Merge it in while the merge is small.
      </span>
      <button class="sm pri" disabled={busy} onclick={() => onmerge(trunk)}>Merge {trunk}…</button>
    </div>
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
  .diffbox > :global(.working-diff) {
    border-top: 0;
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
  .overlap {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 6px 10px 0;
    padding: 6px 9px;
    border-radius: 5px;
    background: var(--warn);
    color: var(--warn-ink);
    font-size: 12px;
  }
  .overlap b {
    color: var(--fg);
  }
  .overlap span {
    flex: 1;
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
    :global(.wc-split) {
      display: none;
    }
    .files {
      grid-area: 1 / 1;
      height: auto !important;
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
