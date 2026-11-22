import type { DiffLine, Hunk } from "$lib/api";

/** One row of a side-by-side diff; a side is `null` where it has no line. */
export interface SplitRow {
  left: DiffLine | null;
  right: DiffLine | null;
}

/**
 * Pairs a hunk's lines for a side-by-side view: context on both sides, and each
 * run of removed lines next to the added lines that follow it.
 */
export function splitRows(hunk: Hunk): SplitRow[] {
  const rows: SplitRow[] = [];
  let removed: DiffLine[] = [];
  let added: DiffLine[] = [];
  const flush = () => {
    for (let i = 0; i < Math.max(removed.length, added.length); i++) {
      rows.push({ left: removed[i] ?? null, right: added[i] ?? null });
    }
    removed = [];
    added = [];
  };
  for (const line of hunk.lines) {
    if (line.kind === "removed") {
      if (added.length) {
        flush();
      }
      removed.push(line);
    } else if (line.kind === "added") {
      added.push(line);
    } else {
      flush();
      rows.push({ left: line, right: line });
    }
  }
  flush();
  return rows;
}
