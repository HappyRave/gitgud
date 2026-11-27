import type { LfsPlan, PartialState, SkippedFile } from "$lib/api";

/** The listed folder a skipped file counts under: up to three levels, as the plan groups them. */
export function folderOf(path: string): string {
  const parts = path.split("/");
  const depth = Math.min(parts.length - 1, 3);
  return depth === 0 ? "(top level)" : parts.slice(0, depth).join("/");
}

/** Skipped files per listed folder. */
export function byFolder(files: SkippedFile[]): Map<string, SkippedFile[]> {
  const out = new Map<string, SkippedFile[]>();
  for (const f of files) {
    const folder = folderOf(f.path);
    const list = out.get(folder);
    if (list) {
      list.push(f);
    } else {
      out.set(folder, [f]);
    }
  }
  return out;
}

export interface TreeNode {
  /** Repo-relative path: a folder's or a file's. */
  path: string;
  name: string;
  folder: boolean;
  bytes: number;
  files: number;
  /** Bytes of it downloaded now. */
  onDiskBytes: number;
}

/** The direct children of `base` among `files` (all under it): folders first, then biggest first. */
export function childrenOf(files: SkippedFile[], base: string): TreeNode[] {
  const prefix = base === "(top level)" ? "" : `${base}/`;
  const folders = new Map<string, TreeNode>();
  const leaves: TreeNode[] = [];
  for (const f of files) {
    const rest = f.path.slice(prefix.length);
    const slash = rest.indexOf("/");
    const onDiskBytes = f.onDisk ? f.bytes : 0;
    if (slash < 0) {
      leaves.push({ path: f.path, name: rest, folder: false, bytes: f.bytes, files: 1, onDiskBytes });
      continue;
    }
    const name = rest.slice(0, slash);
    const node = folders.get(name);
    if (node) {
      node.bytes += f.bytes;
      node.files++;
      node.onDiskBytes += onDiskBytes;
    } else {
      folders.set(name, { path: prefix + name, name, folder: true, bytes: f.bytes, files: 1, onDiskBytes });
    }
  }
  const big = (a: TreeNode, b: TreeNode) => b.bytes - a.bytes || a.name.localeCompare(b.name);
  return [...[...folders.values()].sort(big), ...leaves.sort(big)];
}

/** `files` inside folder `path` (a child folder from `childrenOf`). */
export const under = (files: SkippedFile[], path: string) => files.filter((f) => f.path.startsWith(`${path}/`));

/** Bytes a plan keeps on disk: what the scenes use, what Unity always loads, what was kept. */
export function wantedBytes(plan: LfsPlan): number {
  return plan.used.bytes + plan.always.bytes + plan.kept.bytes;
}

/** What applying `plan` does to the disk: download the missing part, or free what goes. */
export function applyEffect(plan: LfsPlan): { kind: "download" | "free" | "none"; bytes: number } {
  const delta = wantedBytes(plan) - plan.onDisk.bytes;
  if (delta > 0) {
    return { kind: "download", bytes: delta };
  }
  if (delta < 0) {
    return { kind: "free", bytes: -delta };
  }
  return { kind: "none", bytes: 0 };
}

export interface Segment {
  kind: "used" | "always" | "kept" | "skipped";
  label: string;
  bytes: number;
  files: number;
  /** Width in percent of the total. */
  pct: number;
  /** What it means, for the bar's hover box. */
  about: string;
}

/** The bar's segments with what each means; `fmt` formats bytes. */
export function segments(plan: LfsPlan, fmt: (bytes: number) => string): Segment[] {
  const [used, always, kept, skipped] = shares(plan);
  const onDisk = plan.skippedOnDisk.bytes;
  return [
    {
      kind: "used",
      label: "Used by the scenes",
      ...plan.used,
      pct: used,
      about: "Referenced by the chosen scenes, directly or through prefabs, materials and textures. Stays on disk.",
    },
    {
      kind: "always",
      label: "Always kept",
      ...plan.always,
      pct: always,
      about: "Unity loads these without a reference: code, DLLs, Resources, Editor, StreamingAssets, everything outside Assets/.",
    },
    { kind: "kept", label: "Kept by you", ...plan.kept, pct: kept, about: "Folders and files you chose to keep downloaded." },
    {
      kind: "skipped",
      label: "Not used by these scenes",
      ...plan.skipped,
      pct: skipped,
      about:
        onDisk > 0
          ? `${fmt(onDisk)} of it is on disk now. Apply turns it back into pointer files; the content stays in .git/lfs until pruned.`
          : "Left as pointer files, not downloaded. Download a folder below to keep it.",
    },
  ];
}

/** Bar segment widths in percent: used, always, kept, not downloaded. */
export function shares(plan: LfsPlan): [number, number, number, number] {
  const total = plan.total.bytes || 1;
  const pct = (b: number) => (100 * b) / total;
  return [pct(plan.used.bytes), pct(plan.always.bytes), pct(plan.kept.bytes), pct(plan.skipped.bytes)];
}

export function sameState(a: PartialState, b: PartialState): boolean {
  return (
    a.buildScenes === b.buildScenes &&
    a.scenes.length === b.scenes.length &&
    a.scenes.every((s, i) => s === b.scenes[i]) &&
    a.kept.length === b.kept.length &&
    a.kept.every((k, i) => k === b.kept[i])
  );
}

/** `state` with `path` added to its scenes, once. */
export function withScene(state: PartialState, path: string): PartialState {
  const p = path.trim();
  return !p || state.scenes.includes(p) ? state : { ...state, scenes: [...state.scenes, p] };
}

export function withoutScene(state: PartialState, path: string): PartialState {
  return { ...state, scenes: state.scenes.filter((s) => s !== path) };
}

export function withoutKept(state: PartialState, path: string): PartialState {
  return { ...state, kept: state.kept.filter((k) => k !== path) };
}
