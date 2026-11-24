import type { TableDiff } from "$lib/api";

export type RowFilter = "changed" | "all";
export const ROW_FILTERS: readonly RowFilter[] = ["changed", "all"];

/** "48 rows · 2 changed (4 cells) · 1 added · 1 removed · rows matched by id". */
export function tableCaption(t: TableDiff, compare: boolean): { text: string; tone: "" | "mod" | "add" | "del" }[] {
  const out: { text: string; tone: "" | "mod" | "add" | "del" }[] = [{ text: `${t.rowCount.toLocaleString("en-US")} rows`, tone: "" }];
  if (!compare) {
    return out;
  }
  if (t.changed) {
    out.push({ text: `${t.changed} changed (${t.changedCells} ${t.changedCells === 1 ? "cell" : "cells"})`, tone: "mod" });
  }
  if (t.added) {
    out.push({ text: `${t.added} added`, tone: "add" });
  }
  if (t.removed) {
    out.push({ text: `${t.removed} removed`, tone: "del" });
  }
  if (!t.changed && !t.added && !t.removed) {
    out.push({ text: "no row changed", tone: "" });
  }
  for (const c of t.addedColumns) {
    out.push({ text: `column +${c}`, tone: "add" });
  }
  for (const c of t.removedColumns) {
    out.push({ text: `column −${c}`, tone: "del" });
  }
  const first = t.key !== null && t.columns[0] === t.key;
  out.push({
    text: t.key === null ? "rows matched by position (the key column repeats)" : `rows matched by ${t.key}${first ? " (first column)" : ""}`,
    tone: "",
  });
  return out;
}
