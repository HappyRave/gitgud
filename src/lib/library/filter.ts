import type { LibraryFolder, LibraryRepo, RepoSummary } from "$lib/api";

export type SmartFilter = "all" | "recent" | "pinned" | "uncommitted" | "notPushed" | "locks" | "unity" | "missing";

export type LibrarySelection =
  | { kind: "smart"; filter: SmartFilter }
  | { kind: "folder"; folderId: string };

export interface FolderNode {
  folder: LibraryFolder;
  depth: number;
  directCount: number;
  totalCount: number;
}

export interface LibraryGroup {
  id: string;
  label: string;
  folderId: string | null;
  repos: LibraryRepo[];
  hiddenCount: number;
}

/** Case-insensitive normalized path comparison key matching gitgud-core. */
export function pathKey(path: string): string {
  return path.trim().replaceAll("\\", "/").replace(/\/+$/, "").toLowerCase();
}

export function folderPathLabel(folders: LibraryFolder[], folderId: string | null): string {
  if (!folderId) {
    return "Ungrouped";
  }
  const byId = new Map(folders.map((f) => [f.id, f]));
  const parts: string[] = [];
  let cur: string | null = folderId;
  let guard = 0;
  while (cur && guard++ <= folders.length) {
    const f = byId.get(cur);
    if (!f) {
      break;
    }
    parts.unshift(f.name);
    cur = f.parentId;
  }
  return parts.join(" / ") || "Ungrouped";
}

/** Recursively lists visible folder tree nodes with direct and descendant repo counts. */
export function buildFolderTree(
  folders: LibraryFolder[],
  repos: LibraryRepo[],
  collapsed: ReadonlySet<string>,
): FolderNode[] {
  const childrenByParent = new Map<string | null, LibraryFolder[]>();
  for (const f of folders) {
    const list = childrenByParent.get(f.parentId) ?? [];
    list.push(f);
    childrenByParent.set(f.parentId, list);
  }
  for (const list of childrenByParent.values()) {
    list.sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));
  }

  const direct = new Map<string, number>();
  for (const r of repos) {
    if (r.folderId) {
      direct.set(r.folderId, (direct.get(r.folderId) ?? 0) + 1);
    }
  }

  const totalCache = new Map<string, number>();
  function countTotal(id: string): number {
    const cached = totalCache.get(id);
    if (cached !== undefined) {
      return cached;
    }
    let sum = direct.get(id) ?? 0;
    for (const child of childrenByParent.get(id) ?? []) {
      sum += countTotal(child.id);
    }
    totalCache.set(id, sum);
    return sum;
  }

  const nodes: FolderNode[] = [];
  function walk(parentId: string | null, depth: number) {
    for (const f of childrenByParent.get(parentId) ?? []) {
      nodes.push({
        folder: f,
        depth,
        directCount: direct.get(f.id) ?? 0,
        totalCount: countTotal(f.id),
      });
      if (!collapsed.has(f.id)) {
        walk(f.id, depth + 1);
      }
    }
  }
  walk(null, 0);
  return nodes;
}

export function descendantFolderIds(folders: LibraryFolder[], rootId: string): Set<string> {
  const out = new Set<string>([rootId]);
  let added = true;
  while (added) {
    added = false;
    for (const f of folders) {
      if (f.parentId && out.has(f.parentId) && !out.has(f.id)) {
        out.add(f.id);
        added = true;
      }
    }
  }
  return out;
}

export function smartCounts(repos: LibraryRepo[]): Record<SmartFilter, number> {
  const counts: Record<SmartFilter, number> = {
    all: repos.length,
    recent: repos.filter((r) => r.lastOpened !== null || (r.cached?.lastActivity ?? null) !== null).length,
    pinned: repos.filter((r) => r.pinned).length,
    uncommitted: 0,
    notPushed: 0,
    locks: 0,
    unity: 0,
    missing: 0,
  };
  for (const r of repos) {
    const c = r.cached;
    if (!c) {
      continue;
    }
    if (c.missing) {
      counts.missing++;
    }
    if (c.dirtyCount > 0) {
      counts.uncommitted++;
    }
    if (c.ahead > 0) {
      counts.notPushed++;
    }
    if (c.locksHeld > 0) {
      counts.locks++;
    }
    if (c.isUnity) {
      counts.unity++;
    }
  }
  return counts;
}

function repoActivity(r: LibraryRepo): number {
  return Math.max(r.lastOpened ?? 0, r.cached?.lastActivity ?? 0);
}

function matchesSmartFilter(r: LibraryRepo, filter: SmartFilter): boolean {
  const c: RepoSummary | null = r.cached;
  switch (filter) {
    case "all":
      return true;
    case "recent":
      return repoActivity(r) > 0;
    case "pinned":
      return r.pinned;
    case "uncommitted":
      return (c?.dirtyCount ?? 0) > 0;
    case "notPushed":
      return (c?.ahead ?? 0) > 0;
    case "locks":
      return (c?.locksHeld ?? 0) > 0;
    case "unity":
      return c?.isUnity ?? false;
    case "missing":
      return c?.missing ?? false;
  }
}

export function matchesQuery(r: LibraryRepo, folders: LibraryFolder[], query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) {
    return true;
  }
  const folder = folderPathLabel(folders, r.folderId).toLowerCase();
  const haystack = [
    r.name,
    r.path,
    folder,
    r.cached?.branch ?? "",
    r.cached?.host ?? "",
  ]
    .join("\0")
    .toLowerCase();
  return q.split(/\s+/).every((part) => haystack.includes(part));
}

export function filterAndGroupRepos(
  folders: LibraryFolder[],
  repos: LibraryRepo[],
  selection: LibrarySelection,
  query: string,
  expandedGroups: ReadonlySet<string>,
  groupLimit = 10,
): { flat: LibraryRepo[]; groups: LibraryGroup[] } {
  const folderSet = selection.kind === "folder" ? descendantFolderIds(folders, selection.folderId) : null;
  const smartFilter = selection.kind === "smart" ? selection.filter : null;

  const filtered = repos.filter((r) => {
    if (folderSet) {
      if (!r.folderId || !folderSet.has(r.folderId)) {
        return false;
      }
    } else if (smartFilter && !matchesSmartFilter(r, smartFilter)) {
      return false;
    }
    return matchesQuery(r, folders, query);
  });

  if (selection.kind === "smart" && selection.filter === "recent") {
    const sorted = [...filtered].sort((a, b) => repoActivity(b) - repoActivity(a) || a.name.localeCompare(b.name));
    return {
      flat: sorted,
      groups: sorted.length > 0 ? [{ id: "__recent__", label: "Recent", folderId: null, repos: sorted, hiddenCount: 0 }] : [],
    };
  }

  const grouped = new Map<string, LibraryRepo[]>();
  for (const r of filtered) {
    const key = r.folderId ?? "__root__";
    const list = grouped.get(key) ?? [];
    list.push(r);
    grouped.set(key, list);
  }

  const groups: LibraryGroup[] = [];
  const hasQuery = query.trim().length > 0;
  for (const [key, list] of grouped) {
    const folderId = key === "__root__" ? null : key;
    const sorted = [...list].sort(
      (a, b) => repoActivity(b) - repoActivity(a) || a.order - b.order || a.name.localeCompare(b.name),
    );
    const expanded = hasQuery || expandedGroups.has(key) || grouped.size <= 1;
    const shown = expanded || sorted.length <= groupLimit ? sorted : sorted.slice(0, groupLimit);
    groups.push({
      id: key,
      label: folderPathLabel(folders, folderId),
      folderId,
      repos: shown,
      hiddenCount: sorted.length - shown.length,
    });
  }

  groups.sort((a, b) => {
    if (a.folderId === null && b.folderId !== null) return 1;
    if (b.folderId === null && a.folderId !== null) return -1;
    return a.label.localeCompare(b.label);
  });

  const flat = groups.flatMap((g) => g.repos);
  return { flat, groups };
}

/** Fuzzy score for Ctrl+P QuickSwitch (higher is better; null when no match). */
export function scoreRepoMatch(repo: LibraryRepo, folderLabel: string, query: string, isOpen: boolean): number | null {
  const q = query.trim().toLowerCase();
  if (!q) {
    return (isOpen ? 1000 : 0) + (repo.pinned ? 100 : 0) + Math.min(99, (repo.lastOpened ?? 0) > 0 ? 10 : 0);
  }
  const name = repo.name.toLowerCase();
  const path = repo.path.toLowerCase();
  const branch = (repo.cached?.branch ?? "").toLowerCase();
  const folder = folderLabel.toLowerCase();

  let score = 0;
  if (name === q) score += 2000;
  else if (name.startsWith(q)) score += 1200;
  else if (name.includes(q)) score += 800;
  else if (branch.includes(q)) score += 500;
  else if (folder.includes(q)) score += 350;
  else if (path.includes(q)) score += 250;
  else {
    // Subsequence match on repo name.
    let qi = 0;
    for (let i = 0; i < name.length && qi < q.length; i++) {
      if (name[i] === q[qi]) qi++;
    }
    if (qi < q.length) {
      return null;
    }
    score += 150;
  }
  if (isOpen) score += 80;
  if (repo.pinned) score += 40;
  return score;
}
