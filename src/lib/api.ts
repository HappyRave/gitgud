import { invoke } from "@tauri-apps/api/core";

export interface GitInfo {
  path: string;
  version: string;
}

/** Every Rust command: its arguments and its Ok value. Errors arrive as strings. */
export interface Commands {
  git_info: { args: Record<string, never>; result: GitInfo };
}

function call<K extends keyof Commands>(cmd: K, args?: Commands[K]["args"]): Promise<Commands[K]["result"]> {
  return invoke<Commands[K]["result"]>(cmd, args);
}

export const gitInfo = () => call("git_info");
