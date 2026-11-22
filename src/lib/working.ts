import type { Side, Status } from "$lib/api";

export interface FileRef {
  path: string;
  side: Side;
}

/** Files that open a diff, in the order the working-copy list shows them: staged, changes, untracked. */
export function fileOrder(status: Status): FileRef[] {
  const staged = status.entries.filter((e) => e.index).map((e) => ({ path: e.path, side: "staged" as const }));
  const changed = status.entries.filter((e) => e.worktree).map((e) => ({ path: e.path, side: "unstaged" as const }));
  const untracked = status.entries
    .filter((e) => e.kind === "untracked")
    .map((e) => ({ path: e.path, side: "unstaged" as const }));
  return [...staged, ...changed, ...untracked];
}

/** The file `delta` rows from `current` (clamped); the first file when nothing is selected. */
export function stepFile(order: FileRef[], current: FileRef | null, delta: number): FileRef | null {
  if (order.length === 0) {
    return null;
  }
  const at = current ? order.findIndex((f) => f.path === current.path && f.side === current.side) : -1;
  if (at < 0) {
    return order[0];
  }
  return order[Math.max(0, Math.min(order.length - 1, at + delta))];
}
