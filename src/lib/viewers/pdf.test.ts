import { describe, expect, it } from "vitest";
import { changedRuns, matchPages, pageLabel, pdfCaption } from "./pdf";

describe("matchPages", () => {
  it("lines pages up by text so an inserted page shifts nothing", () => {
    const m = matchPages(["intro", "harbour 40s", "fuel", "end"], ["intro", "harbour 55s", "new page", "fuel", "end"]);
    expect(m.map((p) => [p.before, p.after, p.state])).toEqual([
      [0, 0, "same"],
      [1, 1, "changed"],
      [null, 2, "added"],
      [2, 3, "same"],
      [3, 4, "same"],
    ]);
  });

  it("pairs pages without text by position and flags them", () => {
    const m = matchPages(["", "a"], ["", "b", "c"]);
    expect(m[0]).toEqual({ before: 0, after: 0, state: "changed", noText: true });
    expect(m.map((p) => p.state)).toEqual(["changed", "changed", "added"]);
  });

  it("finds removed pages and ignores whitespace", () => {
    const m = matchPages(["a  b", "gone", "c"], ["a b", "c"]);
    expect(m.map((p) => p.state)).toEqual(["same", "removed", "same"]);
  });
});

it("marks runs the other page lacks, counting repeats", () => {
  expect([...changedRuns(["Two tugs", "turn back", "Docking", "x", "x"], ["Two tugs", "follow", "Docking", "x"])]).toEqual([1, 4]);
});

it("labels moved pages and writes the caption", () => {
  const m = matchPages(["a", "b", "c"], ["a", "new", "b", "c2"]);
  expect(m.map(pageLabel)).toEqual(["1", "2", "2→3", "3→4"]);
  expect(pdfCaption(m, 4, true)).toEqual(["4 pages", "1 changed (p.4)", "1 added (p.2)", "pages matched by their text"]);
  expect(pdfCaption(m, 4, false)).toEqual(["4 pages"]);
});
