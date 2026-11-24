import type { ClipCurves, CurveKey, CurveTrack } from "$lib/api";

/**
 * A curve's value at `t` as Unity evaluates it: cubic Hermite between keys from
 * their slopes, holding the earlier key where a slope is infinite (null), and
 * flat before the first and after the last key. `linear` joins keys with lines.
 */
export function evaluate(keys: CurveKey[], t: number, linear = false): number {
  if (keys.length === 0) {
    return 0;
  }
  if (t <= keys[0].time) {
    return keys[0].value;
  }
  const last = keys[keys.length - 1];
  if (t >= last.time) {
    return last.value;
  }
  let i = 0;
  while (keys[i + 1].time < t) {
    i++;
  }
  const a = keys[i];
  const b = keys[i + 1];
  const dt = b.time - a.time;
  const s = dt > 0 ? (t - a.time) / dt : 0;
  if (linear) {
    return a.value + (b.value - a.value) * s;
  }
  if (a.outSlope === null || b.inSlope === null) {
    return a.value;
  }
  const s2 = s * s;
  const s3 = s2 * s;
  return (
    (2 * s3 - 3 * s2 + 1) * a.value + (s3 - 2 * s2 + s) * a.outSlope * dt + (-2 * s3 + 3 * s2) * b.value + (s3 - s2) * b.inSlope * dt
  );
}

/** Lowest and highest value the keys reach over `[0, duration]`, sampled. */
export function range(keys: CurveKey[], duration: number, linear: boolean, steps = 200): [number, number] {
  let lo = Infinity;
  let hi = -Infinity;
  for (let i = 0; i <= steps; i++) {
    const v = evaluate(keys, (i / steps) * duration, linear);
    lo = Math.min(lo, v);
    hi = Math.max(hi, v);
  }
  for (const k of keys) {
    lo = Math.min(lo, k.value);
    hi = Math.max(hi, k.value);
  }
  return [lo, hi];
}

/** An SVG path through the curve, one point per `step` px of `width`. */
export function curvePath(
  keys: CurveKey[],
  duration: number,
  width: number,
  y: (v: number) => number,
  linear: boolean,
  step = 2,
): string {
  if (keys.length === 0 || duration <= 0) {
    return "";
  }
  const parts: string[] = [];
  for (let x = 0; x <= width; x += step) {
    parts.push(`${parts.length ? "L" : "M"}${x.toFixed(1)},${y(evaluate(keys, (x / width) * duration, linear)).toFixed(1)}`);
  }
  return parts.join("");
}

export const trackId = (t: { path: string; label: string }) => `${t.path}|${t.label}`;

/** Tracks grouped by object, in first-seen order. */
export function byObject<T extends { path: string }>(tracks: T[]): [string, T[]][] {
  const out = new Map<string, T[]>();
  for (const t of tracks) {
    out.set(t.path, [...(out.get(t.path) ?? []), t]);
  }
  return [...out];
}

/** What to plot first: the changed tracks (up to `max`), else the first few. */
export function initialSelection(tracks: CurveTrack[], max = 6): Set<string> {
  const changed = tracks.filter((t) => t.state !== "same");
  return new Set((changed.length ? changed : tracks).slice(0, max).map(trackId));
}

/** "2 tracks changed, 1 added · keys 8 → 15". */
export function curveCaption(c: ClipCurves, compare: boolean): { text: string; tone: "" | "mod" | "add" | "del" }[] {
  const all = [...c.tracks, ...c.flipbooks];
  const count = (s: string) => all.filter((t) => t.state === s).length;
  const out: { text: string; tone: "" | "mod" | "add" | "del" }[] = [];
  if (!compare) {
    out.push({ text: `${all.length} tracks · ${c.keysAfter} keys`, tone: "" });
    return out;
  }
  const [ch, add, rem] = [count("changed"), count("added"), count("removed")];
  if (ch) {
    out.push({ text: `${ch} ${ch === 1 ? "track" : "tracks"} changed`, tone: "mod" });
  }
  if (add) {
    out.push({ text: `${add} added`, tone: "add" });
  }
  if (rem) {
    out.push({ text: `${rem} removed`, tone: "del" });
  }
  if (!ch && !add && !rem) {
    out.push({ text: "no curve changed", tone: "" });
  }
  out.push({ text: c.keysBefore === c.keysAfter ? `${c.keysAfter} keys` : `keys ${c.keysBefore} → ${c.keysAfter}`, tone: "" });
  return out;
}

export type CurveMode = "curves" | "3d";
export const CURVE_MODES: readonly CurveMode[] = ["curves", "3d"];
