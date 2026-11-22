import { describe, expect, it } from "vitest";
import type { Chunk, ConflictText } from "$lib/api";
import {
  conflictIndices,
  describePicks,
  foldSame,
  formatSize,
  groupByKind,
  mergeMessage,
  resultText,
  type Pick,
} from "./resolve";

const conflict = (yours: string[], theirs: string[], base: string[] = []): Chunk => ({
  kind: "conflict",
  yours,
  base,
  theirs,
});
const TEXT: ConflictText = {
  finalNewline: true,
  chunks: [
    { kind: "same", lines: ["a"] },
    conflict(["y1"], ["t1"]),
    { kind: "same", lines: ["b"] },
    conflict(["y2"], ["t2", "t2b"]),
  ],
};
const picks = (entries: [number, Pick][]) => new Map(entries);

describe("resultText", () => {
  it("is null until every conflict has a pick", () => {
    expect(resultText(TEXT, picks([[1, { kind: "yours" }]]))).toBeNull();
  });

  it("joins same lines with each pick's lines", () => {
    const text = resultText(
      TEXT,
      picks([
        [1, { kind: "both" }],
        [3, { kind: "theirs" }],
      ]),
    );
    expect(text).toBe("a\ny1\nt1\nb\nt2\nt2b\n");
  });

  it("uses edited lines and respects a missing final newline", () => {
    const text = resultText(
      { ...TEXT, finalNewline: false },
      picks([
        [1, { kind: "edited", lines: ["mine"] }],
        [3, { kind: "edited", lines: [] }],
      ]),
    );
    expect(text).toBe("a\nmine\nb");
  });

  it("gives an empty file when nothing is left", () => {
    const text = resultText({ finalNewline: true, chunks: [conflict(["x"], [])] }, picks([[0, { kind: "theirs" }]]));
    expect(text).toBe("");
  });
});

describe("foldSame", () => {
  it("keeps context next to conflicts and folds the middle", () => {
    expect(foldSame(20, true, true)).toEqual({ head: 3, hidden: 14, tail: 3 });
    expect(foldSame(20, false, true)).toEqual({ head: 0, hidden: 17, tail: 3 });
    expect(foldSame(20, true, false)).toEqual({ head: 3, hidden: 17, tail: 0 });
  });

  it("shows short runs whole instead of folding a line or two", () => {
    expect(foldSame(8, true, true)).toEqual({ head: 8, hidden: 0, tail: 0 });
    expect(foldSame(2, false, true)).toEqual({ head: 2, hidden: 0, tail: 0 });
  });
});

describe("conflictIndices", () => {
  it("lists the conflict chunks", () => {
    expect(conflictIndices(TEXT.chunks)).toEqual([1, 3]);
  });
});

describe("describePicks", () => {
  it("names a single kind of pick", () => {
    expect(describePicks([{ kind: "yours" }, { kind: "yours" }])).toBe("keep yours");
    expect(describePicks([{ kind: "theirs" }])).toBe("take theirs");
    expect(describePicks([{ kind: "both" }])).toBe("use both");
    expect(describePicks([{ kind: "edited", lines: [] }])).toBe("edited");
  });

  it("names a file with no hunks to pick", () => {
    expect(describePicks([])).toBe("as git merged it");
  });

  it("counts mixed picks", () => {
    expect(describePicks([{ kind: "theirs" }, { kind: "yours" }, { kind: "yours" }])).toBe("2 yours, 1 theirs");
  });
});

describe("groupByKind", () => {
  it("orders Unity, code, binary and drops empty groups", () => {
    const groups = groupByKind([
      { path: "b.png", kind: "binary", resolved: false },
      { path: "a.cs", kind: "code", resolved: true },
    ]);
    expect(groups.map((g) => [g.kind, g.conflicts.map((c) => c.path)])).toEqual([
      ["code", ["a.cs"]],
      ["binary", ["b.png"]],
    ]);
  });
});

describe("mergeMessage", () => {
  it("adds what was picked per file to git's message", () => {
    const message = mergeMessage(
      "Merge branch 'other'",
      [
        { path: "a.cs", kind: "code", resolved: true },
        { path: "t.png", kind: "binary", resolved: true },
      ],
      new Map([["a.cs", "use both"]]),
    );
    expect(message).toBe("Merge branch 'other'\n\nConflicts resolved:\n- a.cs: use both\n- t.png: resolved");
  });

  it("leaves a merge without conflicts alone", () => {
    expect(mergeMessage("Merge", [], new Map())).toBe("Merge");
  });
});

describe("formatSize", () => {
  it("formats bytes", () => {
    expect(formatSize(12)).toBe("12 B");
    expect(formatSize(1536)).toBe("1.5 KB");
    expect(formatSize(2_076_180)).toBe("2.0 MB");
    expect(formatSize(300 * 1024 * 1024)).toBe("300 MB");
  });
});
