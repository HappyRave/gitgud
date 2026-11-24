import type { VideoInfo } from "$lib/api";

/** Containers Rust reads fps and codec from; others (WebM) rely on the player. */
export const hasBoxes = (path: string) => /\.(mp4|m4v|mov)$/i.test(path);

/** What the player itself knows once metadata loads. */
export interface PlayerMeta {
  width: number;
  height: number;
  duration: number;
}

const mbps = (b: number) => `${(b / 1e6).toFixed(b < 1e7 ? 1 : 0)} Mb/s`;

/** "1080p" for the header pill, from the frame height. */
export const quality = (h: number) => (h >= 2160 ? "4K" : h > 0 ? `${h}p` : "");

/** "8.0 s · 1920×1080 · 30 fps · H.264 · bitrate 8 → 12 Mb/s", changed parts flagged. */
export function videoCaption(
  before: { info: VideoInfo | null; meta: PlayerMeta } | null,
  after: { info: VideoInfo | null; meta: PlayerMeta },
): { text: string; changed: boolean }[] {
  const pair = (f: (s: NonNullable<typeof before>) => string | null) => {
    const a = f(after);
    const b = before ? f(before) : a;
    if (a === null) {
      return null;
    }
    return b === null || b === a ? { text: a, changed: false } : { text: `${b} → ${a}`, changed: true };
  };
  return [
    pair((s) => `${s.meta.duration.toFixed(1)} s`),
    pair((s) => (s.meta.width ? `${s.meta.width}×${s.meta.height}` : null)),
    pair((s) => (s.info?.fps ? `${Math.round(s.info.fps * 100) / 100} fps` : null)),
    pair((s) => s.info?.codec ?? null),
    pair((s) => (s.info?.bitrate ? mbps(s.info.bitrate) : null)),
    pair((s) => (s.info ? (s.info.audioCodec ? `sound ${s.info.audioCodec}` : "no sound") : null)),
  ].filter((p): p is { text: string; changed: boolean } => p !== null);
}

/** "00:04.3": minutes, seconds and tenths. */
export function timecode(t: number): string {
  const m = Math.floor(t / 60);
  const s = t - m * 60;
  return `${String(m).padStart(2, "0")}:${s.toFixed(1).padStart(4, "0")}`;
}

/** Mean absolute difference of two RGBA frames of the same size, 0–255. */
export function frameDifference(a: Uint8ClampedArray, b: Uint8ClampedArray): number {
  let sum = 0;
  let n = 0;
  for (let i = 0; i < a.length && i < b.length; i += 4) {
    sum += Math.abs(a[i] - b[i]) + Math.abs(a[i + 1] - b[i + 1]) + Math.abs(a[i + 2] - b[i + 2]);
    n += 3;
  }
  return n ? sum / n : 0;
}

/** A frame differing by more than this (0–255 mean) counts as changed. */
export const DIFF_THRESHOLD = 4;

/** Runs of samples over the threshold, as `[start, end]` fractions of the clip. */
export function diffSpans(scores: number[], threshold = DIFF_THRESHOLD): [number, number][] {
  const out: [number, number][] = [];
  const n = scores.length;
  let start = -1;
  scores.forEach((s, i) => {
    if (s > threshold && start < 0) {
      start = i;
    }
    if ((s <= threshold || i === n - 1) && start >= 0) {
      const end = s > threshold ? i + 1 : i;
      out.push([start / n, end / n]);
      start = -1;
    }
  });
  return out;
}

/** The times to sample: one per frame, at most `max`, spread over the shorter clip. */
export function sampleTimes(duration: number, fps: number, max = 240): number[] {
  const count = Math.max(1, Math.min(max, Math.floor(duration * fps)));
  return Array.from({ length: count }, (_, i) => ((i + 0.5) / count) * duration);
}
