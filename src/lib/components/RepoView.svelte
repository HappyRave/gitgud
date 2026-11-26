<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import {
    backgroundRefresh,
    cancelTask,
    checkout,
    checkoutRemote,
    cherryPick,
    closeRepo,
    commit,
    commitDetail,
    createBranch,
    deleteBranch,
    deleteRemoteBranch,
    discard,
    discardHunk,
    discardLines,
    fetchRemotes,
    gitInfo,
    historyPlan,
    hunkHeader,
    lfsInfo,
    lfsLock,
    lfsLocks,
    lfsUnlock,
    merge,
    mergeAbort,
    mergeCommit,
    mergeState,
    onFetchProgress,
    onRepoChanged,
    onTaskProgress,
    openExternalUrl,
    openRepo,
    opLog,
    previewRows,
    prUrl,
    pull,
    push,
    quickEdit,
    rebase,
    rebaseState,
    rebaseStep,
    redo,
    refreshRepo,
    renameBranch,
    repoStatus,
    resetRepo,
    resolveSide,
    resolveWith,
    restoreFiles,
    restoreTo,
    revertCommit,
    rewordHead,
    runPlan,
    sequencerState,
    sequencerStep,
    setUpstream,
    setWindowTitle,
    stage,
    stageHunk,
    stageLines,
    stashApply,
    stashDrop,
    stashPop,
    stashSave,
    undo,
    unstage,
    unstageHunk,
    unstageLines,
    type Branches,
    type BranchStack,
    type CommitOptions,
    type HistoryPlan,
    type Hunk,
    type LfsInfo,
    type LfsLock,
    type LineSelect,
    type MergeSide,
    type MergeState,
    type MetaIssue,
    type Opened,
    type OpEntry,
    type OpLogStats,
    type PlanAction,
    type PlanPrediction,
    type Progress,
    type ProposalSummary,
    type PullStrategy,
    type RebaseState,
    type RebaseStep,
    type ReflogEntry,
    type ResetMode,
    type Row,
    type RowPick,
    type SequencerState,
    type SequencerStep,
    type Side,
    type Status,
    type TagInfo,
    type TaskOp,
    type UnityInfo,
    type WorktreeInfo,
  } from "$lib/api";
  import {
    aiDraftCommitMessage,
    branchStacks,
    createTag,
    deleteRemoteTag,
    deleteTag,
    hasCommitGraph,
    opLogPrune,
    opLogStats,
    outsideReflog,
    proposalApprove,
    proposalReject,
    proposalRequestChanges,
    proposalsList,
    proposeFromWorktree,
    pushTag,
    resolvePropertyMerge,
    restack,
    staleOriginCommits,
    tags,
    unityCheckOpenAssets,
    unityConfigureSmartMerge,
    unityConfirmVerify,
    unityInfo,
    unityMetaIssues,
    unityRemergeConflicts,
    worktreesList,
    writeCommitGraph,
    overlap as readOverlap,
    notify,
    graphFind,
    takeVersion,
    type ComparePoint,
    type Overlap,
    type OverlapFile,
  } from "$lib/api";
  import OverlapHeadsUp from "$lib/components/OverlapHeadsUp.svelte";
  import CompareView from "$lib/components/CompareView.svelte";
  import { launchItems } from "$lib/launch";
  import { rev as compareRev, WORKING_COPY } from "$lib/compare/tree";
  import { advice, freshHeadsUps, headline, headsUpKey, stripLabel } from "$lib/overlap/radar";
  import BranchDialog from "$lib/components/BranchDialog.svelte";
  import type { PaletteRepoActions } from "$lib/components/CommandPalette.svelte";
  import CherryPickDialog from "$lib/components/CherryPickDialog.svelte";
  import CommitDetail from "$lib/components/CommitDetail.svelte";
  import ConflictView from "$lib/components/ConflictView.svelte";
  import ContextMenu, { type MenuItem } from "$lib/components/ContextMenu.svelte";
  import DeleteBranchDialog from "$lib/components/DeleteBranchDialog.svelte";
  import Dialog from "$lib/components/Dialog.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import FileHistoryBlame from "$lib/components/FileHistoryBlame.svelte";
  import FindBar from "$lib/components/FindBar.svelte";
  import Graph from "$lib/components/Graph.svelte";
  import LoadingBar from "$lib/components/LoadingBar.svelte";
  import AppStatsBar from "$lib/components/AppStatsBar.svelte";
  import MergeBanner from "$lib/components/MergeBanner.svelte";
  import MergeDialog from "$lib/components/MergeDialog.svelte";
  import OpLogPanel from "$lib/components/OpLogPanel.svelte";
  import PlanDone, { type PlanResult } from "$lib/components/PlanDone.svelte";
  import PlanEditor from "$lib/components/PlanEditor.svelte";
  import ProposalView from "$lib/components/ProposalView.svelte";
  import PullStrategyDialog from "$lib/components/PullStrategyDialog.svelte";
  import PushDialog from "$lib/components/PushDialog.svelte";
  import RebaseBanner from "$lib/components/RebaseBanner.svelte";
  import RebaseDialog from "$lib/components/RebaseDialog.svelte";
  import ResetDialog from "$lib/components/ResetDialog.svelte";
  import TagDialog from "$lib/components/TagDialog.svelte";
  import LfsDownloadsDialog from "$lib/components/LfsDownloadsDialog.svelte";
  import SequencerBanner from "$lib/components/SequencerBanner.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Splitter from "$lib/components/Splitter.svelte";
  import StashDetail from "$lib/components/StashDetail.svelte";
  import StashDialog from "$lib/components/StashDialog.svelte";
  import WorkingCopy from "$lib/components/WorkingCopy.svelte";
  import {
    inTextField,
    isFetchKey,
    isFindKey,
    isNewBranchKey,
    isPullKey,
    isPushKey,
    isSplitKey,
    opLogKey,
    paneKey,
  } from "$lib/keys";
  import { mergeMessage } from "$lib/merge/resolve";
  import type { FixId } from "$lib/errors";
  import { describe, steps, type PlanRow } from "$lib/plan/plan";
  import { prefs, setPaneSize, toggleSplit } from "$lib/prefs.svelte";
  import { middleTruncate } from "$lib/text";
  import { age } from "$lib/time";

  let {
    path,
    active = true,
    suspended = false,
    git,
    onopenother,
    onstatuschange,
    onopenfail,
    onopensettings,
  }: {
    path: string;
    active?: boolean;
    suspended?: boolean;
    git: ReturnType<typeof gitInfo>;
    onopenother: () => void;
    onstatuschange?: (dirtyCount: number) => void;
    onopenfail?: (message: string) => void;
    onopensettings?: () => void;
  } = $props();

  let rootEl = $state<HTMLElement | null>(null);
  let bodyEl = $state<HTMLElement | null>(null);
  let inspEl = $state<HTMLElement | null>(null);
  let dragStartSidebar = 210;
  let dragStartInspector = 420;

  function measuredSidebarW(): number {
    return prefs.panes.sidebarWidth ?? (Math.round(bodyEl?.querySelector<HTMLElement>(".side")?.getBoundingClientRect().width || 0) || 210);
  }

  function measuredInspectorW(): number {
    return prefs.panes.inspectorWidth ?? (Math.round(inspEl?.getBoundingClientRect().width || 0) || 420);
  }

  function clampSidebar(w: number): number {
    const bodyW = bodyEl?.clientWidth || 1280;
    const inspW = conflicted ? 0 : measuredInspectorW();
    const maxW = Math.min(440, Math.max(150, bodyW - inspW - 260));
    return Math.max(150, Math.min(maxW, Math.round(w)));
  }

  function clampInspector(w: number): number {
    const bodyW = bodyEl?.clientWidth || 1280;
    const maxW = Math.max(300, bodyW - measuredSidebarW() - 260);
    return Math.max(300, Math.min(maxW, Math.round(w)));
  }
  let opened = $state<Opened | null>(null);
  let status = $state<Status | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  /** The commit shown in the side pane; `null` shows the working copy. */
  let selectedCommit = $state<string | null>(null);
  let selectedStash = $state<number | null>(null);
  let selectedProposal = $state<string | null>(null);
  let selectedRow = $state<number | null>(null);

  function closeDetail() {
    selectedCommit = null;
    selectedStash = null;
    selectedProposal = null;
    selectedRow = null;
    fileInspect = null;
  }

  /** The working-copy file whose diff is shown. */
  let selectedFile = $state<{ path: string; side: Side } | null>(null);
  /** Bumped on every status read, so the shown diff reloads with it. */
  let revision = $state(0);

  /** Files also changed on the upstream as last fetched (R11-B overlap radar). */
  let radar = $state<Overlap | null>(null);
  /** Won't-merge overlaps announced and not dismissed, for the heads-up. */
  let headsUp = $state<OverlapFile[]>([]);
  /** Heads-ups already given (`path@commit`), so each is told once. */
  let toldHeadsUps = new Set<string>();
  let radarRepo: number | null = null;
  /** "Commit, then pull": the next commit pulls. */
  let pullAfterCommit = $state(false);

  /** The open compare view (R12-B); `key` remounts it for a new pair. */
  let comparing = $state<{ a: ComparePoint; b: ComparePoint; key: number } | null>(null);
  let compareKey = 0;

  function openCompare(a: ComparePoint, b: ComparePoint) {
    rowMenu = null;
    comparing = { a, b, key: ++compareKey };
  }

  const rowPoint = (row: Row): ComparePoint => (row.wip ? WORKING_COPY : compareRev(row.id));

  function takeFromCompare(rev: string, path: string) {
    write((id) => takeVersion(id, rev, path), `Took ${path} from ${rev} · Ctrl+Z undoes it`, `Taking ${path} from ${rev}…`);
  }

  /** The checked-out branch, else HEAD: the other side of every "Compare with current". */
  const currentPoint = (): ComparePoint => compareRev(currentBranch?.name ?? "HEAD");

  /** The merge in progress; while set, the conflict view replaces graph and inspector. */
  let merging = $state<MergeState | null>(null);
  const resolvedHow = new SvelteMap<string, string>();
  let messageEdit = $state<string | null>(null);
  const mergeDraft = $derived(merging ? (messageEdit ?? mergeMessage(merging.message, merging.conflicts, resolvedHow)) : "");
  const mergeLeft = $derived(merging?.conflicts.filter((c) => !c.resolved).length ?? 0);
  /** What to merge: a branch name or a commit id, and how the dialog names it. */
  let mergeAsk = $state<{ rev: string; label: string } | null>(null);
  let rebasing = $state<RebaseState | null>(null);
  let rebaseAsk = $state<string | null>(null);
  let sequencer = $state<SequencerState | null>(null);
  const conflicted = $derived<MergeState | null>(
    merging ??
      (rebasing && !rebasing.editing
        ? {
            theirs: rebasing.subject ? `"${rebasing.subject}"` : "this commit",
            message: "",
            conflicts: rebasing.conflicts,
          }
        : sequencer
          ? {
              theirs: sequencer.shortId,
              message: sequencer.message,
              conflicts: sequencer.conflicts,
            }
          : null),
  );

  let planning = $state<{ plan: HistoryPlan; preset?: { id: string; action: PlanAction } } | null>(null);
  let preview = $state<{ rows: number; version: number } | null>(null);
  let planDone = $state<PlanResult | null>(null);
  let renaming = $state<{ index: number; id: string } | null>(null);
  let rowMenu = $state<{ row: Row; x: number; y: number } | null>(null);
  let pullAsk = $state<{ ahead: number; behind: number; upstream: string } | null>(null);
  let pushAsk = $state<{ initialForce: boolean } | null>(null);
  let branchAsk = $state<{
    mode: "create" | "rename" | "checkoutRemote";
    currentName?: string;
    at?: string | null;
    atLabel?: string | null;
    remoteBranch?: string | null;
  } | null>(null);
  let deleteAsk = $state<{ kind: "remote" | "forceLocal"; branch: string; remote?: string | null } | null>(null);
  let resetAsk = $state<{ id: string; shortId: string; subject: string } | null>(null);
  let sequencerAsk = $state<{ kind: "cherryPick" | "revert"; id: string; shortId: string; subject: string } | null>(null);
  let stashAsk = $state(false);
  let openAssetsAsk = $state<{ label: string; run: () => void } | null>(null);
  let unityState = $state<UnityInfo | null>(null);
  let metaIssues = $state<MetaIssue[]>([]);
  let lfs = $state<LfsInfo | null>(null);
  let locks = $state<LfsLock[]>([]);
  let tagList = $state<TagInfo[]>([]);
  let stackList = $state<BranchStack[]>([]);
  let proposalList = $state<ProposalSummary[]>([]);
  let worktreeList = $state<WorktreeInfo[]>([]);
  const linkedWorktrees = $derived(worktreeList.filter((w) => !w.isMain));
  let staleOriginIds = $state<ReadonlySet<string>>(new Set());
  let commitGraphPrompt = $state<number | null>(null);
  let checkedCommitGraphRepo = $state<number | null>(null);
  const activeProposals = $derived(
    proposalList.filter((p) => p.status === "pending" || p.status === "changesRequested"),
  );
  const currentProposal = $derived(
    selectedProposal ? (proposalList.find((p) => p.id === selectedProposal) ?? null) : null,
  );
  let tagAsk = $state<{ target: string | null; targetLabel: string | null } | null>(null);
  let lfsAsk = $state(false);
  let fileInspect = $state<{ path: string; mode: "history" | "blame"; rev: string | null } | null>(null);
  let opStats = $state<OpLogStats | null>(null);
  let reflogEntries = $state<ReflogEntry[]>([]);
  const dialogOpen = $derived(
    !!mergeAsk ||
      !!rebaseAsk ||
      !!rowMenu ||
      !!pullAsk ||
      !!pushAsk ||
      !!branchAsk ||
      !!deleteAsk ||
      !!resetAsk ||
      !!sequencerAsk ||
      !!tagAsk ||
      !!openAssetsAsk ||
      stashAsk,
  );

  const branchName = $derived(
    !status ? "" : status.head.kind === "detached" ? status.head.id.slice(0, 7) : status.head.name,
  );
  let branchData = $state<Branches | null>(null);
  const currentBranch = $derived(branchData?.local.find((b) => b.current) ?? null);

  let lastAction = $state<string | null>(null);
  let nextRedo = $state<string | null>(null);

  $effect(() => {
    if (!opened) {
      unityState = null;
      metaIssues = [];
      return;
    }
    const id = opened.id;
    const rev = revision;
    unityInfo(id)
      .then((u) => {
        if (opened?.id !== id || revision !== rev) {
          return;
        }
        unityState = u;
        if (u.isUnity) {
          unityMetaIssues(id)
            .then((m) => {
              if (opened?.id === id && revision === rev) {
                metaIssues = m;
              }
            })
            .catch(() => {});
        } else {
          metaIssues = [];
        }
      })
      .catch(() => {});
  });

  function refreshLocks(id: number) {
    lfsLocks(id)
      .then((list) => {
        if (opened?.id === id) {
          locks = list;
        }
      })
      .catch(() => {});
  }

  $effect(() => {
    if (!opened) {
      tagList = [];
      stackList = [];
      proposalList = [];
      worktreeList = [];
      selectedProposal = null;
      staleOriginIds = new Set();
      commitGraphPrompt = null;
      fileInspect = null;
      opStats = null;
      reflogEntries = [];
      return;
    }
    const id = opened.id;
    const rev = revision;
    let stale = false;
    tags(id)
      .then((list) => {
        if (!stale && opened?.id === id && revision === rev) {
          tagList = list;
        }
      })
      .catch(() => {});
    branchStacks(id)
      .then((list) => {
        if (!stale && opened?.id === id && revision === rev) {
          stackList = list;
        }
      })
      .catch(() => {});
    proposalsList(id)
      .then((list) => {
        if (!stale && opened?.id === id && revision === rev) {
          proposalList = list;
        }
      })
      .catch(() => {});
    worktreesList(id)
      .then((list) => {
        if (!stale && opened?.id === id && revision === rev) {
          worktreeList = list;
        }
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  });

  $effect(() => {
    if (!opened) {
      return;
    }
    const id = opened.id;
    if (opened.rows >= 500 && checkedCommitGraphRepo !== id) {
      checkedCommitGraphRepo = id;
      hasCommitGraph(id)
        .then((built) => {
          if (opened?.id === id && !built) {
            commitGraphPrompt = id;
          }
        })
        .catch(() => {});
    }
  });

  $effect(() => {
    const id = opened?.id;
    const pushed = planDone?.pushed;
    if (!id || !pushed) {
      staleOriginIds = new Set();
      return;
    }
    let stale = false;
    staleOriginCommits(id, pushed)
      .then((ids) => {
        if (!stale && opened?.id === id && planDone?.pushed === pushed) {
          staleOriginIds = new Set(ids);
        }
      })
      .catch(() => {
        if (!stale) {
          staleOriginIds = new Set();
        }
      });
    return () => {
      stale = true;
    };
  });

  $effect(() => {
    if (!opened || inspector !== "oplog") {
      return;
    }
    const id = opened.id;
    const rev = revision;
    let stale = false;
    opLogStats(id)
      .then((s) => {
        if (!stale && opened?.id === id && revision === rev) {
          opStats = s;
        }
      })
      .catch(() => {});
    outsideReflog(id, 50)
      .then((r) => {
        if (!stale && opened?.id === id && revision === rev) {
          reflogEntries = r;
        }
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  });

  $effect(() => {
    if (!opened) {
      lfs = null;
      locks = [];
      return;
    }
    const id = opened.id;
    lfsInfo(id)
      .then((info) => {
        if (opened?.id !== id) {
          return;
        }
        lfs = info;
        if (info.supportsLocks) {
          refreshLocks(id);
        } else {
          locks = [];
        }
      })
      .catch(() => {});
  });

  $effect(() => {
    if (!active || !opened) {
      return;
    }
    const id = opened.id;
    // Every 2 min for the active tab (R11 Q3, Max: "2 minutes"), so the overlap radar is fresh.
    const timer = setInterval(() => {
      if (active && opened?.id === id && !fetching && !conflicted && !busy) {
        backgroundRefresh(id, 110)
          .then((ok) => {
            if (ok && opened?.id === id) {
              onOutsideChange(id);
              if (lfs?.supportsLocks) {
                refreshLocks(id);
              }
            }
          })
          .catch(() => {});
      }
    }, 120_000);
    return () => clearInterval(timer);
  });

  // The overlap radar, read again after each status read (refresh, fetch, outside edit), debounced
  // so a Unity save storm costs one read.
  $effect(() => {
    if (!opened) {
      radar = null;
      headsUp = [];
      return;
    }
    const id = opened.id;
    void revision;
    if (radarRepo !== id) {
      radarRepo = id;
      toldHeadsUps = new Set();
      headsUp = [];
      radar = null;
    }
    const timer = setTimeout(() => {
      readOverlap(id).then(
        (next) => {
          if (opened?.id !== id) {
            return;
          }
          radar = next;
          const still = (f: OverlapFile) => next?.files.some((n) => headsUpKey(n) === headsUpKey(f)) ?? false;
          const fresh = freshHeadsUps(next, toldHeadsUps);
          for (const f of fresh) {
            toldHeadsUps.add(headsUpKey(f));
          }
          headsUp = [...fresh, ...headsUp.filter(still)];
          if (fresh.length && prefs.overlapNotify && !document.hasFocus()) {
            notify(`gitgud · ${repoName(opened.root)}: ${headline(fresh[0])}`, advice(fresh[0])).catch((err) =>
              console.error("overlap notification:", err),
            );
          }
        },
        (err) => console.error("overlap radar:", err),
      );
    }, 1500);
    return () => clearTimeout(timer);
  });

  function commitThenPull() {
    pullAfterCommit = true;
    headsUp = [];
    inspector = "working";
    closeDetail();
    planDone = null;
    tick().then(() => workingCopy?.focusCommit());
  }

  async function seeTheirChange(file: OverlapFile) {
    if (!opened) {
      return;
    }
    const id = opened.id;
    try {
      const rows = await graphFind(id, file.newestCommit);
      if (opened?.id === id && rows.length) {
        graph?.reveal(rows[0]);
      }
    } catch (err) {
      error = String(err);
    }
  }

  $effect(() => {
    if (!opened) {
      return;
    }
    const key = `${opened.id}:${revision}`;
    opLog(opened.id).then(
      (list) => {
        if (key === `${opened?.id}:${revision}`) {
          lastAction = list.find((e) => !e.undone)?.action ?? null;
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
    if (!active) {
      return;
    }
    const title = opened ? `${repoName(opened.root)} · ${opened.root} — gitgud` : "gitgud";
    setWindowTitle(title).catch((err) => console.error("window title:", err));
  });

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
  let finding = $state(false);
  let findBar = $state<ReturnType<typeof FindBar> | null>(null);
  let findHits = $state<ReadonlySet<number> | null>(null);
  let workingCopy = $state<ReturnType<typeof WorkingCopy> | null>(null);

  type RepoState = [Status, MergeState | null, RebaseState | null, SequencerState | null];

  function read(id: number): Promise<RepoState> {
    return Promise.all([repoStatus(id), mergeState(id), rebaseState(id), sequencerState(id)]);
  }

  function show(next: Opened, [nextStatus, nextMerge, nextRebase, nextSequencer]: RepoState) {
    if (next.id !== opened?.id || next.graphVersion !== opened.graphVersion) {
      closeDetail();
    }
    if (next.id !== opened?.id || (selectedFile && !hasSide(nextStatus, selectedFile.path, selectedFile.side))) {
      selectedFile = null;
    }
    const sameRebaseStep = !!nextRebase && nextRebase.step === rebasing?.step;
    const sameSequencer = !!nextSequencer && nextSequencer.commit === sequencer?.commit;
    if ((!nextMerge && !sameRebaseStep && !sameSequencer) || next.id !== opened?.id) {
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
    sequencer = nextSequencer;
    revision++;
    onstatuschange?.(nextStatus.entries.length);
  }

  function hasSide(s: Status, filePath: string, side: Side) {
    const entry = s.entries.find((e) => e.path === filePath);
    return side === "staged" ? !!entry?.index : !!entry && (!!entry.worktree || entry.kind === "untracked");
  }

  async function guardAssets(label: string, range: string, run: () => void) {
    if (!opened || !unityState?.isUnity || !unityState.editorOpen) {
      run();
      return;
    }
    try {
      if (await unityCheckOpenAssets(opened.id, range)) {
        openAssetsAsk = { label, run };
        return;
      }
    } catch {
      // Proceed if asset-touch check isn't applicable (e.g., unborn ref).
    }
    run();
  }

  function guardedCheckout(branch: string) {
    guardAssets(`Check out ${branch}`, `HEAD..${branch}`, () =>
      write((id) => checkout(id, branch), undefined, `Checking out ${branch}…`),
    );
  }

  async function onLockFile(filePath: string) {
    if (!opened || busy) {
      return;
    }
    error = null;
    try {
      await lfsLock(opened.id, filePath);
      notice = `Locked ${filePath}`;
      refreshLocks(opened.id);
    } catch (err) {
      error = String(err);
    }
  }

  async function onUnlockFile(filePath: string, force = false) {
    if (!opened || busy) {
      return;
    }
    error = null;
    try {
      await lfsUnlock(opened.id, filePath, force);
      notice = `Unlocked ${filePath}`;
      refreshLocks(opened.id);
    } catch (err) {
      error = String(err);
    }
  }

  async function setupAndRemergeUnity() {
    if (!opened || busy) {
      return;
    }
    const id = opened.id;
    error = null;
    try {
      unityState = await unityConfigureSmartMerge(id, null);
      await write((rid) => unityRemergeConflicts(rid), "Configured Smart Merge and re-merged Unity files", "Re-merging Unity files…");
    } catch (err) {
      error = String(err);
    }
  }

  function runRemergeUnity() {
    write((id) => unityRemergeConflicts(id), "Re-merged Unity files with UnityYAMLMerge", "Re-merging Unity files…");
  }

  function onConfirmVerify(filePath: string, fileId: string | null, property: string | null, confirmed: boolean) {
    write((id) => unityConfirmVerify(id, filePath, fileId, property, confirmed), undefined, "Saving check…");
  }

  const partLabel = { stage: "Staging", unstage: "Unstaging", discard: "Discarding" };

  const files = (paths: string[]) => (paths.length === 1 ? paths[0] : `${paths.length} files`);

  function stashWrite(kind: "apply" | "pop" | "drop", idx: number) {
    const run = { apply: stashApply, pop: stashPop, drop: stashDrop }[kind];
    const [doing, done] = { apply: ["Applying", "Applied"], pop: ["Popping", "Popped"], drop: ["Dropping", "Dropped"] }[kind];
    return write((id) => run(id, idx), `${done} stash@{${idx}} · Ctrl+Z undoes it`, `${doing} stash@{${idx}}…`);
  }

  function onHunk(filePath: string, action: "stage" | "unstage" | "discard", hunk: Hunk) {
    const header = hunkHeader(hunk);
    const run = { stage: stageHunk, unstage: unstageHunk, discard: discardHunk }[action];
    write((id) => run(id, filePath, header), undefined, `${partLabel[action]} hunk…`);
  }

  function onLines(filePath: string, action: "stage" | "unstage" | "discard", select: LineSelect) {
    const run = { stage: stageLines, unstage: unstageLines, discard: discardLines }[action];
    write((id) => run(id, filePath, select), undefined, `${partLabel[action]} lines…`);
  }

  function handleErrorFix(fix: FixId) {
    if (!opened) {
      return;
    }
    if (fix === "fetch") {
      startFetch();
    } else if (fix === "pull") {
      startPull("auto", false);
    } else if (fix === "stash") {
      stashAsk = true;
    } else if (fix === "settings") {
      onopensettings?.();
    } else if (fix === "forcePush") {
      pushAsk = { initialForce: true };
    } else if (fix === "abortMerge") {
      write((id) => mergeAbort(id), "Merge aborted · Ctrl+Z brings it back", "Aborting merge…");
    }
  }

  export function getPaletteContext(): { repoId: number; repoName: string; actions: PaletteRepoActions } | null {
    if (!opened || !status) {
      return null;
    }
    const name = repoName(opened.root);
    return {
      repoId: opened.id,
      repoName: name,
      actions: {
        repoName: name,
        branches: branchData?.local ?? [],
        canPull: !conflicted && !!currentBranch?.upstream,
        canPush: !conflicted && status.head.kind === "branch",
        canStash: !conflicted && status.entries.length > 0,
        canUndo: !!lastAction,
        canRedo: !!nextRedo,
        onfetch: startFetch,
        onpull: () => startPull("auto", false),
        onpush: () => (pushAsk = { initialForce: false }),
        onstash: () => (stashAsk = true),
        onundo: () => revert("undo"),
        onredo: () => revert("redo"),
        onnewbranch: () =>
          (branchAsk = {
            mode: "create",
            at: selectedCommit,
            atLabel: selectedCommit ? selectedCommit.slice(0, 7) : null,
          }),
        oncheckout: (b) => guardedCheckout(b),
        onmerge: (b) => (mergeAsk = { rev: b, label: b }),
        onrebase: (b) => (rebaseAsk = b),
        ondeletebranch: (b) => onDeleteBranch(b),
        onfind: () => {
          finding = true;
          tick().then(() => findBar?.focus());
        },
        onoplog: () => {
          inspector = inspector === "oplog" ? "working" : "oplog";
          closeDetail();
        },
        oncreatetag: () =>
          (tagAsk = {
            target: selectedCommit,
            targetLabel: selectedCommit ? selectedCommit.slice(0, 7) : null,
          }),
        onoptimizegraph: optimizeCommitGraph,
        onlfsdownloads: () => (lfsAsk = true),
        launch: launchItems(opened.root, null, (message) => (error = message)),
        oncompare: () =>
          openCompare(currentPoint(), currentBranch?.upstream ? compareRev(currentBranch.upstream) : WORKING_COPY),
      },
    };
  }

  let queue: Promise<unknown> = Promise.resolve();
  let pending = $state(0);
  const busy = $derived(pending > 0);
  let working = $state("Working…");

  function enqueue<T>(job: () => Promise<T>, label: string): Promise<T> {
    pending++;
    const run = queue
      .then(() => {
        working = label;
        return job();
      })
      .finally(() => pending--);
    queue = run.catch(() => {});
    return run;
  }

  function write(run: (id: number) => Promise<Opened>, done: string | undefined, label: string): Promise<boolean> {
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
    }, label);
  }

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
    }, kind === "undo" ? "Undoing…" : "Redoing…");
  }

  let fetching = $state<{ op: TaskOp; progress: Progress | null } | null>(null);

  async function startFetch() {
    if (!opened || fetching) {
      return;
    }
    const id = opened.id;
    fetching = { op: "fetch", progress: null };
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

  async function startPull(strategy: PullStrategy, remember: boolean) {
    if (!opened || fetching) {
      return;
    }
    pullAsk = null;
    const id = opened.id;
    fetching = { op: "pull", progress: null };
    error = null;
    try {
      const res = await pull(id, strategy, remember);
      if (opened?.id !== id) {
        return;
      }
      show(res.opened, await read(id));
      switch (res.outcome.kind) {
        case "upToDate":
          notice = "Already up to date";
          break;
        case "fastForwarded":
          notice = "Pulled (fast-forward) · Ctrl+Z undoes it";
          break;
        case "merged":
          notice = "Pulled (merged) · Ctrl+Z undoes it";
          break;
        case "rebased":
          notice = "Pulled (rebased) · Ctrl+Z undoes it";
          break;
        case "conflicts":
          notice = "Pull stopped on conflicts";
          break;
        case "diverged":
          pullAsk = { ahead: res.outcome.ahead, behind: res.outcome.behind, upstream: res.outcome.upstream };
          break;
      }
    } catch (err) {
      if (String(err) === "cancelled") {
        notice = "Pull cancelled";
      } else {
        error = String(err);
      }
    } finally {
      fetching = null;
    }
  }

  async function startPush(opts: {
    remote: string;
    branch: string;
    remoteBranch: string;
    setUpstream: boolean;
    forceWithLease: boolean;
    unlockPaths?: string[];
  }) {
    if (!opened || fetching) {
      return;
    }
    pushAsk = null;
    const id = opened.id;
    fetching = { op: "push", progress: null };
    error = null;
    try {
      const next = await push(id, opts.remote, opts.branch, opts.remoteBranch, opts.setUpstream, opts.forceWithLease);
      for (const path of opts.unlockPaths ?? []) {
        await lfsUnlock(id, path, false).catch(() => {});
      }
      if (opened?.id === id) {
        show(next, await read(id));
        if ((opts.unlockPaths?.length ?? 0) > 0 && lfs?.supportsLocks) {
          refreshLocks(id);
        }
        notice = opts.forceWithLease
          ? `Force-pushed ${opts.branch} to ${opts.remote}/${opts.remoteBranch}`
          : `Pushed ${opts.branch} to ${opts.remote}/${opts.remoteBranch}`;
      }
    } catch (err) {
      if (String(err) === "cancelled") {
        notice = "Push cancelled";
      } else {
        error = String(err);
      }
    } finally {
      fetching = null;
    }
  }

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
    }, "Restoring…");
  }

  async function startMerge(rev: string, label: string) {
    mergeAsk = null;
    const into = branchName;
    guardAssets(`Merge ${label} into ${into}`, `HEAD...${rev}`, async () => {
      if ((await write((id) => merge(id, rev), undefined, `Merging ${label} into ${into}…`)) && !merging) {
        notice = `Merged ${label} into ${into}`;
      }
    });
  }

  /** Merging a commit from the history into the checked-out branch: by its branches, else by id. */
  function mergeItems(row: Row): MenuItem[] {
    if (row.wip || row.stash != null || status?.head.kind !== "branch") {
      return [];
    }
    const disabled = busy || !!conflicted || !!merging || row.labels.some((l) => l.kind === "head" || (l.kind === "branch" && l.current));
    const into = branchName;
    const names = row.labels.flatMap((l) =>
      (l.kind === "branch" && !l.current) || l.kind === "remote" ? [l.name] : [],
    );
    if (names.length === 0) {
      return [
        {
          label: `Merge commit ${row.shortId} into ${into}…`,
          disabled,
          run: () => (mergeAsk = { rev: row.id, label: `commit ${row.shortId}` }),
        },
      ];
    }
    return names.map((name) => ({
      label: `Merge ${name} into ${into}…`,
      disabled,
      run: () => (mergeAsk = { rev: name, label: name }),
    }));
  }

  async function resolveFile(filePath: string, how: string, run: (id: number) => Promise<Opened>) {
    const before = resolvedHow.get(filePath);
    resolvedHow.set(filePath, how);
    const ok = await write(run, undefined, `Resolving ${filePath}…`);
    if (!ok) {
      if (before === undefined) {
        resolvedHow.delete(filePath);
      } else {
        resolvedHow.set(filePath, before);
      }
    }
    return ok;
  }

  const resolveText = (filePath: string, content: string, how: string) =>
    resolveFile(filePath, how, (id) => resolveWith(id, filePath, content, how));

  const resolvePick = (filePath: string, side: MergeSide) =>
    resolveFile(filePath, side === "yours" ? "keep yours" : "take theirs", (id) => resolveSide(id, filePath, side));

  const resolveUnity = (filePath: string, picks: RowPick[]) =>
    resolveFile(filePath, "merged by property", (id) => resolvePropertyMerge(id, filePath, picks));

  function commitMerge() {
    const unconfirmed =
      merging?.unityVerify?.some((f) => f.verify.some((v) => !v.confirmed)) ?? false;
    if (mergeLeft === 0 && !unconfirmed && mergeDraft.trim()) {
      write((id) => mergeCommit(id, mergeDraft), "Merge committed · Ctrl+Z undoes it", "Committing merge…");
    }
  }

  async function startRebase(onto: string) {
    rebaseAsk = null;
    const branch = branchName;
    guardAssets(`Rebase ${branch} onto ${onto}`, `${onto}...HEAD`, async () => {
      if ((await write((id) => rebase(id, onto), undefined, `Rebasing ${branch} onto ${onto}…`)) && !rebasing) {
        notice = `Rebased ${branch} onto ${onto} · Ctrl+Z undoes it`;
      }
    });
  }

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
    const ok = await write((id) => runPlan(id, run.onto, plan.base, steps(run.rows), run.updateRefs), undefined, "Rewriting history…");
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

  async function editRow(row: Row, edit: "reword" | "fixup" | "drop" | "plan") {
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
      const branch = currentBranch;
      const ok = await write(
        (id) => quickEdit(id, row.id, edit),
        `${done} · Ctrl+Z undoes it`,
        edit === "fixup" ? `Folding "${row.subject}" into its parent…` : `Dropping "${row.subject}"…`,
      );
      if (ok && branch?.upstream) {
        planDone = {
          branch: branch.name,
          time: Math.floor(Date.now() / 1000),
          before: 2,
          after: edit === "drop" ? 0 : 1,
          changed: null,
          summary: done,
          pushed: branch.upstream,
        };
      }
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
      const branch = currentBranch;
      const ok = await write((id) => rewordHead(id, message), "Reworded · Ctrl+Z undoes it", "Rewording…");
      if (ok && branch?.upstream) {
        planDone = {
          branch: branch.name,
          time: Math.floor(Date.now() / 1000),
          before: 1,
          after: 1,
          changed: null,
          summary: `Reworded "${detail.subject}" → "${subject}"`,
          pushed: branch.upstream,
        };
      }
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
    {
      label: "New branch here…",
      hint: "Ctrl+B",
      disabled: busy,
      run: () => (branchAsk = { mode: "create", at: row.id, atLabel: `${row.shortId} "${row.subject}"` }),
    },
    {
      label: "New tag here…",
      disabled: busy,
      run: () => (tagAsk = { target: row.id, targetLabel: `${row.shortId} "${row.subject}"` }),
    },
    ...mergeItems(row),
    {
      label: "Cherry-pick…",
      disabled: busy || !!conflicted,
      run: () => (sequencerAsk = { kind: "cherryPick", id: row.id, shortId: row.shortId, subject: row.subject }),
    },
    {
      label: "Revert commit…",
      disabled: busy || !!conflicted,
      run: () => (sequencerAsk = { kind: "revert", id: row.id, shortId: row.shortId, subject: row.subject }),
    },
    {
      label: `Reset ${branchName || "branch"} to here…`,
      disabled: busy || status?.head.kind !== "branch",
      run: () => (resetAsk = { id: row.id, shortId: row.shortId, subject: row.subject }),
    },
    {
      label: `Compare with ${currentBranch?.name ?? "HEAD"}`,
      hint: "Ctrl+click",
      run: () => openCompare(compareRev(row.id), currentPoint()),
    },
    { label: "Compare with working copy", run: () => openCompare(compareRev(row.id), WORKING_COPY) },
    { label: "Copy hash", hint: "Ctrl+C", run: () => copyHash(row) },
  ];

  async function stepRebase(step: RebaseStep) {
    const done = { continue: "Rebase finished", skip: "Rebase finished", abort: "Rebase aborted · Ctrl+Z brings it back" }[step];
    const label = { continue: "Continuing rebase…", skip: "Skipping commit…", abort: "Aborting rebase…" }[step];
    if ((await write((id) => rebaseStep(id, step), undefined, label)) && !rebasing) {
      notice = done;
    }
  }

  async function stepSequencer(step: SequencerStep) {
    const verb = sequencer?.kind === "revert" ? "Revert" : "Cherry-pick";
    const done = step === "continue" ? `${verb} finished · Ctrl+Z undoes it` : `${verb} aborted · Ctrl+Z brings it back`;
    const label = step === "continue" ? `Continuing ${verb.toLowerCase()}…` : `Aborting ${verb.toLowerCase()}…`;
    if ((await write((id) => sequencerStep(id, step), undefined, label)) && !sequencer) {
      notice = done;
    }
  }

  async function submitBranch(name: string, checkoutNew: boolean) {
    const ask = branchAsk;
    branchAsk = null;
    if (!ask) {
      return;
    }
    if (ask.mode === "create") {
      await write(
        (id) => createBranch(id, name, ask.at ?? null, checkoutNew),
        `Created ${name} · Ctrl+Z undoes it`,
        `Creating ${name}…`,
      );
    } else if (ask.mode === "rename" && ask.currentName) {
      await write(
        (id) => renameBranch(id, ask.currentName!, name),
        `Renamed ${ask.currentName} to ${name} · Ctrl+Z undoes it`,
        `Renaming ${ask.currentName}…`,
      );
    } else if (ask.mode === "checkoutRemote" && ask.remoteBranch) {
      await write(
        (id) => checkoutRemote(id, ask.remoteBranch!, name),
        `Checked out ${name} · Ctrl+Z undoes it`,
        `Checking out ${name}…`,
      );
    }
  }

  async function submitTag(name: string, message: string | null) {
    const ask = tagAsk;
    tagAsk = null;
    if (!ask) {
      return;
    }
    await write((id) => createTag(id, name, ask.target, message), `Created tag ${name}`, `Creating tag ${name}…`);
  }

  function onDeleteTag(name: string) {
    return write((id) => deleteTag(id, name), `Deleted tag ${name}`, `Deleting tag ${name}…`);
  }

  function onPushTag(remote: string, name: string) {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        await pushTag(opened.id, remote, name);
        notice = `Pushed tag ${name} to ${remote}`;
      } catch (err) {
        error = String(err);
      }
    }, `Pushing tag ${name} to ${remote}…`);
  }

  function onDeleteRemoteTag(remote: string, name: string) {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        await deleteRemoteTag(opened.id, remote, name);
        notice = `Deleted tag ${name} on ${remote}`;
      } catch (err) {
        error = String(err);
      }
    }, `Deleting tag ${name} on ${remote}…`);
  }

  function pruneOpLog() {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        opStats = await opLogPrune(opened.id);
        revision += 1;
        notice = "Pruned old op-log entries";
      } catch (err) {
        error = String(err);
      }
    }, "Pruning op log…");
  }

  function optimizeCommitGraph() {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        await writeCommitGraph(opened.id);
        notice = "Commit-graph cache updated";
      } catch (err) {
        error = String(err);
      }
    }, "Writing commit-graph cache…");
  }

  function inspectCommit(commitId: string) {
    fileInspect = null;
    selectedStash = null;
    selectedProposal = null;
    inspector = "working";
    selectedCommit = commitId;
  }

  function openFileInspect(path: string, mode: "history" | "blame", rev: string | null = null) {
    selectedStash = null;
    selectedProposal = null;
    fileInspect = { path, mode, rev };
  }

  async function approveProposal(proposalId: string, message: string, selectedPaths: string[]) {
    const ok = await write(
      (id) => proposalApprove(id, proposalId, selectedPaths, message),
      "Proposal approved and committed · Ctrl+Z undoes it",
      "Committing proposal…",
    );
    if (ok) {
      selectedProposal = null;
    }
  }

  function requestProposalChanges(proposalId: string, comment: string) {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        const updated = await proposalRequestChanges(opened.id, proposalId, comment);
        proposalList = proposalList.map((p) => (p.id === updated.id ? updated : p));
        notice = "Requested changes on proposal";
      } catch (err) {
        error = String(err);
      }
    }, "Requesting changes…");
  }

  async function rejectProposal(proposalId: string) {
    const ok = await write((id) => proposalReject(id, proposalId), "Proposal rejected", "Rejecting proposal…");
    if (ok) {
      selectedProposal = null;
    }
  }

  function runRestack(topBranch: string, onto: string) {
    return write(
      (id) => restack(id, topBranch, onto),
      `Restacked ${topBranch} onto ${onto} · Ctrl+Z undoes it`,
      `Restacking ${topBranch} onto ${onto}…`,
    );
  }

  function openStackPr(url: string) {
    openExternalUrl(url).catch((err) => (error = String(err)));
  }

  async function draftCommitMessage(): Promise<string | null> {
    if (!opened) {
      return null;
    }
    error = null;
    try {
      return await aiDraftCommitMessage(opened.id);
    } catch (err) {
      error = String(err);
      return null;
    }
  }

  function proposeWorktree(wt: WorktreeInfo) {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        const created = await proposeFromWorktree(
          opened.id,
          wt.path,
          `Worktree changes from ${wt.branch ?? "detached"}`,
          "Imported from linked agent worktree",
          "worktree",
        );
        proposalList = [created, ...proposalList.filter((p) => p.id !== created.id)];
        fileInspect = null;
        selectedCommit = null;
        selectedStash = null;
        planDone = null;
        inspector = "working";
        selectedProposal = created.id;
        revision += 1;
        notice = `Proposed changes from ${wt.branch ?? wt.path}`;
      } catch (err) {
        error = String(err);
      }
    }, `Importing ${wt.branch ?? wt.path}…`);
  }

  async function onDeleteBranch(branch: string, remote?: string) {
    if (remote) {
      deleteAsk = { kind: "remote", branch, remote };
      return;
    }
    const ok = await write((id) => deleteBranch(id, branch, false), `Deleted ${branch} · Ctrl+Z undoes it`, `Deleting ${branch}…`);
    if (!ok && error?.includes("not fully merged")) {
      error = null;
      deleteAsk = { kind: "forceLocal", branch };
    }
  }

  async function confirmDeleteBranch() {
    const ask = deleteAsk;
    deleteAsk = null;
    if (!ask) {
      return;
    }
    if (ask.kind === "remote" && ask.remote) {
      await write(
        (id) => deleteRemoteBranch(id, ask.remote!, ask.branch),
        `Deleted ${ask.remote}/${ask.branch}`,
        `Deleting ${ask.remote}/${ask.branch}…`,
      );
    } else {
      await write((id) => deleteBranch(id, ask.branch, true), `Deleted ${ask.branch} · Ctrl+Z undoes it`, `Deleting ${ask.branch}…`);
    }
  }

  async function runReset(mode: ResetMode) {
    const target = resetAsk;
    resetAsk = null;
    if (!target) {
      return;
    }
    await write(
      (id) => resetRepo(id, target.id, mode),
      `Reset (${mode}) to ${target.shortId} · Ctrl+Z undoes it`,
      `Resetting (${mode}) to ${target.shortId}…`,
    );
  }

  async function runSequencer() {
    const ask = sequencerAsk;
    sequencerAsk = null;
    if (!ask) {
      return;
    }
    if (ask.kind === "cherryPick") {
      const ok = await write((id) => cherryPick(id, ask.id), undefined, `Cherry-picking ${ask.shortId}…`);
      if (ok && !sequencer) {
        notice = `Cherry-picked ${ask.shortId} · Ctrl+Z undoes it`;
      }
    } else {
      const ok = await write((id) => revertCommit(id, ask.id), undefined, `Reverting ${ask.shortId}…`);
      if (ok && !sequencer) {
        notice = `Reverted ${ask.shortId} · Ctrl+Z undoes it`;
      }
    }
  }

  async function openPrForBranch(branch: string) {
    if (!opened) {
      return;
    }
    try {
      const url = await prUrl(opened.id, branch);
      if (url) {
        await openExternalUrl(url);
      } else {
        notice = "No GitHub or Bitbucket remote found for this branch";
      }
    } catch (err) {
      error = String(err);
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (!active || suspended || dialogOpen) {
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
    if (isPullKey(event) && opened && !conflicted) {
      event.preventDefault();
      startPull("auto", false);
      return;
    }
    if (isPushKey(event) && opened && status?.head.kind === "branch" && !conflicted) {
      event.preventDefault();
      pushAsk = { initialForce: false };
      return;
    }
    if (isNewBranchKey(event) && opened && !conflicted) {
      event.preventDefault();
      branchAsk = {
        mode: "create",
        at: selectedCommit,
        atLabel: selectedCommit ? selectedCommit.slice(0, 7) : null,
      };
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
    if (
      event.key === "Escape" &&
      (selectedCommit || selectedStash !== null || selectedProposal || fileInspect) &&
      !inTextField(event.target)
    ) {
      closeDetail();
      return;
    }
    if (event.key === "Escape" && planDone && !inTextField(event.target)) {
      planDone = null;
      return;
    }
    if (event.key === "Escape" && inspector === "oplog" && !inTextField(event.target)) {
      inspector = "working";
      return;
    }
    const kind = opLogKey(event);
    if (kind && opened && !planning) {
      event.preventDefault();
      revert(kind);
    }
  }

  function cyclePanes(step: number) {
    const panes = [...(rootEl?.querySelectorAll<HTMLElement>("[data-pane]") ?? [])];
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

  async function onCommit(message: string, options?: CommitOptions) {
    if (!(await write((id) => commit(id, message, options), undefined, options?.amend ? "Amending…" : "Committing…"))) {
      throw new Error("commit failed");
    }
    if (pullAfterCommit) {
      pullAfterCommit = false;
      startPull("auto", false);
    }
  }

  let refreshing = false;
  let refreshAgain = false;

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

  async function load(repoPath: string) {
    loading = true;
    error = null;
    try {
      const next = await openRepo(repoPath);
      const state = await read(next.id);
      if (opened) {
        await closeRepo(opened.id);
      }
      show(next, state);
    } catch (err) {
      const msg = String(err);
      if (!opened && onopenfail) {
        onopenfail(msg);
      } else {
        error = msg;
      }
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    const stops: (() => void)[] = [];
    let destroyed = false;
    const subscriptions = [
      onRepoChanged(onOutsideChange),
      onFetchProgress((id, progress) => {
        if (fetching && opened?.id === id) {
          fetching = { ...fetching, progress };
        }
      }),
      onTaskProgress((id, op, progress) => {
        if (fetching && opened?.id === id) {
          fetching = { op, progress };
        }
      }),
    ];
    for (const subscription of subscriptions) {
      subscription.then((unlisten) => (destroyed ? unlisten() : stops.push(unlisten)));
    }
    load(path);
    return () => {
      destroyed = true;
      stops.forEach((stop) => stop());
    };
  });

  onDestroy(() => {
    if (opened) {
      const id = opened.id;
      opened = null;
      closeRepo(id).catch((err) => console.error("close repo:", err));
    }
  });
</script>

<svelte:window onkeydown={onKeydown} />

<main class="app" bind:this={rootEl}>
  <header class="tb">
    <button class="pill repo" aria-label="Open repository…" title={opened ? `${opened.root} · open another…` : "Open a repository"} onclick={onopenother}>
      <span aria-hidden="true">▣</span>
      <span class="nm">{opened ? repoName(opened.root) : repoName(path)}</span>
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
        <span class="pill warn" title="Commits made here belong to no branch">
          ⚠ Detached HEAD at {branchName}
          <button
            class="bare link"
            disabled={busy}
            onclick={() => (branchAsk = { mode: "create", at: null, atLabel: `detached HEAD (${branchName})` })}
          >
            Create branch…
          </button>
        </span>
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
      <button
        disabled={!!fetching || busy || !!conflicted || !currentBranch?.upstream}
        title={currentBranch?.upstream ? `Pull from ${currentBranch.upstream} (Ctrl+Shift+L)` : "No upstream configured"}
        onclick={() => startPull("auto", false)}
      >
        <span aria-hidden="true">⇣</span> Pull{#if currentBranch?.behind}<span class="ab">↓{currentBranch.behind}</span>{/if}
      </button>
      <button
        disabled={!!fetching || busy || !!conflicted || status.head.kind !== "branch"}
        title="Push (Ctrl+Shift+P)"
        onclick={() => (pushAsk = { initialForce: false })}
      >
        <span aria-hidden="true">⇡</span> Push{#if currentBranch?.ahead}<span class="ab">↑{currentBranch.ahead}</span>{/if}
      </button>
      <button
        disabled={busy || !!conflicted || status.entries.length === 0}
        title="Stash uncommitted changes"
        onclick={() => (stashAsk = true)}
      >
        Stash…
      </button>
      <span class="sp"></span>
      <button class="pill undo" disabled={busy} title="Undo the last action (Ctrl+Z)" onclick={() => revert("undo")}>
        <span aria-hidden="true">↶</span> Undo{lastAction ? `: ${lastAction}` : ""}
        <span class="k" aria-hidden="true">Ctrl+Z</span>
      </button>
      <button class="pill" disabled={busy} title={nextRedo ? `Redo: ${nextRedo} (Ctrl+Y)` : "Redo (Ctrl+Y)"} aria-label="Redo" onclick={() => revert("redo")}>↷</button>
    {/if}
  </header>

  {#if error}
    <ErrorBanner message={error} onfix={handleErrorFix} ondismiss={() => (error = null)} />
  {/if}

  {#if opened && status}
    {#if merging}
      <MergeBanner
        theirs={merging.theirs}
        branch={branchName}
        left={mergeLeft}
        total={merging.conflicts.length}
        unconfirmed={merging.unityVerify?.reduce((n, f) => n + f.verify.filter((v) => !v.confirmed).length, 0) ?? 0}
        {busy}
        oncommit={commitMerge}
        onabort={() => write((id) => mergeAbort(id), "Merge aborted · Ctrl+Z brings it back", "Aborting merge…")}
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
    {:else if sequencer}
      <SequencerBanner
        {sequencer}
        {busy}
        oncontinue={() => stepSequencer("continue")}
        onabort={() => stepSequencer("abort")}
      />
    {/if}
    <div
      class="body"
      bind:this={bodyEl}
      class:merging={!!conflicted || !!comparing}
      style:--side-w={prefs.panes.sidebarWidth != null ? `${prefs.panes.sidebarWidth}px` : undefined}
      style:--insp-w={prefs.panes.inspectorWidth != null ? `${prefs.panes.inspectorWidth}px` : undefined}
    >
      <div class="contents" data-pane="sidebar">
        <Sidebar
          repoId={opened.id}
          {revision}
          {busy}
          bind:data={branchData}
          merging={!!conflicted}
          {selectedStash}
          proposals={activeProposals}
          worktrees={linkedWorktrees}
          stacks={stackList}
          {selectedProposal}
          workingOpen={inspector === "working" && !selectedCommit && selectedStash === null && !selectedProposal && !fileInspect && !planning && !planDone}
          changesCount={status.entries.length}
          oplogOpen={inspector === "oplog" && !selectedCommit && selectedStash === null && !selectedProposal && !fileInspect}
          lfsInfo={lfs}
          {locks}
          tags={tagList}
          oncreatetag={() =>
            (tagAsk = {
              target: selectedCommit,
              targetLabel: selectedCommit ? selectedCommit.slice(0, 7) : null,
            })}
          onpushtag={onPushTag}
          ondeletetag={onDeleteTag}
          ondeleteremotetag={onDeleteRemoteTag}
          onselecttag={inspectCommit}
          oncheckout={guardedCheckout}
          onmerge={(branch) => (mergeAsk = { rev: branch, label: branch })}
          onrebase={(branch) => (rebaseAsk = branch)}
          oncompare={(branch) => openCompare(currentPoint(), compareRev(branch))}
          oncomparetag={(tag) => openCompare(compareRev(tag), currentPoint())}
          oncreatebranch={(at) => (branchAsk = { mode: "create", at: at ?? null, atLabel: at ?? null })}
          onrenamebranch={(branch) => (branchAsk = { mode: "rename", currentName: branch })}
          ondeletebranch={onDeleteBranch}
          onsetupstream={(branch, upstream) =>
            write(
              (id) => setUpstream(id, branch, upstream),
              upstream ? `Tracking ${upstream}` : `Cleared upstream for ${branch}`,
              "Setting upstream…",
            )}
          oncheckoutremote={(remoteBranch) => (branchAsk = { mode: "checkoutRemote", remoteBranch })}
          onopenpr={openPrForBranch}
          onselectproposal={(id) => {
            fileInspect = null;
            selectedCommit = null;
            selectedStash = null;
            planDone = null;
            inspector = "working";
            selectedProposal = id;
          }}
          onrestack={runRestack}
          onopenstackpr={openStackPr}
          onproposeworktree={proposeWorktree}
          onstashselect={(idx) => {
            selectedStash = idx;
            selectedProposal = null;
            selectedCommit = null;
            planDone = null;
          }}
          onstashapply={(idx) => stashWrite("apply", idx)}
          onstashpop={(idx) => stashWrite("pop", idx)}
          onstashdrop={(idx) => stashWrite("drop", idx)}
          onrefreshlocks={() => opened && refreshLocks(opened.id)}
          onunlock={onUnlockFile}
          onworking={() => {
            inspector = "working";
            planDone = null;
            closeDetail();
          }}
          onoplog={() => {
            inspector = inspector === "oplog" ? "working" : "oplog";
            closeDetail();
          }}
        />
      </div>
      <Splitter
        orientation="vertical"
        label="Resize sidebar"
        onstart={() => (dragStartSidebar = measuredSidebarW())}
        ondrag={(d) => setPaneSize("sidebarWidth", clampSidebar(dragStartSidebar + d))}
        onstep={(d) => setPaneSize("sidebarWidth", clampSidebar(measuredSidebarW() + d))}
        onreset={() => setPaneSize("sidebarWidth", null)}
      />
      {#snippet opLog(repoId: number, onclose?: () => void)}
        <OpLogPanel
          {repoId}
          {revision}
          {busy}
          stats={opStats}
          reflog={reflogEntries}
          onundo={() => revert("undo")}
          onrestoreto={restoreToEntry}
          onrestorefiles={(entry) => write((id) => restoreFiles(id, entry.id), undefined, "Restoring files…")}
          onprune={pruneOpLog}
          onrestorereflog={(entry) => inspectCommit(entry.id)}
          {onclose}
        />
      {/snippet}
      {#if conflicted}
        {@const repoId = opened.id}
        {#snippet continueOp()}
          {#if rebasing}
            <section class="pane" aria-label="Step resolved">
              <div class="ph">Step {rebasing.step} of {rebasing.total} resolved</div>
              <div class="empty">
                <b>Every file of "{rebasing.subject}" is resolved</b>
                <span>Continue commits it and applies the next {rebasing.total - rebasing.step || "no"} commit{rebasing.total - rebasing.step === 1 ? "" : "s"}.</span>
                <span><button class="pri" disabled={busy} onclick={() => stepRebase("continue")}>Continue rebase</button></span>
              </div>
            </section>
          {:else if sequencer}
            {@const verb = sequencer.kind === "cherryPick" ? "cherry-pick" : "revert"}
            <section class="pane" aria-label="Conflicts resolved">
              <div class="ph">{sequencer.shortId} resolved</div>
              <div class="empty">
                <b>Every file of "{sequencer.subject}" is resolved</b>
                <span>Continue commits the {verb}.</span>
                <span><button class="pri" disabled={busy} onclick={() => stepSequencer("continue")}>Continue {verb}</button></span>
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
          unityInfo={unityState}
          {locks}
          onresolvewith={resolveText}
          onresolveside={resolvePick}
          onresolveunity={resolveUnity}
          oncommit={commitMerge}
          onsetupSmartMerge={setupAndRemergeUnity}
          onremergeUnity={runRemergeUnity}
          onconfirmVerify={onConfirmVerify}
          children={inspector === "oplog" ? () => opLog(repoId, () => (inspector = "working")) : undefined}
          whenResolved={merging ? undefined : continueOp}
        />
      {:else if comparing}
        {#key comparing.key}
          <CompareView
            repoId={opened.id}
            a={comparing.a}
            b={comparing.b}
            currentBranch={currentBranch?.name ?? null}
            root={opened.root}
            onlaunchfail={(message) => (error = message)}
            {busy}
            ontake={takeFromCompare}
            onclose={() => {
              comparing = null;
              tick().then(() => graph?.focus());
            }}
          />
        {/key}
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
          {#if commitGraphPrompt === opened.id}
            <div class="cgraph" role="status">
              <span>Large history ({opened.rows} rows) — build a git commit-graph cache for faster loads?</span>
              <span class="sp"></span>
              <button
                type="button"
                class="pri"
                disabled={busy}
                onclick={async () => {
                  await optimizeCommitGraph();
                  commitGraphPrompt = null;
                }}>Build cache</button
              >
              <button type="button" onclick={() => (commitGraphPrompt = null)}>Dismiss</button>
            </div>
          {/if}
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
                  fileInspect = null;
                  if (row && row.proposal) {
                    selectedProposal = row.proposal;
                    selectedStash = null;
                    selectedCommit = null;
                  } else if (row && row.stash != null) {
                    selectedStash = row.stash;
                    selectedProposal = null;
                    selectedCommit = null;
                  } else {
                    selectedStash = null;
                    selectedProposal = null;
                    selectedCommit = row && !row.wip ? row.id : null;
                    if (row?.wip) {
                      inspector = "working";
                    }
                  }
                  planDone = null;
                }
              }}
              oncopy={copyHash}
              hits={finding ? findHits : null}
              fadedIds={staleOriginIds}
              onedit={planning ? undefined : editRow}
              onmenu={planning ? undefined : (row, x, y) => (rowMenu = { row, x, y })}
              oncompare={planning ? undefined : (ra, rb) => openCompare(rowPoint(ra), rowPoint(rb))}
              renaming={renaming?.index ?? null}
              onrename={renameTip}
            />
          {/key}
        </section>
        <Splitter
          orientation="vertical"
          label="Resize inspector"
          onstart={() => (dragStartInspector = measuredInspectorW())}
          ondrag={(d) => setPaneSize("inspectorWidth", clampInspector(dragStartInspector - d))}
          onstep={(d) => setPaneSize("inspectorWidth", clampInspector(measuredInspectorW() - d))}
          onreset={() => setPaneSize("inspectorWidth", null)}
        />
        <aside class="pane inspector" bind:this={inspEl} data-pane="inspector">
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
          {:else if currentProposal}
            <ProposalView
              repoId={opened.id}
              proposal={currentProposal}
              {busy}
              onapprove={(message, paths) => approveProposal(currentProposal.id, message, paths)}
              onrequestchanges={(comment) => requestProposalChanges(currentProposal.id, comment)}
              onreject={() => rejectProposal(currentProposal.id)}
              onclose={closeDetail}
            />
          {:else if selectedStash !== null}
            {@const idx = selectedStash}
            <StashDetail
              repoId={opened.id}
              index={idx}
              {busy}
              onapply={() => stashWrite("apply", idx)}
              onpop={() =>
                stashWrite("pop", idx).then((ok) => {
                  if (ok) closeDetail();
                })}
              ondrop={() =>
                stashWrite("drop", idx).then((ok) => {
                  if (ok) closeDetail();
                })}
              onclose={closeDetail}
            />
          {:else if fileInspect}
            <FileHistoryBlame
              repoId={opened.id}
              path={fileInspect.path}
              initialMode={fileInspect.mode}
              rev={fileInspect.rev}
              onselectcommit={inspectCommit}
              onclose={() => (fileInspect = null)}
            />
          {:else if selectedCommit}
            <CommitDetail
              repoId={opened.id}
              commit={selectedCommit}
              root={opened.root}
              onlaunchfail={(message) => (error = message)}
              onhistory={(p) => openFileInspect(p, "history", null)}
              onblame={(p, rev) => openFileInspect(p, "blame", rev)}
              onclose={closeDetail}
            />
          {:else if planDone}
            {@const repoId = opened.id}
            <PlanDone
              done={planDone}
              {busy}
              onundo={() => revert("undo")}
              onforcepush={() => (pushAsk = { initialForce: true })}
              onopenpr={() => planDone && openPrForBranch(planDone.branch)}
            >
              {@render opLog(repoId)}
            </PlanDone>
          {:else if inspector === "oplog"}
            {@render opLog(opened.id, () => (inspector = "working"))}
          {:else}
            <WorkingCopy
              bind:this={workingCopy}
              repoId={opened.id}
              {status}
              {revision}
              branch={branchName}
              {busy}
              suspended={suspended || dialogOpen}
              {lastAction}
              {metaIssues}
              {locks}
              lfsPatterns={lfs?.trackedPatterns ?? []}
              locksSupported={lfs?.supportsLocks ?? false}
              overlap={radar}
              {pullAfterCommit}
              onpull={() => startPull("auto", false)}
              oncommitpull={commitThenPull}
              onlaunchfail={(message) => (error = message)}
              bind:selected={selectedFile}
              onstage={(paths) => write((id) => stage(id, paths), undefined, `Staging ${files(paths)}…`)}
              onunstage={(paths) => write((id) => unstage(id, paths), undefined, `Unstaging ${files(paths)}…`)}
              ondiscard={(paths) => write((id) => discard(id, paths), undefined, `Discarding ${files(paths)}…`)}
              onhunk={onHunk}
              onlines={onLines}
              onlock={onLockFile}
              onunlock={onUnlockFile}
              oncommit={onCommit}
              ondraft={draftCommitMessage}
              onstash={() => (stashAsk = true)}
              onhistory={(p) => openFileInspect(p, "history", null)}
              onblame={(p) => openFileInspect(p, "blame", null)}
            />
          {/if}
        </aside>
      {/if}
      {#if headsUp.length && !conflicted && !comparing}
        <OverlapHeadsUp
          files={headsUp}
          busy={busy || !!fetching}
          onpull={() => {
            headsUp = [];
            startPull("auto", false);
          }}
          oncommitpull={commitThenPull}
          onsee={seeTheirChange}
          ondismiss={() => (headsUp = [])}
        />
      {/if}
    </div>
    {#if rowMenu}
      <ContextMenu x={rowMenu.x} y={rowMenu.y} items={menuItems(rowMenu.row)} onclose={() => (rowMenu = null)} />
    {/if}
    {#if mergeAsk}
      <MergeDialog
        repoId={opened.id}
        branch={mergeAsk.rev}
        label={mergeAsk.label}
        into={branchName}
        dirty={status.entries.length > 0}
        onmerge={() => mergeAsk && startMerge(mergeAsk.rev, mergeAsk.label)}
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
    {#if pullAsk}
      <PullStrategyDialog
        ahead={pullAsk.ahead}
        behind={pullAsk.behind}
        upstream={pullAsk.upstream}
        {busy}
        onpull={(strategy, remember) => startPull(strategy, remember)}
        oncancel={() => (pullAsk = null)}
      />
    {/if}
    {#if pushAsk}
      <PushDialog
        repoId={opened.id}
        initialForce={pushAsk.initialForce}
        locksSupported={lfs?.supportsLocks ?? false}
        {busy}
        onpush={startPush}
        oncancel={() => (pushAsk = null)}
      />
    {/if}
    {#if branchAsk}
      <BranchDialog
        mode={branchAsk.mode}
        currentName={branchAsk.currentName}
        atLabel={branchAsk.atLabel}
        remoteBranch={branchAsk.remoteBranch}
        {busy}
        onsubmit={submitBranch}
        oncancel={() => (branchAsk = null)}
      />
    {/if}
    {#if deleteAsk}
      <DeleteBranchDialog
        kind={deleteAsk.kind}
        branch={deleteAsk.branch}
        remote={deleteAsk.remote}
        {busy}
        onconfirm={confirmDeleteBranch}
        oncancel={() => (deleteAsk = null)}
      />
    {/if}
    {#if resetAsk}
      <ResetDialog
        branch={branchName}
        shortId={resetAsk.shortId}
        subject={resetAsk.subject}
        dirty={status.entries.length > 0}
        {busy}
        onreset={runReset}
        oncancel={() => (resetAsk = null)}
      />
    {/if}
    {#if sequencerAsk}
      <CherryPickDialog
        repoId={opened.id}
        commit={sequencerAsk.id}
        shortId={sequencerAsk.shortId}
        subject={sequencerAsk.subject}
        branch={branchName}
        kind={sequencerAsk.kind}
        {busy}
        onconfirm={runSequencer}
        oncancel={() => (sequencerAsk = null)}
      />
    {/if}
    {#if stashAsk}
      <StashDialog
        branch={branchName}
        hasUntracked={status.entries.some((e) => e.kind === "untracked")}
        onlyUntracked={status.entries.length > 0 && status.entries.every((e) => e.kind === "untracked")}
        {busy}
        onsave={(message, includeUntracked) => {
          stashAsk = false;
          write((id) => stashSave(id, message, includeUntracked), "Stashed changes · Ctrl+Z undoes it", "Stashing…");
        }}
        oncancel={() => (stashAsk = false)}
      />
    {/if}
    {#if lfsAsk && opened}
      <LfsDownloadsDialog repoId={opened.id} repoName={repoName(opened.root)} oncancel={() => (lfsAsk = false)} />
    {/if}
    {#if tagAsk}
      <TagDialog
        targetLabel={tagAsk.targetLabel}
        {busy}
        onsubmit={submitTag}
        oncancel={() => (tagAsk = null)}
      />
    {/if}
    {#if openAssetsAsk}
      <Dialog title="Unity Editor is open" oncancel={() => (openAssetsAsk = null)}>
        <p class="ab">
          <code>Temp/UnityLockfile</code> is present and <b>{openAssetsAsk.label}</b> updates files inside
          <code>Assets/</code>. Modifying Assets while Unity is running can trigger broken asset reloads or lock errors.
        </p>
        {#snippet footer()}
          <button type="button" onclick={() => (openAssetsAsk = null)}>Cancel</button>
          <button
            type="button"
            class="pri"
            onclick={() => {
              const run = openAssetsAsk?.run;
              openAssetsAsk = null;
              run?.();
            }}>Proceed anyway</button
          >
        {/snippet}
      </Dialog>
    {/if}
  {:else}
    <div class="start empty">
      {#if loading}
        <b>Opening…</b>
      {:else}
        <b>gitgud</b>
        <span>Open a repository to see its history and changes.</span>
      {/if}
    </div>
  {/if}

  <footer class="strip">
    {#if fetching && opened}
      {@const id = opened.id}
      {@const pct = fetching.progress?.percent ?? 0}
      {@const opLabel = { fetch: "Fetch", pull: "Pull", push: "Push", lfs: "LFS download" }[fetching.op]}
      {@const verb = { fetch: "Fetching", pull: "Pulling", push: "Pushing", lfs: "Downloading" }[fetching.op]}
      <span class="task" aria-label="{opLabel} progress">
        {fetching.op === "push" ? "↑" : "↓"} {verb}…
        {#if fetching.progress}
          {fetching.progress.phase} {fetching.progress.percent}%
        {/if}
        <span class="bar" aria-hidden="true"><i style:width="{pct}%"></i></span>
        <button class="bare link" onclick={() => cancelTask(id)}>Cancel</button>
      </span>
    {:else if busy}
      <LoadingBar label={working} />
    {:else if notice}
      <span role="status">{notice}</span>
    {:else if upstreamLine}
      <span class="muted">● {upstreamLine}</span>
      {@const radarLabel = stripLabel(radar)}
      {#if radarLabel}
        <span class="radar {radarLabel.risk}" title="Overlap radar: files you changed that also changed on the remote">
          {radarLabel.risk === "wontMerge" ? "!" : "~"}
          {radarLabel.text}
        </span>
      {/if}
    {:else if opened}
      <span class="muted">{opened.root}</span>
    {/if}
    <span class="sp"></span>
    {#if active}
      {#await git catch err}
        <span role="alert" class="bad">{err}</span>
      {/await}
      {#if opened}
        <AppStatsBar repos={[{ root: opened.root, name: repoName(opened.root) }]} {onopensettings} />
      {/if}
    {/if}
  </footer>
</main>

<style>
  .app {
    flex: 1;
    min-height: 0;
    height: 100%;
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
  .body {
    --side-w: 210px;
    --insp-w: clamp(380px, 36vw, 560px);
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    /* Side panes give way to their drag minimums (clampSidebar / clampInspector) before history drops below 260px. */
    grid-template-columns: minmax(150px, var(--side-w)) 5px minmax(260px, 1fr) 5px minmax(300px, var(--insp-w));
  }
  .body.merging {
    grid-template-columns: minmax(150px, var(--side-w)) 5px minmax(260px, 1fr);
  }
  @media (min-width: 2400px) {
    .body:not(.merging) {
      --side-w: 240px;
      --insp-w: min(1150px, 46vw);
    }
  }
  .body > .contents > :global(.side) {
    border-right: 0;
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--panel);
  }
  .pane.center {
    overflow: hidden;
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
  .center > :global(.graph) {
    flex: 1;
  }
  .cgraph {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    background: var(--sel);
    border-bottom: 1px solid var(--line);
    font-size: 11.5px;
  }
  .inspector {
    overflow: hidden;
  }
  .inspector > :global(.detail),
  .inspector > :global(.oplog),
  .inspector > :global(.fhb) {
    flex: 1;
    min-height: 0;
  }
  .start {
    flex: 1;
  }
  .radar {
    font: 600 10.5px var(--mono);
    padding: 1px 7px;
    border-radius: 9px;
    background: var(--warn);
    color: var(--warn-ink);
  }
  .radar.wontMerge {
    background: var(--bad);
    color: var(--bad-ink);
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
  .bad {
    color: var(--bad-ink);
  }
</style>
