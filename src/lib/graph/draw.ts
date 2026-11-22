import type { RowLayout, Segment } from "$lib/api";
import { ROW_HEIGHT } from "./window";

export const LANE_WIDTH = 14;
/** Mirrors `gitgud_core::graph::MAX_LANES`: lanes beyond it fold into a marker. */
export const MAX_LANES = 32;
const DOT_RADIUS = 4;

/** Canvas colours; the page reads them from the CSS custom properties so dark mode applies. */
export interface Theme {
  lanes: string[];
  /** Fill of the hollow working-copy dot. */
  panel: string;
  muted: string;
}

/** The lab mock's lane colours (`--l0`…`--l4` in app.css), light theme. */
export const PALETTE = ["#0072b2", "#d55e00", "#009e73", "#cc79a7", "#e69f00"];
export const DEFAULT_THEME: Theme = { lanes: PALETTE, panel: "#fff", muted: "#687386" };

/** The CSS variable for a lane, for HTML that should match the canvas. */
export const laneVar = (lane: number) => `var(--l${lane % PALETTE.length})`;

export const laneColor = (lane: number, theme = DEFAULT_THEME) => theme.lanes[lane % theme.lanes.length];
export const laneX = (lane: number) => LANE_WIDTH / 2 + lane * LANE_WIDTH;

/** Resolves the theme from the document's CSS variables, falling back to the light defaults. */
export function readTheme(el: Element): Theme {
  const css = getComputedStyle(el);
  const get = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  return {
    lanes: PALETTE.map((c, i) => get(`--l${i}`, c)),
    panel: get("--panel", DEFAULT_THEME.panel),
    muted: get("--muted", DEFAULT_THEME.muted),
  };
}

export interface Line {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
}

/** A segment as a line in canvas space, for a row whose top edge is at `y`. */
export function segmentLine(s: Segment, y: number, theme = DEFAULT_THEME, rowHeight = ROW_HEIGHT): Line {
  const top = y;
  const mid = y + rowHeight / 2;
  const bottom = y + rowHeight;
  switch (s.part) {
    case "top":
      return { x1: laneX(s.from), y1: top, x2: laneX(s.to), y2: mid, color: laneColor(s.from, theme) };
    case "bottom":
      return { x1: laneX(s.from), y1: mid, x2: laneX(s.to), y2: bottom, color: laneColor(s.to, theme) };
    case "through":
      return { x1: laneX(s.from), y1: top, x2: laneX(s.from), y2: bottom, color: laneColor(s.from, theme) };
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
  /** A planned commit of a history-plan preview: hollow dashed dot. */
  ghost?: boolean;
  /** A commit the plan replaces. */
  faded?: boolean;
  /** A stash entry: drawn as a hollow square so it stands out in grayscale too. */
  stash?: boolean;
}

const DASH = [3, 2];
const FADED_ALPHA = 0.45;

/** Draws the lane column for `rows` into a canvas showing the viewport at `scrollTop`. */
export function drawGraph(
  ctx: CanvasRenderingContext2D,
  rows: DrawRow[],
  scrollTop: number,
  width: number,
  height: number,
  theme = DEFAULT_THEME,
  rowHeight = ROW_HEIGHT,
) {
  ctx.clearRect(0, 0, width, height);
  ctx.lineWidth = 2;
  ctx.lineCap = "round";
  for (const row of rows) {
    const y = row.index * rowHeight - scrollTop;
    for (const s of row.layout.segments) {
      const l = segmentLine(s, y, theme, rowHeight);
      ctx.strokeStyle = l.color;
      ctx.setLineDash(s.stroke === "dashed" ? DASH : []);
      ctx.globalAlpha = s.stroke === "faded" ? FADED_ALPHA : 1;
      ctx.beginPath();
      ctx.moveTo(l.x1, l.y1);
      ctx.lineTo(l.x2, l.y2);
      ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.globalAlpha = row.faded ? FADED_ALPHA : 1;
    const cx = laneX(row.layout.node);
    const cy = y + rowHeight / 2;
    if (row.stash) {
      const s = DOT_RADIUS * 1.7;
      ctx.fillStyle = theme.panel;
      ctx.fillRect(cx - s / 2, cy - s / 2, s, s);
      ctx.strokeStyle = laneColor(row.layout.node, theme);
      ctx.strokeRect(cx - s / 2, cy - s / 2, s, s);
    } else {
      ctx.beginPath();
      ctx.arc(cx, cy, DOT_RADIUS, 0, Math.PI * 2);
      if (row.ghost) {
        ctx.fillStyle = theme.panel;
        ctx.fill();
        ctx.setLineDash([2, 2]);
        ctx.strokeStyle = laneColor(row.layout.node, theme);
        ctx.stroke();
        ctx.setLineDash([]);
      } else if (row.wip) {
        ctx.fillStyle = theme.panel;
        ctx.fill();
        ctx.strokeStyle = laneColor(row.layout.node, theme);
        ctx.stroke();
      } else {
        ctx.fillStyle = laneColor(row.layout.node, theme);
        ctx.fill();
      }
    }
    ctx.globalAlpha = 1;
    if (row.layout.overflow) {
      ctx.fillStyle = theme.muted;
      ctx.fillText("›", width - 6, cy + 4);
    }
  }
}
