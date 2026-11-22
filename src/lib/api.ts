import { invoke } from "@tauri-apps/api/core";
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

/** Every command the frontend invokes: its arguments and its Ok value. Errors arrive as strings. */
export interface Commands {
  git_info: { args: Record<string, never>; result: GitInfo };
  launch_path: { args: Record<string, never>; result: string | null };
  open_repo: { args: { path: string }; result: Opened };
  close_repo: { args: { id: number }; result: null };
  repo_status: { args: { id: number }; result: Status };
  graph_rows: { args: { id: number; start: number; count: number }; result: Row[] };
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

/** Native folder picker; `null` when cancelled. */
export const pickFolder = () => open({ directory: true, title: "Open repository" });
