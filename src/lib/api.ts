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
export const commitDetail = (id: number, commit: string) => call("commit_detail", { id, commit });
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

/** Native folder picker; `null` when cancelled. */
export const pickFolder = () => open({ directory: true, title: "Open repository" });
