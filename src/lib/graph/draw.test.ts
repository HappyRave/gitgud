import { describe, expect, it } from "vitest";
import type { RowLayout } from "$lib/api";
import { fakeContext } from "$lib/test/canvas";
import { drawGraph, LANE_WIDTH, laneColor, laneCount, laneX, PALETTE, segmentLine } from "./draw";
import { ROW_HEIGHT } from "./window";

const mid = ROW_HEIGHT / 2;

describe("segmentLine", () => {
  it("draws a top segment from the row top into the dot, in the incoming lane's colour", () => {
    expect(segmentLine({ from: 2, to: 0, part: "top" }, 100)).toEqual({
      x1: laneX(2),
      y1: 100,
      x2: laneX(0),
      y2: 100 + mid,
      color: laneColor(2),
    });
  });

  it("draws a bottom segment from the dot to the row bottom, in the parent lane's colour", () => {
    expect(segmentLine({ from: 0, to: 1, part: "bottom" }, 0)).toEqual({
      x1: laneX(0),
      y1: mid,
      x2: laneX(1),
      y2: ROW_HEIGHT,
      color: laneColor(1),
    });
  });

  it("draws a through segment straight down its lane", () => {
    expect(segmentLine({ from: 3, to: 3, part: "through" }, 24)).toEqual({
      x1: laneX(3),
      y1: 24,
      x2: laneX(3),
      y2: 48,
      color: laneColor(3),
    });
  });
});

describe("lanes", () => {
  it("centres lanes in fixed-width columns", () => {
    expect(laneX(0)).toBe(LANE_WIDTH / 2);
    expect(laneX(1) - laneX(0)).toBe(LANE_WIDTH);
  });

  it("cycles the palette", () => {
    expect(laneColor(PALETTE.length)).toBe(laneColor(0));
  });

  it("counts the widest lane a row touches", () => {
    expect(laneCount({ node: 0, segments: [], overflow: false })).toBe(1);
    expect(laneCount({ node: 1, segments: [{ from: 1, to: 4, part: "bottom" }], overflow: false })).toBe(5);
  });
});

describe("drawGraph", () => {
  const layout = (node: number, overflow = false): RowLayout => ({
    node,
    segments: [{ from: node, to: node, part: "bottom" }],
    overflow,
  });

  it("draws each segment and a filled dot per row, offset by the scroll position", () => {
    const ctx = fakeContext();
    drawGraph(ctx, [{ index: 10, layout: layout(0), wip: false }], 10 * ROW_HEIGHT, 100, 200);

    const moves = ctx.calls.filter((c) => c.op === "moveTo");
    expect(moves).toEqual([expect.objectContaining({ args: [laneX(0), mid] })]);
    expect(ctx.calls.filter((c) => c.op === "arc")[0].args.slice(0, 2)).toEqual([laneX(0), mid]);
    expect(ctx.calls.filter((c) => c.op === "fill")).toHaveLength(1);
  });

  it("draws the working-copy dot hollow", () => {
    const ctx = fakeContext();
    drawGraph(ctx, [{ index: 0, layout: layout(0), wip: true }], 0, 100, 200);

    const fill = ctx.calls.find((c) => c.op === "fill");
    expect(fill?.fillStyle).toBe("#fff");
    expect(ctx.calls.filter((c) => c.op === "stroke")).toHaveLength(2);
  });

  it("marks rows with folded lanes", () => {
    const ctx = fakeContext();
    drawGraph(ctx, [{ index: 0, layout: layout(0, true), wip: false }], 0, 100, 200);

    expect(ctx.calls.find((c) => c.op === "fillText")?.args[0]).toBe("›");
  });

  it("clears the canvas first", () => {
    const ctx = fakeContext();
    drawGraph(ctx, [], 0, 100, 200);

    expect(ctx.calls).toEqual([expect.objectContaining({ op: "clearRect", args: [0, 0, 100, 200] })]);
  });
});
