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

export interface FileSelection {
  active: FileRef | null;
  anchor: FileRef | null;
  keys: Set<string>;
}

export function fileKey(path: string, side: Side): string {
  return `${side}:${path}`;
}

export function singleFileSelection(file: FileRef | null): FileSelection {
  if (!file) {
    return { active: null, anchor: null, keys: new Set() };
  }
  return { active: file, anchor: file, keys: new Set([fileKey(file.path, file.side)]) };
}

export function selectedPathsForSide(order: FileRef[], keys: ReadonlySet<string>, side: Side): string[] {
  return order.filter((f) => f.side === side && keys.has(fileKey(f.path, side))).map((f) => f.path);
}

export function applyFileClick(
  order: FileRef[],
  current: FileSelection,
  target: FileRef,
  mods?: { ctrl?: boolean; shift?: boolean },
): FileSelection {
  const key = fileKey(target.path, target.side);
  const ctrl = Boolean(mods?.ctrl);
  const shift = Boolean(mods?.shift);

  if (!ctrl && !shift) {
    return { active: target, anchor: target, keys: new Set([key]) };
  }

  const sameSide = current.active?.side === target.side;
  if (shift) {
    const anchor =
      current.anchor &&
      current.anchor.side === target.side &&
      order.some((f) => f.path === current.anchor!.path && f.side === current.anchor!.side)
        ? current.anchor
        : sameSide && current.active
          ? current.active
          : target;
    const aIdx = order.findIndex((f) => f.path === anchor.path && f.side === anchor.side);
    const tIdx = order.findIndex((f) => f.path === target.path && f.side === target.side);
    const next = ctrl && sameSide ? new Set(current.keys) : new Set<string>();
    if (aIdx >= 0 && tIdx >= 0) {
      const [lo, hi] = aIdx <= tIdx ? [aIdx, tIdx] : [tIdx, aIdx];
      for (let i = lo; i <= hi; i++) {
        if (order[i].side === target.side) {
          next.add(fileKey(order[i].path, order[i].side));
        }
      }
    } else {
      next.add(key);
    }
    return { active: target, anchor, keys: next };
  }

  if (!sameSide || current.keys.size === 0) {
    return { active: target, anchor: target, keys: new Set([key]) };
  }

  const next = new Set(current.keys);
  if (next.has(key)) {
    next.delete(key);
    if (next.size === 0) {
      return { active: null, anchor: null, keys: next };
    }
    const remaining = order.filter((f) => f.side === target.side && next.has(fileKey(f.path, f.side)));
    const wasActive = current.active?.path === target.path && current.active?.side === target.side;
    const active = wasActive ? (remaining[remaining.length - 1] ?? null) : current.active;
    const anchor =
      current.anchor && current.anchor.path === target.path && current.anchor.side === target.side ? active : current.anchor;
    return { active, anchor, keys: next };
  }

  next.add(key);
  return { active: target, anchor: target, keys: next };
}

export function extendFileSelection(order: FileRef[], current: FileSelection, delta: -1 | 1): FileSelection {
  if (order.length === 0) {
    return { active: null, anchor: null, keys: new Set() };
  }
  if (!current.active) {
    return singleFileSelection(order[0]);
  }
  const side = current.active.side;
  const sideOrder = order.filter((f) => f.side === side);
  const idx = sideOrder.findIndex((f) => f.path === current.active!.path);
  if (idx < 0) {
    return singleFileSelection(order[0]);
  }
  const nextIdx = Math.max(0, Math.min(sideOrder.length - 1, idx + delta));
  const target = sideOrder[nextIdx];
  const anchor =
    current.anchor && current.anchor.side === side && sideOrder.some((f) => f.path === current.anchor!.path)
      ? current.anchor
      : current.active;
  const aIdx = sideOrder.findIndex((f) => f.path === anchor.path);
  const [lo, hi] = aIdx <= nextIdx ? [aIdx, nextIdx] : [nextIdx, aIdx];
  const keys = new Set<string>();
  for (let i = lo; i <= hi; i++) {
    keys.add(fileKey(sideOrder[i].path, side));
  }
  return { active: target, anchor, keys };
}
