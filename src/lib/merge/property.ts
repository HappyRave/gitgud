import type { MergeRow, RowPick } from "$lib/api";

/** What the user chose for a row: a side, or a value typed in (E). */
export type Choice = "yours" | "theirs" | { custom: string };

export type Filter = "all" | "clashes" | "yours" | "theirs" | "objects";

/** One-sided changes come in applied; clashes wait for a pick (R9). */
export function defaultChoice(row: MergeRow): Choice | null {
  return row.source === "clash" ? null : row.source;
}

export function choiceOf(row: MergeRow, picks: ReadonlyMap<string, Choice>): Choice | null {
  return picks.get(row.id) ?? defaultChoice(row);
}

/** The result cell's text for `choice`. */
export function resultText(row: MergeRow, choice: Choice | null): string | null {
  if (choice === null) {
    return null;
  }
  if (typeof choice === "object") {
    return choice.custom;
  }
  if (row.kind === "object") {
    return objectResult(row, choice);
  }
  const v = choice === "yours" ? row.yours : row.theirs;
  return v ?? "—";
}

function objectResult(row: MergeRow, choice: "yours" | "theirs"): string {
  const said = choice === "yours" ? row.yours : row.theirs;
  if (said === "deleted") {
    return "deleted";
  }
  if (said?.startsWith("added")) {
    return "added";
  }
  return said === null ? "not added" : "kept";
}

export function matches(row: MergeRow, filter: Filter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "clashes":
      return row.source === "clash";
    case "objects":
      return row.kind === "object";
    default:
      return row.source === filter;
  }
}

/** Only the choices that differ from the default go to the backend. */
export function toPicks(rows: readonly MergeRow[], picks: ReadonlyMap<string, Choice>): RowPick[] {
  const out: RowPick[] = [];
  for (const row of rows) {
    const c = picks.get(row.id);
    if (c === undefined || c === defaultChoice(row)) {
      continue;
    }
    out.push(
      typeof c === "object"
        ? { id: row.id, choice: "custom", value: c.custom }
        : { id: row.id, choice: c, value: null },
    );
  }
  return out;
}

export function openClashes(rows: readonly MergeRow[], picks: ReadonlyMap<string, Choice>): number {
  return rows.filter((r) => r.source === "clash" && !picks.has(r.id)).length;
}
