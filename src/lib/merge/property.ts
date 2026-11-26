import type { MergeRow, RowPick } from "$lib/api";
import type { MergeSource } from "$lib/model/compare";

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

const same = (a: Choice | null, b: Choice | null) =>
  typeof a === "object" && a !== null && typeof b === "object" && b !== null ? a.custom === b.custom : a === b;

/** How many rows' choices differ between `built` (what the 3D result shows) and `now`. */
export function picksSince(
  rows: readonly MergeRow[],
  built: ReadonlyMap<string, Choice>,
  now: ReadonlyMap<string, Choice>,
): number {
  return rows.filter((r) => !same(choiceOf(r, built), choiceOf(r, now))).length;
}

/**
 * Each changed object's 3D name path and the tint it gets in the result: clashes as
 * clashes, every other change by the side its value was taken from.
 */
export function resultSources(
  rows: readonly MergeRow[],
  picks: ReadonlyMap<string, Choice>,
): Map<string, MergeSource> {
  const out = new Map<string, MergeSource>();
  for (const row of rows) {
    if (!row.objectPath || row.objectPath === "(file)") {
      continue;
    }
    const c = choiceOf(row, picks);
    const tint: MergeSource =
      row.source === "clash" ? "clash" : c === "yours" || c === "theirs" ? c : row.source;
    const key = row.objectPath.split(" / ").join("/");
    if (out.get(key) !== "clash") {
      out.set(key, tint);
    }
  }
  return out;
}

export function openClashes(rows: readonly MergeRow[], picks: ReadonlyMap<string, Choice>): number {
  return rows.filter((r) => r.source === "clash" && !picks.has(r.id)).length;
}
