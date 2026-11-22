import type { Chunk, Conflict, ConflictKind, ConflictText } from "$lib/api";

/** How one conflict hunk is resolved; `edited` holds the typed lines. */
export type Pick = { kind: "yours" } | { kind: "theirs" } | { kind: "both" } | { kind: "edited"; lines: string[] };

/** Index into `chunks` of every conflict hunk, in file order. */
export function conflictIndices(chunks: Chunk[]): number[] {
  return chunks.flatMap((c, i) => (c.kind === "conflict" ? [i] : []));
}

/** The lines a pick puts into the result. */
export function pickLines(chunk: Extract<Chunk, { kind: "conflict" }>, pick: Pick): string[] {
  switch (pick.kind) {
    case "yours":
      return chunk.yours;
    case "theirs":
      return chunk.theirs;
    case "both":
      return [...chunk.yours, ...chunk.theirs];
    case "edited":
      return pick.lines;
  }
}

/** The merged file (`\n` line ends), or `null` while a hunk has no pick. */
export function resultText(text: ConflictText, picks: ReadonlyMap<number, Pick>): string | null {
  const lines: string[] = [];
  for (const [i, chunk] of text.chunks.entries()) {
    if (chunk.kind === "same") {
      lines.push(...chunk.lines);
      continue;
    }
    const pick = picks.get(i);
    if (!pick) {
      return null;
    }
    lines.push(...pickLines(chunk, pick));
  }
  if (lines.length === 0) {
    return "";
  }
  return lines.join("\n") + (text.finalNewline ? "\n" : "");
}

/** The op-log label for a file's picks, e.g. "use both" or "2 yours, 1 theirs". */
export function describePicks(picks: Iterable<Pick>): string {
  const counts = new Map<Pick["kind"], number>();
  for (const p of picks) {
    counts.set(p.kind, (counts.get(p.kind) ?? 0) + 1);
  }
  const words: Record<Pick["kind"], string> = { yours: "yours", theirs: "theirs", both: "both", edited: "edited" };
  if (counts.size === 0) {
    return "as git merged it";
  }
  if (counts.size === 1) {
    const [kind] = counts.keys();
    return { yours: "keep yours", theirs: "take theirs", both: "use both", edited: "edited" }[kind];
  }
  return (["yours", "theirs", "both", "edited"] as const)
    .filter((k) => counts.has(k))
    .map((k) => `${counts.get(k)} ${words[k]}`)
    .join(", ");
}

/**
 * How much of an unchanged run to show around conflicts: `context` lines next
 * to each conflict, the middle folded when it would hide more than 2 lines.
 */
export function foldSame(
  count: number,
  conflictBefore: boolean,
  conflictAfter: boolean,
  context = 3,
): { head: number; hidden: number; tail: number } {
  const head = conflictBefore ? Math.min(context, count) : 0;
  const tail = conflictAfter ? Math.min(context, count - head) : 0;
  const hidden = count - head - tail;
  if (hidden <= 2) {
    return { head: count, hidden: 0, tail: 0 };
  }
  return { head, hidden, tail };
}

export const KIND_LABEL: Record<ConflictKind, string> = {
  unityYaml: "Unity YAML",
  code: "Code",
  binary: "Binary",
};

/** Conflicts grouped by kind, in list order; empty groups left out. */
export function groupByKind(conflicts: Conflict[]): { kind: ConflictKind; conflicts: Conflict[] }[] {
  return (["unityYaml", "code", "binary"] as const)
    .map((kind) => ({ kind, conflicts: conflicts.filter((c) => c.kind === kind) }))
    .filter((g) => g.conflicts.length > 0);
}

/** The prefilled merge message: git's subject, then what was picked per file. */
export function mergeMessage(gitMessage: string, conflicts: Conflict[], how: ReadonlyMap<string, string>): string {
  if (conflicts.length === 0) {
    return gitMessage;
  }
  const lines = conflicts.map((c) => `- ${c.path}: ${how.get(c.path) ?? "resolved"}`);
  return `${gitMessage}\n\nConflicts resolved:\n${lines.join("\n")}`;
}

/** e.g. 1536 → "1.5 KB". */
export function formatSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB"];
  let n = bytes / 1024;
  let u = 0;
  while (n >= 1024 && u < units.length - 1) {
    n /= 1024;
    u++;
  }
  return `${n.toFixed(n < 10 ? 1 : 0)} ${units[u]}`;
}
