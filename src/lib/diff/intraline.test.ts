import { describe, expect, it } from "vitest";
import type { Hunk } from "$lib/api";
import { intralineDiff, pairHunkLines } from "./intraline";

describe("intralineDiff", () => {
  it("highlights only changed tokens and preserves full text on both sides", () => {
    const oldText = "var thrust = horsePower * 1.5;";
    const newText = "var thrust = torque * 2.0;";
    const { left, right } = intralineDiff(oldText, newText);
    expect(left.map((s) => s.text).join("")).toBe(oldText);
    expect(right.map((s) => s.text).join("")).toBe(newText);
    expect(left.filter((s) => s.changed).map((s) => s.text)).toEqual(["horsePower", "1", "5"]);
    expect(right.filter((s) => s.changed).map((s) => s.text)).toEqual(["torque", "2", "0"]);
  });

  it("caps long lines cleanly without throwing or altering text", () => {
    const a = "x".repeat(600) + "a";
    const b = "x".repeat(600) + "b";
    const { left, right } = intralineDiff(a, b);
    expect(left).toEqual([{ text: a, changed: false }]);
    expect(right).toEqual([{ text: b, changed: false }]);
  });
});

describe("pairHunkLines", () => {
  it("pairs equal-count removed/added runs inside a hunk", () => {
    const hunk: Hunk = {
      oldStart: 1,
      oldLines: 4,
      newStart: 1,
      newLines: 4,
      lines: [
        { kind: "context", old: 1, new: 1, text: "fn f() {" },
        { kind: "removed", old: 2, new: null, text: "  let x = 1;" },
        { kind: "added", old: null, new: 2, text: "  let x = 2;" },
        { kind: "context", old: 3, new: 3, text: "}" },
      ],
    };
    const pairs = pairHunkLines(hunk);
    expect(pairs.get(1)?.some((s) => s.changed && s.text === "1")).toBe(true);
    expect(pairs.get(2)?.some((s) => s.changed && s.text === "2")).toBe(true);
    expect(pairs.has(0)).toBe(false);
  });
});
