import { describe, expect, it } from "vitest";
import type { DiffLine, Hunk } from "$lib/api";
import { splitRows } from "./split";

const ctx = (n: number): DiffLine => ({ kind: "context", old: n, new: n, text: `c${n}` });
const del = (n: number): DiffLine => ({ kind: "removed", old: n, new: null, text: `d${n}` });
const add = (n: number): DiffLine => ({ kind: "added", old: null, new: n, text: `a${n}` });
const hunk = (lines: DiffLine[]): Hunk => ({ oldStart: 1, oldLines: 0, newStart: 1, newLines: 0, lines });
const show = (h: Hunk) => splitRows(h).map((r) => `${r.left?.text ?? "-"}|${r.right?.text ?? "-"}`);

describe("splitRows", () => {
  it("puts context on both sides and pairs a change's removed and added lines", () => {
    expect(show(hunk([ctx(1), del(2), del(3), add(2), ctx(4)]))).toEqual(["c1|c1", "d2|a2", "d3|-", "c4|c4"]);
  });

  it("leaves the other side empty for pure additions and deletions", () => {
    expect(show(hunk([add(1), add(2)]))).toEqual(["-|a1", "-|a2"]);
    expect(show(hunk([del(1)]))).toEqual(["d1|-"]);
  });

  it("starts a new pair when removals follow additions", () => {
    expect(show(hunk([del(1), add(1), del(2), add(2), add(3)]))).toEqual(["d1|a1", "d2|a2", "-|a3"]);
  });
});
