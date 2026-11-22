import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

export interface GitInfo {
  path: string;
  version: string;
}

export type Head = { kind: "branch"; name: string } | { kind: "unborn"; name: string } | { kind: "detached"; id: string };

export type Change = "added" | "modified" | "deleted" | "typeChange";

export interface Entry {
  path: string;
  kind: "tracked" | "untracked" | "conflicted";
  index: Change | null;
  worktree: Change | null;
}

export interface Status {
  root: string;
  head: Head;
  entries: Entry[];
}

export interface Opened {
  id: number;
  root: string;
  /** Graph rows, the working-copy row included. */
  rows: number;
  /** Bumped each time the graph is rebuilt: cached rows are stale. */
  graphVersion: number;
}

export type RefLabel =
  | { kind: "head" }
  | { kind: "branch"; name: string; current: boolean }
  | { kind: "remote"; name: string }
  | { kind: "tag"; name: string };

export interface Segment {
  from: number;
  to: number;
  part: "top" | "bottom" | "through";
  /** Plan previews dash the planned result and fade what it replaces; absent is solid. */
  stroke?: "dashed" | "faded";
}

export interface RowLayout {
  node: number;
  segments: Segment[];
  overflow: boolean;
}

export interface Row {
  id: string;
  shortId: string;
  subject: string;
  author: string;
  /** Author time, seconds since the Unix epoch. */
  time: number;
  labels: RefLabel[];
  wip: boolean;
  /** On a plan preview's planned commit: what the plan does to it ("reworded"…). */
  after?: string | null;
  /** On a plan preview: a commit the plan replaces. */
  faded?: boolean;
  layout: RowLayout;
}

export interface Person {
  name: string;
  email: string;
  /** Seconds since the Unix epoch. */
  time: number;
}

export interface FileChange {
  path: string;
  change: Change;
}

export interface CommitDetail {
  id: string;
  shortId: string;
  subject: string;
  body: string;
  author: Person;
  committer: Person;
  parents: string[];
  /** Against the first parent, no rename detection, sorted by path. */
  files: FileChange[];
}

export interface DiffLine {
  kind: "context" | "added" | "removed";
  old: number | null;
  new: number | null;
  text: string;
}

export interface Hunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  binary: boolean;
  hunks: Hunk[];
}

export interface Branch {
  /** Short name, e.g. `feature/login`. */
  name: string;
  current: boolean;
  /** e.g. `origin/main`. */
  upstream: string | null;
  ahead: number;
  behind: number;
}

export interface Remote {
  name: string;
  /** e.g. `origin/main`. */
  branches: string[];
}

export interface Branches {
  local: Branch[];
  remotes: Remote[];
  /** When the repo last fetched, seconds since the Unix epoch; `null` if never. */
  fetched: number | null;
}

export interface OpEntry {
  id: number;
  /** Seconds since the Unix epoch. */
  time: number;
  action: string;
  undone: boolean;
  /** Files a destructive action (a discard) changed. */
  paths: string[];
}

export interface Reverted {
  opened: Opened;
  /** The action undone or redone, e.g. `Stage a.txt`. */
  action: string;
}

export type ConflictKind = "unityYaml" | "code" | "binary";

export interface Conflict {
  path: string;
  kind: ConflictKind;
  /** Staged since the merge stopped. */
  resolved: boolean;
}

export interface MergeState {
  /** The branch being merged in, when gitgud started the merge. */
  theirs: string | null;
  /** Prefilled commit message. */
  message: string;
  /** In list order: Unity YAML, code, binary; then path. */
  conflicts: Conflict[];
}

export type Chunk =
  | { kind: "same"; lines: string[] }
  | { kind: "conflict"; yours: string[]; base: string[]; theirs: string[] };

export interface ConflictText {
  chunks: Chunk[];
  finalNewline: boolean;
}

export interface SideInfo {
  size: number;
  lfs: boolean;
}

/** `null` sides were deleted. */
export interface ConflictSides {
  yours: SideInfo | null;
  theirs: SideInfo | null;
}

export type MergeSide = "yours" | "theirs";

export interface StepConflict {
  /** 1-based. */
  step: number;
  shortId: string;
  subject: string;
  paths: string[];
}

export interface RebasePlan {
  /** Commits that would be replayed. */
  commits: number;
  /** Commits on the target the branch doesn't have. */
  behind: number;
  conflict: StepConflict | null;
}

export interface RebaseState {
  branch: string;
  onto: string;
  /** 1-based, of `total`. */
  step: number;
  total: number;
  /** Subject of the commit being applied. */
  subject: string;
  conflicts: Conflict[];
  /** Stopped by an Edit step: Continue amends the commit with what is staged. */
  editing: boolean;
}

export type RebaseStep = "continue" | "skip" | "abort";

export type PlanAction = "pick" | "reword" | "edit" | "squash" | "fixup" | "drop";

export interface PlanCommit {
  id: string;
  shortId: string;
  subject: string;
  message: string;
  author: string;
  time: number;
  /** Pick, or fixup / squash for `fixup!` / `squash!` commits. */
  action: PlanAction;
}

export interface HistoryPlan {
  branch: string;
  /** Parent of the oldest commit: the plan replays onto it unless moved. */
  base: string;
  baseShort: string;
  baseSubject: string;
  /** Newest first, fixup! / squash! already moved under their target. */
  commits: PlanCommit[];
  /** Other local branches on one of the commits (`--update-refs`). */
  dependents: string[];
  /** The upstream, when it has some of these commits. */
  pushed: string | null;
}

export interface PlanStep {
  id: string;
  action: PlanAction;
  message?: string | null;
}

export type StepOutcome =
  | { kind: "clean" }
  | { kind: "afterConflict"; step: number }
  | { kind: "conflict"; paths: string[]; tree: string }
  | { kind: "dropped" };

export interface StepPrediction {
  id: string;
  /** 1-based, oldest first; null for a drop. */
  step: number | null;
  outcome: StepOutcome;
}

export interface PlanPrediction {
  /** Newest first. */
  steps: StepPrediction[];
  before: number;
  after: number;
  /** Files the result differs in from HEAD; null when a conflict leaves it open. */
  changed: number | null;
}

export interface PlanPreview {
  prediction: PlanPrediction;
  /** Rows of the preview graph (`preview_rows`). */
  rows: number;
}

/** Staged: HEAD → index. Unstaged: index → work tree. */
export type Side = "staged" | "unstaged";

/** A hunk's `@@` numbers; Rust refuses the action if the diff moved since. */
export type HunkHeader = Pick<Hunk, "oldStart" | "oldLines" | "newStart" | "newLines">;

export const hunkHeader = (h: Hunk): HunkHeader => ({
  oldStart: h.oldStart,
  oldLines: h.oldLines,
  newStart: h.newStart,
  newLines: h.newLines,
});

/** Every command the frontend invokes: its arguments and its Ok value. Errors arrive as strings. */
export interface Commands {
  git_info: { args: Record<string, never>; result: GitInfo };
  launch_path: { args: Record<string, never>; result: string | null };
  open_repo: { args: { path: string }; result: Opened };
  close_repo: { args: { id: number }; result: null };
  repo_status: { args: { id: number }; result: Status };
  graph_rows: { args: { id: number; start: number; count: number }; result: Row[] };
  graph_find: { args: { id: number; query: string }; result: number[] };
  commit_detail: { args: { id: number; commit: string }; result: CommitDetail };
  commit_file_diff: { args: { id: number; commit: string; path: string }; result: FileDiff };
  worktree_file_diff: { args: { id: number; path: string; side: Side }; result: FileDiff };
  stage_hunk: { args: { id: number; path: string; hunk: HunkHeader }; result: Opened };
  unstage_hunk: { args: { id: number; path: string; hunk: HunkHeader }; result: Opened };
  discard: { args: { id: number; paths: string[] }; result: Opened };
  discard_hunk: { args: { id: number; path: string; hunk: HunkHeader }; result: Opened };
  refresh_repo: { args: { id: number }; result: Opened };
  undo: { args: { id: number }; result: Reverted };
  redo: { args: { id: number }; result: Reverted };
  fetch: { args: { id: number }; result: Opened };
  op_log: { args: { id: number }; result: OpEntry[] };
  branches: { args: { id: number }; result: Branches };
  checkout: { args: { id: number; branch: string }; result: Opened };
  restore_to: { args: { id: number; entry: number }; result: Reverted };
  restore_files: { args: { id: number; entry: number }; result: Opened };
  cancel_fetch: { args: { id: number }; result: boolean };
  stage: { args: { id: number; paths: string[] }; result: Opened };
  unstage: { args: { id: number; paths: string[] }; result: Opened };
  commit: { args: { id: number; message: string }; result: Opened };
  predict_merge: { args: { id: number; branch: string }; result: string[] };
  merge: { args: { id: number; branch: string }; result: Opened };
  merge_state: { args: { id: number }; result: MergeState | null };
  conflict_text: { args: { id: number; path: string }; result: ConflictText };
  conflict_sides: { args: { id: number; path: string }; result: ConflictSides };
  resolve_with: { args: { id: number; path: string; content: string; how: string }; result: Opened };
  resolve_side: { args: { id: number; path: string; side: MergeSide }; result: Opened };
  merge_abort: { args: { id: number }; result: Opened };
  merge_commit: { args: { id: number; message: string }; result: Opened };
  predict_rebase: { args: { id: number; onto: string }; result: RebasePlan };
  rebase: { args: { id: number; onto: string }; result: Opened };
  rebase_state: { args: { id: number }; result: RebaseState | null };
  rebase_step: { args: { id: number; step: RebaseStep }; result: Opened };
  history_plan: { args: { id: number; from: string }; result: HistoryPlan };
  plan_preview: { args: { id: number; onto: string; base: string; steps: PlanStep[] }; result: PlanPreview };
  preview_rows: { args: { id: number; start: number; count: number }; result: Row[] };
  predicted_file: { args: { id: number; tree: string; path: string }; result: string };
  run_plan: { args: { id: number; onto: string; base: string; steps: PlanStep[]; updateRefs: boolean }; result: Opened };
  reword_head: { args: { id: number; message: string }; result: Opened };
  quick_edit: { args: { id: number; commit: string; action: "fixup" | "drop" }; result: Opened };
  "plugin:window|set_title": { args: { label: string; value: string }; result: null };
  "plugin:dialog|open": { args: { options: { directory: boolean; title: string } }; result: string | null };
}

function call<K extends keyof Commands>(cmd: K, args?: Commands[K]["args"]): Promise<Commands[K]["result"]> {
  return invoke<Commands[K]["result"]>(cmd, args);
}

export const gitInfo = () => call("git_info");
export const launchPath = () => call("launch_path");
export const openRepo = (path: string) => call("open_repo", { path });
export const closeRepo = (id: number) => call("close_repo", { id });
export const repoStatus = (id: number) => call("repo_status", { id });
export const graphRows = (id: number, start: number, count: number) => call("graph_rows", { id, start, count });
/** Rows matching `query` (subject, author, hash prefix, ref name), in graph order. */
export const graphFind = (id: number, query: string) => call("graph_find", { id, query });
export const commitDetail =(id: number, commit: string) => call("commit_detail", { id, commit });
export const commitFileDiff = (id: number, commit: string, path: string) =>
  call("commit_file_diff", { id, commit, path });
export const worktreeFileDiff = (id: number, path: string, side: Side) => call("worktree_file_diff", { id, path, side });
export const stageHunk = (id: number, path: string, hunk: HunkHeader) => call("stage_hunk", { id, path, hunk });
export const unstageHunk = (id: number, path: string, hunk: HunkHeader) => call("unstage_hunk", { id, path, hunk });
/** Undoable with Ctrl+Z: the work tree is snapshotted first. */
export const discard = (id: number, paths: string[]) => call("discard", { id, paths });
export const discardHunk = (id: number, path: string, hunk: HunkHeader) => call("discard_hunk", { id, path, hunk });
export const refreshRepo = (id: number) => call("refresh_repo", { id });
export const undo = (id: number) => call("undo", { id });
export const redo = (id: number) => call("redo", { id });
export const fetchRemotes = (id: number) => call("fetch", { id });
/** Newest first. */
export const opLog = (id: number) => call("op_log", { id });
export const branches = (id: number) => call("branches", { id });
/** Undoable with Ctrl+Z: switches back and restores the files it rewrote. */
export const checkout = (id: number, branch: string) => call("checkout", { id, branch });
export const restoreTo = (id: number, entry: number) => call("restore_to", { id, entry });
export const restoreFiles = (id: number, entry: number) => call("restore_files", { id, entry });
export const cancelFetch = (id: number) => call("cancel_fetch", { id });
/** Files merging `branch` would leave conflicted; touches nothing. */
export const predictMerge = (id: number, branch: string) => call("predict_merge", { id, branch });
/** Clean merges commit at once; undoable with Ctrl+Z either way. */
export const merge = (id: number, branch: string) => call("merge", { id, branch });
export const mergeState = (id: number) => call("merge_state", { id });
export const conflictText = (id: number, path: string) => call("conflict_text", { id, path });
export const conflictSides = (id: number, path: string) => call("conflict_sides", { id, path });
/** `how` names the pick in the op log, e.g. "use both". */
export const resolveWith = (id: number, path: string, content: string, how: string) =>
  call("resolve_with", { id, path, content, how });
export const resolveSide = (id: number, path: string, side: MergeSide) => call("resolve_side", { id, path, side });
export const mergeAbort = (id: number) => call("merge_abort", { id });
export const mergeCommit = (id: number, message: string) => call("merge_commit", { id, message });

export interface Progress {
  phase: string;
  percent: number;
  done: number;
  total: number;
}

/** Calls `handler` with each progress line of a running fetch. Resolves to an unsubscribe. */
export const onFetchProgress = (handler: (id: number, progress: Progress) => void) =>
  listen<{ id: number; progress: Progress }>("fetch-progress", (e) => handler(e.payload.id, e.payload.progress));
export const stage = (id: number, paths: string[]) => call("stage", { id, paths });
export const unstage = (id: number, paths: string[]) => call("unstage", { id, paths });
export const commit = (id: number, message: string) => call("commit", { id, message });

/** Calls `handler` with a repo id when its files or refs change outside gitgud. Resolves to an unsubscribe. */
export const onRepoChanged = (handler: (id: number) => void) => listen<number>("repo-changed", (e) => handler(e.payload));

/** What rebasing onto `onto` would do; touches nothing. */
export const predictRebase = (id: number, onto: string) => call("predict_rebase", { id, onto });
/** Stops on conflicts; undoable with Ctrl+Z at every step. */
export const rebase = (id: number, onto: string) => call("rebase", { id, onto });
export const rebaseState = (id: number) => call("rebase_state", { id });
export const rebaseStep = (id: number, step: RebaseStep) => call("rebase_step", { id, step });

/** The plan for editing the current branch from commit `from` up to HEAD. */
export const historyPlan = (id: number, from: string) => call("history_plan", { id, from });
/** Predicts the plan and lays out its graph preview, read with `previewRows`; touches nothing. */
export const planPreview = (id: number, onto: string, base: string, steps: PlanStep[]) =>
  call("plan_preview", { id, onto, base, steps });
export const previewRows = (id: number, start: number, count: number) => call("preview_rows", { id, start, count });
/** A predicted file, conflict markers included. */
export const predictedFile = (id: number, tree: string, path: string) => call("predicted_file", { id, tree, path });
/** Runs the plan as one undoable rebase; stops on a conflict or an Edit step. */
export const runPlan = (id: number, onto: string, base: string, steps: PlanStep[], updateRefs: boolean) =>
  call("run_plan", { id, onto, base, steps, updateRefs });
export const rewordHead = (id: number, message: string) => call("reword_head", { id, message });
/** Folds a commit into its parent or drops it, at once; undoable. */
export const quickEdit = (id: number, commit: string, action: "fixup" | "drop") => call("quick_edit", { id, commit, action });

/** Names the open repo in the native title bar. */
export const setWindowTitle = (value: string) => call("plugin:window|set_title", { label: "main", value });

/** Native folder picker; `null` when cancelled. */
export const pickFolder = () => open({ directory: true, title: "Open repository" });
