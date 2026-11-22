import type { RowLayout, Segment } from "$lib/api";
import { ROW_HEIGHT } from "./window";

export const LANE_WIDTH = 14;
/** Mirrors `gitgud_core::graph::MAX_LANES`: lanes beyond it fold into a marker. */
export const MAX_LANES = 32;
const DOT_RADIUS = 4;

/** PLACEHOLDER lane colours until the graph gets a design round. */
export const PALETTE = ["#4c8bf5", "#e8710a", "#1e8e3e", "#d93025", "#9334e6", "#12b5cb", "#e52592", "#7cb342"];

export const laneColor = (lane: number) => PALETTE[lane % PALETTE.length];
export const laneX = (lane: number) => LANE_WIDTH / 2 + lane * LANE_WIDTH;

export interface Line {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
}

/** A segment as a line in canvas space, for a row whose top edge is at `y`. */
export function segmentLine(s: Segment, y: number): Line {
  const top = y;
  const mid = y + ROW_HEIGHT / 2;
  const bottom = y + ROW_HEIGHT;
  switch (s.part) {
    case "top":
      return { x1: laneX(s.from), y1: top, x2: laneX(s.to), y2: mid, color: laneColor(s.from) };
    case "bottom":
      return { x1: laneX(s.from), y1: mid, x2: laneX(s.to), y2: bottom, color: laneColor(s.to) };
    case "through":
      return { x1: laneX(s.from), y1: top, x2: laneX(s.from), y2: bottom, color: laneColor(s.from) };
  }
}

/** Lanes a row needs drawn, so the graph column can be sized. */
export function laneCount(layout: RowLayout): number {
  let max = layout.node;
  for (const s of layout.segments) {
    max = Math.max(max, s.from, s.to);
  }
  return max + 1;
}

export interface DrawRow {
  index: number;
  layout: RowLayout;
  wip: boolean;
}

/** Draws the lane column for `rows` into a canvas showing the viewport at `scrollTop`. */
export function drawGraph(ctx: CanvasRenderingContext2D, rows: DrawRow[], scrollTop: number, width: number, height: number) {
  ctx.clearRect(0, 0, width, height);
  ctx.lineWidth = 2;
  ctx.lineCap = "round";
  for (const row of rows) {
    const y = row.index * ROW_HEIGHT - scrollTop;
    for (const s of row.layout.segments) {
      const l = segmentLine(s, y);
      ctx.strokeStyle = l.color;
      ctx.beginPath();
      ctx.moveTo(l.x1, l.y1);
      ctx.lineTo(l.x2, l.y2);
      ctx.stroke();
    }
    const cx = laneX(row.layout.node);
    const cy = y + ROW_HEIGHT / 2;
    ctx.beginPath();
    ctx.arc(cx, cy, DOT_RADIUS, 0, Math.PI * 2);
    if (row.wip) {
      ctx.fillStyle = "#fff";
      ctx.fill();
      ctx.strokeStyle = laneColor(row.layout.node);
      ctx.stroke();
    } else {
      ctx.fillStyle = laneColor(row.layout.node);
      ctx.fill();
    }
    if (row.layout.overflow) {
      ctx.fillStyle = "#888";
      ctx.fillText("›", width - 6, cy + 4);
    }
  }
}
