import type { RowDiff, TableDiff } from "$lib/api";

/** A TableDiff from rows; counts are derived from the row states. */
export function tableDiff(columns: string[], rows: RowDiff[], over: Partial<TableDiff> = {}): TableDiff {
  const count = (s: RowDiff["state"]) => rows.filter((r) => r.state === s).length;
  const changedCells = rows
    .filter((r) => r.state === "changed")
    .reduce((n, r) => n + r.cells.filter(([o, n2]) => o !== null && n2 !== null && o !== n2).length, 0);
  return {
    name: "rows",
    columns,
    addedColumns: [],
    removedColumns: [],
    key: columns[0] ?? null,
    rows,
    rowCount: rows.filter((r) => r.state !== "removed").length,
    changed: count("changed"),
    changedCells,
    added: count("added"),
    removed: count("removed"),
    truncated: false,
    ...over,
  };
}

/** A row whose cells are `old→new` strings, or one value for both. */
export function row(state: RowDiff["state"], ...cells: string[]): RowDiff {
  return {
    state,
    cells: cells.map((c) => {
      const [o, n] = c.includes("→") ? c.split("→") : [c, c];
      return [state === "added" ? null : o, state === "removed" ? null : n];
    }),
  };
}
