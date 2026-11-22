/** Standard row height (R1: 24 px; Compact is 20 px, not built yet). */
export const ROW_HEIGHT = 24;
/** Rows fetched per request. */
export const CHUNK = 200;

export interface Range {
  start: number;
  end: number;
}

/** Rows to render for a scroll position, with `overscan` extra rows each side. */
export function visibleRange(scrollTop: number, viewport: number, total: number, overscan = 10): Range {
  const start = Math.min(total, Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - overscan));
  const end = Math.min(total, Math.ceil((scrollTop + viewport) / ROW_HEIGHT) + overscan);
  return { start, end: Math.max(start, end) };
}

/** Chunk indices covering `range`. */
export function chunksFor(range: Range): number[] {
  if (range.end <= range.start) {
    return [];
  }
  const first = Math.floor(range.start / CHUNK);
  const last = Math.floor((range.end - 1) / CHUNK);
  return Array.from({ length: last - first + 1 }, (_, i) => first + i);
}
