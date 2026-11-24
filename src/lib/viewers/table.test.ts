import { describe, expect, it } from "vitest";
import { row, tableDiff } from "$lib/test/table";
import { tableCaption } from "./table";

describe("tableCaption", () => {
  it("counts changes and says how rows were matched", () => {
    const t = tableDiff(["id", "speed", "cost"], [row("changed", "tug", "7", "800→650"), row("added", "foil", "22", "2400"), row("removed", "raft", "4", "0")], {
      addedColumns: ["cost"],
    });
    expect(tableCaption(t, true).map((p) => p.text)).toEqual([
      "2 rows",
      "1 changed (1 cell)",
      "1 added",
      "1 removed",
      "column +cost",
      "rows matched by id (first column)",
    ]);
  });

  it("says when rows were matched by position, and nothing for a lone version", () => {
    const t = tableDiff(["kind"], [row("same", "boat")], { key: null });
    expect(tableCaption(t, true).at(-1)?.text).toBe("rows matched by position (the key column repeats)");
    expect(tableCaption(t, true)[1].text).toBe("no row changed");
    expect(tableCaption(t, false).map((p) => p.text)).toEqual(["1 rows"]);
  });
});
