import type { DiffLine, Hunk } from "$lib/api";

/** One row of a side-by-side diff; a side is `null` where it has no line. */
export interface SplitRow {
  left: DiffLine | null;
  right: DiffLine | null;
  leftIndex: number | null;
  rightIndex: number | null;
}

/**
 * Pairs a hunk's lines for a side-by-side view: context on both sides, and each
 * run of removed lines next to the added lines that follow it.
 */
export function splitRows(hunk: Hunk): SplitRow[] {
  const rows: SplitRow[] = [];
  let removed: { line: DiffLine; index: number }[] = [];
  let added: { line: DiffLine; index: number }[] = [];
  const flush = () => {
    for (let i = 0; i < Math.max(removed.length, added.length); i++) {
      rows.push({
        left: removed[i]?.line ?? null,
        right: added[i]?.line ?? null,
        leftIndex: removed[i]?.index ?? null,
        rightIndex: added[i]?.index ?? null,
      });
    }
    removed = [];
    added = [];
  };
  for (let idx = 0; idx < hunk.lines.length; idx++) {
    const line = hunk.lines[idx];
    if (line.kind === "removed") {
      if (added.length) {
        flush();
      }
      removed.push({ line, index: idx });
    } else if (line.kind === "added") {
      added.push({ line, index: idx });
    } else {
      flush();
      rows.push({ left: line, right: line, leftIndex: idx, rightIndex: idx });
    }
  }
  flush();
  return rows;
}
