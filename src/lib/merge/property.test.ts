import { describe, expect, it } from "vitest";
import type { MergeRow } from "$lib/api";
import { type Choice, choiceOf, defaultChoice, matches, openClashes, resultText, toPicks } from "./property";

function row(over: Partial<MergeRow>): MergeRow {
  return {
    id: "102/m_Range",
    kind: "value",
    source: "theirs",
    fileId: "102",
    objectPath: "Lighthouse",
    component: "Light",
    property: "m_Range",
    base: "24",
    yours: "24",
    theirs: "30",
    yoursRaw: " 24",
    theirsRaw: " 30",
    editable: true,
    yoursDetail: [],
    theirsDetail: [],
    ...over,
  };
}

describe("property merge choices", () => {
  it("applies one-sided changes and leaves clashes open", () => {
    expect(defaultChoice(row({}))).toBe("theirs");
    expect(defaultChoice(row({ source: "clash" }))).toBeNull();
    expect(choiceOf(row({ source: "clash" }), new Map([["102/m_Range", "yours"]]))).toBe("yours");
  });

  it("shows the result of a choice", () => {
    expect(resultText(row({}), "theirs")).toBe("30");
    expect(resultText(row({}), { custom: "27" })).toBe("27");
    expect(resultText(row({ source: "clash" }), null)).toBeNull();
    const deleted = row({ kind: "object", yours: "deleted", theirs: "kept, 1 change" });
    expect(resultText(deleted, "yours")).toBe("deleted");
    expect(resultText(deleted, "theirs")).toBe("kept");
    const added = row({ kind: "object", yours: null, theirs: "added: 1 object" });
    expect(resultText(added, "theirs")).toBe("added");
    expect(resultText(added, "yours")).toBe("not added");
  });

  it("filters by source and kind", () => {
    const rows = [row({}), row({ id: "a", source: "clash" }), row({ id: "b", kind: "object", source: "yours" })];
    expect(rows.filter((r) => matches(r, "clashes")).map((r) => r.id)).toEqual(["a"]);
    expect(rows.filter((r) => matches(r, "yours")).map((r) => r.id)).toEqual(["b"]);
    expect(rows.filter((r) => matches(r, "objects")).map((r) => r.id)).toEqual(["b"]);
    expect(rows.filter((r) => matches(r, "all"))).toHaveLength(3);
  });

  it("sends only choices that differ from the default", () => {
    const rows = [row({}), row({ id: "c", source: "clash" }), row({ id: "y", source: "yours" })];
    const picks = new Map<string, Choice>([
      ["102/m_Range", "theirs"],
      ["c", { custom: "2" }],
      ["y", "theirs"],
    ]);
    expect(toPicks(rows, picks)).toEqual([
      { id: "c", choice: "custom", value: "2" },
      { id: "y", choice: "theirs", value: null },
    ]);
    expect(openClashes(rows, new Map())).toBe(1);
    expect(openClashes(rows, picks)).toBe(0);
  });
});
