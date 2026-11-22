import { describe, expect, it } from "vitest";
import { CHUNK, chunksFor, ROW_HEIGHT, visibleRange } from "./window";

describe("visibleRange", () => {
  it("covers the viewport plus overscan at the top", () => {
    expect(visibleRange(0, 10 * ROW_HEIGHT, 1000, 5)).toEqual({ start: 0, end: 15 });
  });

  it("follows the scroll position", () => {
    expect(visibleRange(100 * ROW_HEIGHT, 10 * ROW_HEIGHT, 1000, 5)).toEqual({ start: 95, end: 115 });
  });

  it("counts a partly visible row", () => {
    expect(visibleRange(ROW_HEIGHT / 2, 2 * ROW_HEIGHT, 1000, 0)).toEqual({ start: 0, end: 3 });
  });

  it("clamps to the row count", () => {
    expect(visibleRange(995 * ROW_HEIGHT, 10 * ROW_HEIGHT, 1000, 5)).toEqual({ start: 990, end: 1000 });
    expect(visibleRange(0, 500, 0)).toEqual({ start: 0, end: 0 });
  });

  it("is empty when scrolled past the end or with no height", () => {
    expect(visibleRange(5000 * ROW_HEIGHT, 480, 10, 0)).toEqual({ start: 10, end: 10 });
    expect(visibleRange(0, 0, 100, 0)).toEqual({ start: 0, end: 0 });
  });
});

describe("chunksFor", () => {
  it("lists every chunk the range touches", () => {
    expect(chunksFor({ start: 0, end: 1 })).toEqual([0]);
    expect(chunksFor({ start: CHUNK - 1, end: CHUNK + 1 })).toEqual([0, 1]);
    expect(chunksFor({ start: CHUNK, end: 3 * CHUNK })).toEqual([1, 2]);
  });

  it("is empty for an empty range", () => {
    expect(chunksFor({ start: 5, end: 5 })).toEqual([]);
  });
});
