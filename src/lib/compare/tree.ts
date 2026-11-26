import type { Change, ComparePoint, Comparison, FileChange } from "$lib/api";
import type { ImageAt } from "$lib/images";

/** A folder of the compared files, with how many changed files it holds. */
export interface Folder {
  path: string;
  name: string;
  count: number;
  folders: Folder[];
  files: FileChange[];
}

export type TreeRow =
  | { kind: "dir"; path: string; name: string; depth: number; count: number; open: boolean }
  | { kind: "file"; path: string; name: string; depth: number; change: Change };

/** Folders sorted by name, then files by name; a folder with one subfolder and no files merges into it ("a/b"). */
export function buildTree(files: readonly FileChange[]): Folder {
  const root: Folder = { path: "", name: "", count: 0, folders: [], files: [] };
  const index = new Map<string, Folder>([["", root]]);
  for (const f of files) {
    const cut = f.path.lastIndexOf("/");
    const dir = cut < 0 ? "" : f.path.slice(0, cut);
    folderAt(index, dir).files.push(f);
  }
  count(root);
  sortAndMerge(root);
  return root;
}

function folderAt(index: Map<string, Folder>, path: string): Folder {
  const known = index.get(path);
  if (known) {
    return known;
  }
  const cut = path.lastIndexOf("/");
  const parent = folderAt(index, cut < 0 ? "" : path.slice(0, cut));
  const folder: Folder = { path, name: path.slice(cut + 1), count: 0, folders: [], files: [] };
  parent.folders.push(folder);
  index.set(path, folder);
  return folder;
}

function count(f: Folder): number {
  f.count = f.files.length + f.folders.reduce((n, sub) => n + count(sub), 0);
  return f.count;
}

function sortAndMerge(f: Folder) {
  f.folders.sort((a, b) => a.name.localeCompare(b.name));
  f.files.sort((a, b) => a.path.localeCompare(b.path));
  for (let i = 0; i < f.folders.length; i++) {
    let sub = f.folders[i];
    while (sub.files.length === 0 && sub.folders.length === 1) {
      const only = sub.folders[0];
      sub = { ...only, name: `${sub.name}/${only.name}` };
    }
    f.folders[i] = sub;
    sortAndMerge(sub);
  }
}

/** The rows to draw: open folders show their contents. */
export function treeRows(root: Folder, open: ReadonlySet<string>): TreeRow[] {
  const rows: TreeRow[] = [];
  const walk = (f: Folder, depth: number) => {
    for (const sub of f.folders) {
      const isOpen = open.has(sub.path);
      rows.push({ kind: "dir", path: sub.path, name: sub.name, depth, count: sub.count, open: isOpen });
      if (isOpen) {
        walk(sub, depth + 1);
      }
    }
    for (const file of f.files) {
      rows.push({ kind: "file", path: file.path, name: file.path.slice(file.path.lastIndexOf("/") + 1), depth, change: file.change });
    }
  };
  walk(root, 0);
  return rows;
}

/** Every folder path, for "expand all" on small comparisons. */
export function allFolders(root: Folder): string[] {
  const out: string[] = [];
  const walk = (f: Folder) => {
    for (const sub of f.folders) {
      out.push(sub.path);
      walk(sub);
    }
  };
  walk(root);
  return out;
}

/** Changed files per extension, most first: ".asset 15,126". `.meta` files are left out. */
export function kinds(files: readonly FileChange[]): { ext: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const f of files) {
    if (f.path.endsWith(".meta")) {
      continue;
    }
    const name = f.path.slice(f.path.lastIndexOf("/") + 1);
    const dot = name.lastIndexOf(".");
    const ext = dot > 0 ? name.slice(dot).toLowerCase() : "(none)";
    counts.set(ext, (counts.get(ext) ?? 0) + 1);
  }
  return [...counts].map(([ext, count]) => ({ ext, count })).sort((a, b) => b.count - a.count || a.ext.localeCompare(b.ext));
}

export const WORKING_COPY: ComparePoint = { kind: "worktree" };
export const rev = (r: string): ComparePoint => ({ kind: "rev", rev: r });

export function pointLabel(p: ComparePoint): string {
  return p.kind === "worktree" ? "Working copy" : p.rev;
}

export function samePoint(a: ComparePoint, b: ComparePoint): boolean {
  return a.kind === b.kind && (a.kind === "worktree" || a.rev === (b as { rev: string }).rev);
}

/** The two points file versions come from: A's side is the split point in "brings" mode. */
export function fileSides(c: Comparison, a: ComparePoint, b: ComparePoint): { from: ComparePoint; to: ComparePoint } {
  return {
    from: a.kind === "worktree" ? WORKING_COPY : rev(c.from),
    to: b.kind === "worktree" ? WORKING_COPY : rev(c.b),
  };
}

/** The image / file protocol's version for a side. */
export function imageAt(p: ComparePoint): ImageAt {
  return p.kind === "worktree" ? { at: "worktree" } : { at: "commit", commit: p.rev };
}
