import type { Branch } from "$lib/api";

export type Node =
  | { kind: "folder"; name: string; path: string; children: Node[] }
  | { kind: "branch"; name: string; branch: Branch };

/**
 * Branches grouped into folders by `/` (`feature/login` → `feature/` › `login`).
 * Each level lists plain branches first, then folders, both by name (R1-A mock).
 */
export function branchTree(branches: Branch[]): Node[] {
  return level(branches.map((b) => ({ rest: b.name.split("/"), branch: b })), "");
}

function level(items: { rest: string[]; branch: Branch }[], prefix: string): Node[] {
  const leaves: Node[] = [];
  const folders = new Map<string, { rest: string[]; branch: Branch }[]>();
  for (const item of items) {
    const [head, ...tail] = item.rest;
    if (tail.length === 0) {
      leaves.push({ kind: "branch", name: head, branch: item.branch });
    } else {
      folders.set(head, [...(folders.get(head) ?? []), { rest: tail, branch: item.branch }]);
    }
  }
  leaves.sort((a, b) => a.name.localeCompare(b.name));
  const folderNodes: Node[] = [...folders.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([name, children]) => ({
      kind: "folder" as const,
      name,
      path: `${prefix}${name}/`,
      children: level(children, `${prefix}${name}/`),
    }));
  return [...leaves, ...folderNodes];
}

/** The rows to draw, depth-first, skipping the inside of collapsed folders. */
export function visibleRows(nodes: Node[], collapsed: ReadonlySet<string>, depth = 0): { node: Node; depth: number }[] {
  return nodes.flatMap((node) => [
    { node, depth },
    ...(node.kind === "folder" && !collapsed.has(node.path) ? visibleRows(node.children, collapsed, depth + 1) : []),
  ]);
}

/** "↑2 ↓1", or "" when level with the upstream or there is none. */
export function aheadBehind(b: Branch): string {
  return [b.ahead ? `↑${b.ahead}` : "", b.behind ? `↓${b.behind}` : ""].filter(Boolean).join(" ");
}
