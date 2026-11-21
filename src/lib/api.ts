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

/** Every command the frontend invokes: its arguments and its Ok value. Errors arrive as strings. */
export interface Commands {
  git_info: { args: Record<string, never>; result: GitInfo };
  repo_status: { args: { path: string }; result: Status };
  launch_path: { args: Record<string, never>; result: string | null };
  "plugin:dialog|open": { args: { options: { directory: boolean; title: string } }; result: string | null };
}

function call<K extends keyof Commands>(cmd: K, args?: Commands[K]["args"]): Promise<Commands[K]["result"]> {
  return invoke<Commands[K]["result"]>(cmd, args);
}

export const gitInfo = () => call("git_info");
export const repoStatus = (path: string) => call("repo_status", { path });
export const launchPath = () => call("launch_path");

/** Native folder picker; `null` when cancelled. */
export const pickFolder = () => open({ directory: true, title: "Open repository" });
