<script lang="ts">
  import { onMount, tick } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import {
    closeRepo,
    commit,
    gitInfo,
    launchPath,
    onRepoChanged,
    openRepo,
    pickFolder,
    cancelFetch,
    checkout,
    discard,
    discardHunk,
    fetchRemotes,
    merge,
    mergeAbort,
    mergeCommit,
    mergeState,
    rebase,
    rebaseState,
    rebaseStep,
    onFetchProgress,
    opLog,
    redo,
    refreshRepo,
    resolveSide,
    resolveWith,
    restoreFiles,
    restoreTo,
    repoStatus,
    setWindowTitle,
    undo,
    hunkHeader,
    stage,
    stageHunk,
    unstage,
    unstageHunk,
    commitDetail,
    historyPlan,
    previewRows,
    quickEdit,
    rewordHead,
    runPlan,
    type HistoryPlan,
    type PlanAction,
    type PlanPrediction,
    type Branches,
    type Hunk,
    type MergeSide,
    type MergeState,
    type OpEntry,
    type Opened,
    type Progress,
    type RebaseState,
    type RebaseStep,
    type Row,
    type Side,
    type Status,
  } from "$lib/api";
  import CommitDetail from "$lib/components/CommitDetail.svelte";
  import ConflictView from "$lib/components/ConflictView.svelte";
  import FindBar from "$lib/components/FindBar.svelte";
  import Graph from "$lib/components/Graph.svelte";
  import MergeBanner from "$lib/components/MergeBanner.svelte";
  import MergeDialog from "$lib/components/MergeDialog.svelte";
  import RebaseBanner from "$lib/components/RebaseBanner.svelte";
  import RebaseDialog from "$lib/components/RebaseDialog.svelte";
  import WorkingCopy from "$lib/components/WorkingCopy.svelte";
  import OpLogPanel from "$lib/components/OpLogPanel.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import ContextMenu, { type MenuItem } from "$lib/components/ContextMenu.svelte";
  import PlanDone, { type PlanResult } from "$lib/components/PlanDone.svelte";
  import PlanEditor from "$lib/components/PlanEditor.svelte";
  import { describe, steps, type PlanRow } from "$lib/plan/plan";
  import { inTextField, isFetchKey, isFindKey, isSplitKey, opLogKey, paneKey } from "$lib/keys";
  import { toggleSplit } from "$lib/prefs.svelte";
  import { mergeMessage } from "$lib/merge/resolve";
  import { middleTruncate } from "$lib/text";
  import { age } from "$lib/time";

  const git = gitInfo();

  let opened = $state<Opened | null>(null);
  let status = $state<Status | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  /** The commit shown in the side pane; `null` shows the working copy. */
  let selectedCommit = $state<string | null>(null);
  let selectedRow = $state<number | null>(null);

  function closeDetail() {
    selectedCommit = null;
    selectedRow = null;
  }

  /** The working-copy file whose diff is shown. */
  let selectedFile = $state<{ path: string; side: Side } | null>(null);
  /** Bumped on every status read, so the shown diff reloads with it. */
  let revision = $state(0);

  /** The merge in progress; while set, the conflict view replaces graph and inspector. */
  let merging = $state<MergeState | null>(null);
  /** How each conflicted file was resolved in this session, e.g. "use both". */
  const resolvedHow = new SvelteMap<string, string>();
  /** The merge message once edited; until then it follows the picks. */
  let messageEdit = $state<string | null>(null);
  const mergeDraft = $derived(merging ? (messageEdit ?? mergeMessage(merging.message, merging.conflicts, resolvedHow)) : "");
  const mergeLeft = $derived(merging?.conflicts.filter((c) => !c.resolved).length ?? 0);
  /** The branch the merge dialog asks about. */
  let mergeAsk = $state<string | null>(null);
  /** The rebase stopped on conflicts; shown through the same conflict view. */
  let rebasing = $state<RebaseState | null>(null);
  /** The branch the rebase dialog would rebase onto. */
  let rebaseAsk = $state<string | null>(null);
  /** A merge or a rebase stopped on conflicts: the conflict view replaces graph and inspector. */
  const conflicted = $derived<MergeState | null>(
    merging ??
      (rebasing && !rebasing.editing
        ? {
            theirs: rebasing.subject ? `"${rebasing.subject}"` : "this commit",
            message: "",
            conflicts: rebasing.conflicts,
          }
        : null),
  );

  /** The history plan being edited (R2-A); the inspector shows it and the graph its preview. */
  let planning = $state<{ plan: HistoryPlan; preset?: { id: string; action: PlanAction } } | null>(null);
  /** The plan's graph preview: its row count, and a version bumped on every new preview. */
  let preview = $state<{ rows: number; version: number } | null>(null);
  /** The Done panel after a plan finished (R2 state 3). */
  let planDone = $state<PlanResult | null>(null);
  /** The tip row being reworded inline (R on the tip). */
  let renaming = $state<{ index: number; id: string } | null>(null);
  let rowMenu = $state<{ row: Row; x: number; y: number } | null>(null);
  const dialogOpen = $derived(!!mergeAsk || !!rebaseAsk || !!rowMenu);

  const branchName = $derived(
    !status ? "" : status.head.kind === "detached" ? status.head.id.slice(0, 7) : status.head.name,
  );
  /** Filled by the sidebar; the toolbar shows the current branch's ahead / behind from it. */
  let branchData = $state<Branches | null>(null);
  const currentBranch = $derived(branchData?.local.find((b) => b.current) ?? null);

  /** The newest action Ctrl+Z would undo, named on the toolbar's Undo. */
  let lastAction = $state<string | null>(null);
  /** What Ctrl+Y would redo, for the Redo button's tooltip. */
  let nextRedo = $state<string | null>(null);
  $effect(() => {
    if (!opened) {
      return;
    }
    const key = `${opened.id}:${revision}`;
    opLog(opened.id).then(
      (list) => {
        if (key === `${opened?.id}:${revision}`) {
          lastAction = list.find((e) => !e.undone)?.action ?? null;
          // Undone entries on top of the log, newest first: redo replays the oldest of them.
          const firstDone = list.findIndex((e) => !e.undone);
          const undone = firstDone < 0 ? list : list.slice(0, firstDone);
          nextRedo = undone.at(-1)?.action ?? null;
        }
      },
      (err) => {
        console.error("op log for the Undo label:", err);
        lastAction = null;
      },
    );
  });

  $effect(() => {
    const title = opened ? `${repoName(opened.root)} · ${opened.root} — gitgud` : "gitgud";
    setWindowTitle(title).catch((err) => console.error("window title:", err));
  });

  /** The strip's resting line: where the branch stands against its upstream, and the last fetch. */
  const upstreamLine = $derived.by(() => {
    if (!currentBranch?.upstream) {
      return null;
    }
    const { ahead, behind, upstream } = currentBranch;
    const where = ahead || behind ? `↑${ahead} ↓${behind} against ${upstream}` : `Up to date with ${upstream}`;
    const fetched = branchData?.fetched;
    return `${where} · ${fetched ? `fetched ${age(fetched) === "now" ? "just now" : `${age(fetched)} ago`}` : "never fetched"}`;
  });

  function repoName(root: string) {
    const parts = root.split(/[\\/]/).filter(Boolean);
    return parts[parts.length - 1] ?? root;
  }

  let graph = $state<ReturnType<typeof Graph> | null>(null);
  /** The history find bar (Ctrl+F) and the rows it matched. */
  let finding = $state(false);
  let findBar = $state<ReturnType<typeof FindBar> | null>(null);
  let findHits = $state<ReadonlySet<number> | null>(null);
  let workingCopy = $state<ReturnType<typeof WorkingCopy> | null>(null);

  type RepoState = [Status, MergeState | null, RebaseState | null];

  /** Status, merge and rebase state, read together after every change. */
  function read(id: number): Promise<RepoState> {
    return Promise.all([repoStatus(id), mergeState(id), rebaseState(id)]);
  }

  /** Shows a (re)opened repo. A new or rebuilt graph drops the selection with its rows. */
  function show(next: Opened, [nextStatus, nextMerge, nextRebase]: RepoState) {
    if (next.id !== opened?.id || next.graphVersion !== opened.graphVersion) {
      closeDetail();
    }
    if (next.id !== opened?.id || (selectedFile && !hasSide(nextStatus, selectedFile.path, selectedFile.side))) {
      selectedFile = null;
    }
    const sameRebaseStep = !!nextRebase && nextRebase.step === rebasing?.step;
    if ((!nextMerge && !sameRebaseStep) || next.id !== opened?.id) {
      resolvedHow.clear();
      messageEdit = null;
    }
    if (nextMerge?.conflicts.some((c) => !c.resolved)) {
      messageEdit = null;
    }
    opened = next;
    status = nextStatus;
    merging = nextMerge;
    rebasing = nextRebase;
    revision++;
  }

  function hasSide(s: Status, path: string, side: Side) {
    const entry = s.entries.find((e) => e.path === path);
    return side === "staged" ? !!entry?.index : !!entry && (!!entry.worktree || entry.kind === "untracked");
  }

  function onHunk(path: string, action: "stage" | "unstage" | "discard", hunk: Hunk) {
    const header = hunkHeader(hunk);
    const run = { stage: stageHunk, unstage: unstageHunk, discard: discardHunk }[action];
    write((id) => run(id, path, header));
  }

  /** Writes, undos and redos run one after another, in the order asked for. */
  let queue: Promise<unknown> = Promise.resolve();
  let pending = $state(0);
  const busy = $derived(pending > 0);

  function enqueue<T>(job: () => Promise<T>): Promise<T> {
    pending++;
    const run = queue.then(job).finally(() => pending--);
    queue = run.catch(() => {});
    return run;
  }

  /** Runs a write, then shows the refreshed repo. Resolves to whether it worked. */
  function write(run: (id: number) => Promise<Opened>, done?: string): Promise<boolean> {
    return enqueue(async () => {
      if (!opened) {
        return false;
      }
      error = null;
      try {
        const next = await run(opened.id);
        show(next, await read(next.id));
        notice = done ?? null;
        return true;
      } catch (err) {
        error = String(err);
        return false;
      }
    });
  }

  /** Names the last undo or redo, e.g. "Undid: Stage a.txt". */
  let notice = $state<string | null>(null);

  function revert(kind: "undo" | "redo") {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        const result = kind === "undo" ? await undo(opened.id) : await redo(opened.id);
        planDone = null;
        show(result.opened, await read(result.opened.id));
        notice = `${kind === "undo" ? "Undid" : "Redid"}: ${result.action}`;
      } catch (err) {
        error = String(err);
      }
    });
  }

  /** The running fetch: `null` when idle, its latest progress line once git prints one. */
  let fetching = $state<{ progress: Progress | null } | null>(null);

  async function startFetch() {
    if (!opened || fetching) {
      return;
    }
    const id = opened.id;
    fetching = { progress: null };
    error = null;
    try {
      const next = await fetchRemotes(id);
      if (opened?.id === id) {
        show(next, await read(id));
        notice = "Fetched";
      }
    } catch (err) {
      if (String(err) === "cancelled") {
        notice = "Fetch cancelled";
      } else {
        error = String(err);
      }
    } finally {
      fetching = null;
    }
  }

  /** Which panel the side pane shows when no commit is selected. */
  let inspector = $state<"working" | "oplog">("working");

  function restoreToEntry(entry: OpEntry) {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        const result = await restoreTo(opened.id, entry.id);
        show(result.opened, await read(result.opened.id));
        notice = `Restored to before: ${result.action}`;
      } catch (err) {
        error = String(err);
      }
    });
  }

  async function startMerge(branch: string) {
    mergeAsk = null;
    const into = branchName;
    if ((await write((id) => merge(id, branch))) && !merging) {
      notice = `Merged ${branch} into ${into}`;
    }
  }

  /** Records `how` up front so the list and summary show it as soon as the file turns resolved. */
  async function resolveFile(path: string, how: string, run: (id: number) => Promise<Opened>) {
    const before = resolvedHow.get(path);
    resolvedHow.set(path, how);
    const ok = await write(run);
    if (!ok) {
      if (before === undefined) {
        resolvedHow.delete(path);
      } else {
        resolvedHow.set(path, before);
      }
    }
    return ok;
  }

  const resolveText = (path: string, content: string, how: string) =>
    resolveFile(path, how, (id) => resolveWith(id, path, content, how));

  const resolvePick = (path: string, side: MergeSide) =>
    resolveFile(path, side === "yours" ? "keep yours" : "take theirs", (id) => resolveSide(id, path, side));

  function commitMerge() {
    if (mergeLeft === 0 && mergeDraft.trim()) {
      write((id) => mergeCommit(id, mergeDraft), "Merge committed · Ctrl+Z undoes it");
    }
  }

  async function startRebase(onto: string) {
    rebaseAsk = null;
    const branch = branchName;
    if ((await write((id) => rebase(id, onto))) && !rebasing) {
      notice = `Rebased ${branch} onto ${onto} · Ctrl+Z undoes it`;
    }
  }

  /** Opens the history plan from commit `from` up to HEAD (Shift+E, R on an older commit). */
  async function openPlan(from: string, preset?: { id: string; action: PlanAction }) {
    if (!opened || busy) {
      return;
    }
    error = null;
    try {
      const plan = await historyPlan(opened.id, from);
      closeDetail();
      planDone = null;
      planning = { plan, preset };
    } catch (err) {
      error = String(err);
    }
  }

  function closePlan() {
    planning = null;
    preview = null;
    tick().then(() => graph?.focus());
  }

  async function startPlan(run: { onto: string; rows: PlanRow[]; updateRefs: boolean; prediction: PlanPrediction | null }) {
    if (!planning) {
      return;
    }
    const plan = planning.plan;
    const ok = await write((id) => runPlan(id, run.onto, plan.base, steps(run.rows), run.updateRefs));
    if (!ok) {
      return;
    }
    planning = null;
    preview = null;
    if (!rebasing) {
      planDone = {
        branch: plan.branch,
        time: Math.floor(Date.now() / 1000),
        before: run.rows.length,
        after: run.prediction?.after ?? run.rows.filter((r) => r.action !== "drop" && r.action !== "squash" && r.action !== "fixup").length,
        changed: run.prediction?.changed ?? null,
        summary: describe(run.rows),
        pushed: plan.pushed,
      };
      notice = `Rebased ${plan.branch} · Ctrl+Z undoes it`;
    }
  }

  const isTip = (row: Row) => row.labels.some((l) => l.kind === "branch" && l.current);

  /** R / F / D / Shift+E on a graph row (spec §3.4 single-commit rules). */
  function editRow(row: Row, edit: "reword" | "fixup" | "drop" | "plan") {
    if (busy || planning) {
      return;
    }
    if (edit === "plan") {
      openPlan(row.id);
    } else if (edit === "reword") {
      if (isTip(row) && selectedRow !== null) {
        renaming = { index: selectedRow, id: row.id };
      } else {
        openPlan(row.id, { id: row.id, action: "reword" });
      }
    } else {
      const done = edit === "fixup" ? `Folded "${row.subject}" into its parent` : `Dropped "${row.subject}"`;
      write((id) => quickEdit(id, row.id, edit), `${done} · Ctrl+Z undoes it`);
    }
  }

  async function renameTip(subject: string | null) {
    const target = renaming;
    renaming = null;
    if (!target || subject === null || !opened) {
      return;
    }
    if (!subject.trim()) {
      error = "A commit needs a subject";
      return;
    }
    try {
      const detail = await commitDetail(opened.id, target.id);
      if (detail.subject === subject) {
        return;
      }
      const message = detail.body ? `${subject}\n\n${detail.body}` : subject;
      await write((id) => rewordHead(id, message), "Reworded · Ctrl+Z undoes it");
    } catch (err) {
      error = String(err);
    }
    graph?.focus();
  }

  const menuItems = (row: Row): MenuItem[] => [
    { label: "Edit history from here…", hint: "Shift+E", disabled: busy, run: () => editRow(row, "plan") },
    { label: isTip(row) ? "Reword" : "Reword…", hint: "R", disabled: busy, run: () => editRow(row, "reword") },
    { label: "Fixup into parent", hint: "F", disabled: busy, run: () => editRow(row, "fixup") },
    { label: "Drop", hint: "D", disabled: busy, run: () => editRow(row, "drop") },
    { label: "Copy hash", hint: "Ctrl+C", run: () => copyHash(row) },
  ];

  async function stepRebase(step: RebaseStep) {
    const done = { continue: "Rebase finished", skip: "Rebase finished", abort: "Rebase aborted · Ctrl+Z brings it back" }[step];
    if ((await write((id) => rebaseStep(id, step))) && !rebasing) {
      notice = done;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (dialogOpen) {
      return;
    }
    const pane = paneKey(event);
    if (pane && opened) {
      event.preventDefault();
      if (pane === "graph") {
        graph?.focus();
        return;
      }
      inspector = pane;
      closeDetail();
      planDone = null;
      if (pane === "working") {
        tick().then(() => workingCopy?.focus());
      }
      return;
    }
    if (isFetchKey(event) && opened) {
      event.preventDefault();
      startFetch();
      return;
    }
    if (isSplitKey(event) && opened) {
      event.preventDefault();
      toggleSplit();
      return;
    }
    if (isFindKey(event) && opened && !conflicted) {
      event.preventDefault();
      if (finding) {
        findBar?.focus();
      }
      finding = true;
      return;
    }
    if (event.key === "F6" && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      cyclePanes(event.shiftKey ? -1 : 1);
      return;
    }
    if (event.key === "Escape" && selectedCommit && !inTextField(event.target)) {
      closeDetail();
      return;
    }
    if (event.key === "Escape" && planDone && !inTextField(event.target)) {
      planDone = null;
      return;
    }
    const kind = opLogKey(event);
    // While planning nothing changes until Start, so the op log stays put too.
    if (kind && opened && !planning) {
      event.preventDefault();
      revert(kind);
    }
  }

  /** F6 / Shift+F6: focus the next / previous pane (elements marked `data-pane`, in page order). */
  function cyclePanes(step: number) {
    const panes = [...document.querySelectorAll<HTMLElement>("[data-pane]")];
    if (panes.length === 0) {
      return;
    }
    const at = panes.findIndex((p) => p.contains(document.activeElement));
    const next = at < 0 ? panes[step > 0 ? 0 : panes.length - 1] : panes[(at + step + panes.length) % panes.length];
    const target =
      next.querySelector<HTMLElement>('[aria-current="true"], [role="listbox"], [tabindex="-1"]') ??
      next.querySelector<HTMLElement>("button:not(:disabled), textarea");
    target?.focus();
  }

  function copyHash(row: Row) {
    navigator.clipboard.writeText(row.id).then(
      () => (notice = `Copied ${row.shortId}`),
      (err) => (error = `Couldn't copy the hash: ${err}`),
    );
  }

  async function onCommit(message: string) {
    if (!(await write((id) => commit(id, message)))) {
      throw new Error("commit failed");
    }
  }

  let refreshing = false;
  let refreshAgain = false;

  /** Outside changes: one refresh at a time, and one more if changes arrived meanwhile. */
  async function onOutsideChange(id: number) {
    if (opened?.id !== id || busy) {
      return;
    }
    if (refreshing) {
      refreshAgain = true;
      return;
    }
    refreshing = true;
    try {
      do {
        refreshAgain = false;
        const next = await refreshRepo(id);
        const state = await read(id);
        if (opened?.id === id) {
          show(next, state);
        }
      } while (refreshAgain);
    } catch (err) {
      error = String(err);
    } finally {
      refreshing = false;
    }
  }

  onMount(() => {
    const stops: (() => void)[] = [];
    let destroyed = false;
    const subscriptions = [
      onRepoChanged(onOutsideChange),
      onFetchProgress((id, progress) => {
        if (fetching && opened?.id === id) {
          fetching = { progress };
        }
      }),
    ];
    for (const subscription of subscriptions) {
      subscription.then((unlisten) => (destroyed ? unlisten() : stops.push(unlisten)));
    }
    return () => {
      destroyed = true;
      stops.forEach((stop) => stop());
    };
  });

  async function load(path: string) {
    loading = true;
    error = null;
    try {
      const next = await openRepo(path);
      const state = await read(next.id);
      if (opened) {
        await closeRepo(opened.id);
      }
      show(next, state);
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  async function chooseRepo() {
    const path = await pickFolder();
    if (path) {
      await load(path);
    }
  }

  launchPath().then(
    (path) => {
      if (path) {
        load(path);
      }
    },
    (err) => (error = String(err)),
  );
</script>

<svelte:window onkeydown={onKeydown} />

<main class="app">
  <header class="tb">
    <button class="pill repo" aria-label="Open repository…" title={opened ? `${opened.root} · open another…` : "Open a repository"} onclick={chooseRepo}>
      <span aria-hidden="true">▣</span>
      <span class="nm">{opened ? repoName(opened.root) : "Open repository…"}</span>
      <span class="ab" aria-hidden="true">▾</span>
    </button>
    {#if opened && status}
      {#if rebasing}
        <span class="pill br" title="Rebasing {rebasing.branch} onto {rebasing.onto}">
          <span aria-hidden="true">⎇</span>
          <span class="nm">{middleTruncate(rebasing.branch, 44)}</span>
          <span class="ab">· rebasing</span>
        </span>
      {:else if status.head.kind === "detached"}
        <span class="pill warn" title="Commits made here belong to no branch">⚠ Detached HEAD at {branchName}</span>
      {:else}
        <span class="pill br" title="Current branch: {branchName}">
          <span aria-hidden="true">⎇</span>
          <span class="nm">{middleTruncate(branchName, 44)}</span>
          {#if currentBranch && (currentBranch.ahead || currentBranch.behind || currentBranch.upstream)}
            <span class="ab" title="Ahead / behind {currentBranch.upstream}">↑{currentBranch.ahead} ↓{currentBranch.behind}</span>
          {/if}
          {#if merging}<span class="ab">· merging</span>{/if}
        </span>
      {/if}
      <button disabled={!!fetching} title="Fetch all remotes (Ctrl+Shift+F)" onclick={startFetch}><span aria-hidden="true">↓</span> Fetch</button>
      <span class="sp"></span>
      <button class="pill undo" disabled={busy} title="Undo the last action (Ctrl+Z)" onclick={() => revert("undo")}>
        <span aria-hidden="true">↶</span> Undo{lastAction ? `: ${lastAction}` : ""}
        <span class="k" aria-hidden="true">Ctrl+Z</span>
      </button>
      <button class="pill" disabled={busy} title={nextRedo ? `Redo: ${nextRedo} (Ctrl+Y)` : "Redo (Ctrl+Y)"} aria-label="Redo" onclick={() => revert("redo")}>↷</button>
    {/if}
  </header>

  {#if error}
    <div class="errbar" role="alert">
      <span class="msg">{error}</span>
      <button class="bare x" aria-label="Dismiss" title="Dismiss" onclick={() => (error = null)}>✕</button>
    </div>
  {/if}

  {#if opened && status}
    {#if merging}
      <MergeBanner
        theirs={merging.theirs}
        branch={branchName}
        left={mergeLeft}
        total={merging.conflicts.length}
        {busy}
        oncommit={commitMerge}
        onabort={() => write((id) => mergeAbort(id), "Merge aborted · Ctrl+Z brings it back")}
      />
    {:else if planning}
      <div class="banner plan" role="region" aria-label="Planning">
        ✎ Planning a history edit of <b>{planning.plan.branch}</b> · nothing changes until Start
        <span class="sp"></span>
        <span class="ab">Ctrl+Z after Start undoes the whole rebase</span>
      </div>
    {:else if rebasing}
      <RebaseBanner
        rebase={rebasing}
        {busy}
        oncontinue={() => stepRebase("continue")}
        onskip={() => stepRebase("skip")}
        onabort={() => stepRebase("abort")}
      />
    {/if}
    <div class="body" class:merging={!!conflicted}>
      <div class="contents" data-pane="sidebar">
      <Sidebar
        repoId={opened.id}
        {revision}
        {busy}
        bind:data={branchData}
        merging={!!conflicted}
        oplogOpen={inspector === "oplog" && !selectedCommit}
        oncheckout={(branch) => write((id) => checkout(id, branch))}
        onmerge={(branch) => (mergeAsk = branch)}
        onrebase={(branch) => (rebaseAsk = branch)}
        onoplog={() => {
          inspector = inspector === "oplog" ? "working" : "oplog";
          closeDetail();
        }}
      />
      </div>
      {#snippet opLog(repoId: number)}
        <OpLogPanel
          {repoId}
          {revision}
          {busy}
          onundo={() => revert("undo")}
          onrestoreto={restoreToEntry}
          onrestorefiles={(entry) => write((id) => restoreFiles(id, entry.id))}
        />
      {/snippet}
      {#if conflicted}
        {@const repoId = opened.id}
        {#snippet continueRebase()}
          {#if rebasing}
            <section class="pane" aria-label="Step resolved">
              <div class="ph">Step {rebasing.step} of {rebasing.total} resolved</div>
              <div class="empty">
                <b>Every file of "{rebasing.subject}" is resolved</b>
                <span>Continue commits it and applies the next {rebasing.total - rebasing.step || "no"} commit{rebasing.total - rebasing.step === 1 ? "" : "s"}.</span>
                <span><button class="pri" disabled={busy} onclick={() => stepRebase("continue")}>Continue rebase</button></span>
              </div>
            </section>
          {/if}
        {/snippet}
        <ConflictView
          {repoId}
          merge={conflicted}
          {revision}
          branch={rebasing && !merging ? `${rebasing.onto} (rebased so far)` : branchName}
          how={resolvedHow}
          bind:message={() => mergeDraft, (v) => (messageEdit = v)}
          {busy}
          onresolvewith={resolveText}
          onresolveside={resolvePick}
          oncommit={commitMerge}
          children={inspector === "oplog" ? () => opLog(repoId) : undefined}
          whenResolved={merging ? undefined : continueRebase}
        />
      {:else}
        <section class="pane center" aria-label="History" data-pane="graph">
          <div class="ph">
            History <span class="ct">· all branches</span>
            {#if finding}
              <FindBar
                bind:this={findBar}
                repoId={opened.id}
                graphVersion={opened.graphVersion}
                bind:hits={findHits}
                onreveal={(row) => graph?.reveal(row)}
                onclose={() => {
                  finding = false;
                  graph?.focus();
                }}
              />
            {:else}
              <button class="bare findbtn r" title="Find commits (Ctrl+F)" onclick={() => (finding = true)}>⌕ Find <span class="k">Ctrl+F</span></button>
            {/if}
          </div>
          {#key `${opened.id}:${opened.graphVersion}`}
            {@const id = opened.id}
            <Graph
              bind:this={graph}
              repoId={id}
              rowCount={preview?.rows ?? opened.rows}
              rows={preview ? (start, count) => previewRows(id, start, count) : undefined}
              rowsVersion={preview?.version ?? 0}
              bind:selected={selectedRow}
              onselect={(row) => {
                if (!planning) {
                  selectedCommit = row && !row.wip ? row.id : null;
                  planDone = null;
                }
              }}
              oncopy={copyHash}
              hits={finding ? findHits : null}
              onedit={planning ? undefined : editRow}
              onmenu={planning ? undefined : (row, x, y) => (rowMenu = { row, x, y })}
              renaming={renaming?.index ?? null}
              onrename={renameTip}
            />
          {/key}
        </section>
        <aside class="pane inspector" data-pane="inspector">
          {#if planning}
            {#key planning}
              <PlanEditor
                repoId={opened.id}
                plan={planning.plan}
                preset={planning.preset}
                branches={branchData?.local.filter((b) => !b.current).map((b) => b.name) ?? []}
                {busy}
                onpreview={(rows) => (preview = { rows, version: (preview?.version ?? 0) + 1 })}
                onstart={startPlan}
                oncancel={closePlan}
              />
            {/key}
          {:else if selectedCommit}
            <CommitDetail repoId={opened.id} commit={selectedCommit} onclose={closeDetail} />
          {:else if planDone}
            {@const repoId = opened.id}
            <PlanDone done={planDone} {busy} onundo={() => revert("undo")}>
              {@render opLog(repoId)}
            </PlanDone>
          {:else if inspector === "oplog"}
            {@render opLog(opened.id)}
          {:else}
            <WorkingCopy
              bind:this={workingCopy}
              repoId={opened.id}
              {status}
              {revision}
              branch={branchName}
              {busy}
              suspended={dialogOpen}
              {lastAction}
              bind:selected={selectedFile}
              onstage={(paths) => write((id) => stage(id, paths))}
              onunstage={(paths) => write((id) => unstage(id, paths))}
              ondiscard={(paths) => write((id) => discard(id, paths))}
              onhunk={onHunk}
              oncommit={onCommit}
            />
          {/if}
        </aside>
      {/if}
    </div>
    {#if rowMenu}
      <ContextMenu x={rowMenu.x} y={rowMenu.y} items={menuItems(rowMenu.row)} onclose={() => (rowMenu = null)} />
    {/if}
    {#if mergeAsk}
      <MergeDialog
        repoId={opened.id}
        branch={mergeAsk}
        into={branchName}
        dirty={status.entries.length > 0}
        onmerge={() => mergeAsk && startMerge(mergeAsk)}
        oncancel={() => (mergeAsk = null)}
      />
    {/if}
    {#if rebaseAsk}
      <RebaseDialog
        repoId={opened.id}
        onto={rebaseAsk}
        branch={branchName}
        dirty={status.entries.length > 0}
        onrebase={() => rebaseAsk && startRebase(rebaseAsk)}
        oncancel={() => (rebaseAsk = null)}
      />
    {/if}
  {:else}
    <div class="start empty">
      {#if loading}
        <b>Opening…</b>
      {:else}
        <b>gitgud</b>
        <span>Open a repository to see its history and changes.</span>
        <span class="ab">Use the ▣ button above, or start with GITGUD_OPEN=&lt;path&gt;</span>
      {/if}
    </div>
  {/if}

  <footer class="strip">
    {#if fetching && opened}
      {@const id = opened.id}
      {@const pct = fetching.progress?.percent ?? 0}
      <span class="task" aria-label="Fetch progress">
        ↓ Fetching…
        {#if fetching.progress}
          {fetching.progress.phase} {fetching.progress.percent}%
        {/if}
        <span class="bar" aria-hidden="true"><i style:width="{pct}%"></i></span>
        <button class="bare link" onclick={() => cancelFetch(id)}>Cancel</button>
      </span>
    {:else if notice}
      <span role="status">{notice}</span>
    {:else if busy}
      <span>Working…</span>
    {:else if upstreamLine}
      <span class="muted">● {upstreamLine}</span>
    {:else if opened}
      <span class="muted">{opened.root}</span>
    {/if}
    <span class="sp"></span>
    {#await git}
      <span>Looking for git…</span>
    {:then info}
      <span class="git" data-testid="git-info" title={info.path}>git {info.version} <code>{info.path}</code></span>
    {:catch err}
      <span role="alert" class="bad">{err}</span>
    {/await}
  </footer>
</main>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .tb {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 40px;
    padding: 0 10px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    flex: none;
    min-width: 0;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--bg);
    white-space: nowrap;
    min-width: 0;
  }
  .pill .nm {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .repo {
    font-weight: 600;
    max-width: 220px;
  }
  .br {
    font-weight: 600;
    max-width: 420px;
    overflow: hidden;
  }
  .br .ab {
    font-weight: 400;
  }
  .warn {
    background: var(--warn);
    color: var(--warn-ink);
    border-color: transparent;
  }
  .undo {
    max-width: 380px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pill .k {
    font: 10.5px var(--mono);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 4px;
    background: var(--panel);
  }
  .sp {
    flex: 1;
  }
  .banner.plan {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: var(--sel);
    border-bottom: 1px solid var(--line);
    font-weight: 500;
    white-space: nowrap;
    flex: none;
  }
  .errbar {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 6px 12px;
    background: var(--bad);
    color: var(--bad-ink);
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .errbar .msg {
    flex: 1;
    white-space: pre-wrap;
    font-family: var(--mono);
    font-size: 11.5px;
    max-height: 6.5em;
    overflow-y: auto;
  }
  .x {
    all: unset;
    cursor: default;
    padding: 0 4px;
    border-radius: 3px;
  }
  .x:hover {
    background: color-mix(in srgb, currentColor 15%, transparent);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr) clamp(380px, 36vw, 560px);
  }
  .body.merging {
    grid-template-columns: 210px minmax(0, 1fr);
  }
  @media (min-width: 2400px) {
    .body:not(.merging) {
      grid-template-columns: 240px minmax(0, 1fr) min(1150px, 46vw);
    }
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--panel);
  }
  .contents {
    display: contents;
  }
  .findbtn {
    all: unset;
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
    cursor: default;
    padding: 0 4px;
    border-radius: 3px;
  }
  .findbtn:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .center {
    border-right: 1px solid var(--line);
  }
  .center > :global(.graph) {
    flex: 1;
  }
  .inspector {
    overflow: hidden;
  }
  .inspector > :global(.detail),
  .inspector > :global(.oplog) {
    flex: 1;
    overflow-y: auto;
  }
  .start {
    flex: 1;
  }
  .strip {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 24px;
    padding: 0 10px;
    flex: none;
    background: var(--panel);
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 11.5px;
    white-space: nowrap;
    overflow: hidden;
  }
  .strip [role="status"] {
    color: var(--fg);
  }
  .strip .muted {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .task {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--fg);
  }
  .bar {
    width: 120px;
    height: 5px;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--l0);
    transition: width 0.2s;
  }
  .link {
    all: unset;
    color: var(--l0);
    cursor: default;
  }
  .link:hover {
    text-decoration: underline;
  }
  .git {
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 40vw;
  }
  .git code {
    font-size: 10.5px;
  }
  .bad {
    color: var(--bad-ink);
  }
</style>
