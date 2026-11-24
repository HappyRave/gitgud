import type { AudioInfo } from "$lib/api";

/** Formats the webview can't decode; they're played from Rust's WAV (`/audio`). */
const NEEDS_WAV = /\.(aiff?|aifc|caf)$/i;

export const needsWav = (path: string) => NEEDS_WAV.test(path);

const sec = (s: number) => `${s.toFixed(2)} s`;
const db = (v: number | null) => (v === null ? "silent" : `${v.toFixed(1).replace("-", "−")} dBFS`);
const signed = (v: number, digits: number, unit: string) => `${v > 0 ? "+" : v < 0 ? "−" : "±"}${Math.abs(v).toFixed(digits)} ${unit}`;

export interface CaptionPart {
  text: string;
  changed: boolean;
}

function rate(hz: number) {
  return hz % 1000 === 0 ? `${hz / 1000} kHz` : `${(hz / 1000).toFixed(1)} kHz`;
}

const channelWord = (n: number) => (n === 1 ? "mono" : n === 2 ? "stereo" : `${n} channels`);

/** "2.40 → 2.85 s · +0.45 s · 48 kHz · mono · peak −3.1 → −1.2 dBFS · loudness +2.4 LU". */
export function audioCaption(before: AudioInfo | null, after: AudioInfo): CaptionPart[] {
  if (!before) {
    const parts = [sec(after.duration), rate(after.sampleRate), channelWord(after.channels), `peak ${db(after.peakDbfs)}`];
    if (after.loudnessLufs !== null) {
      parts.push(`${after.loudnessLufs.toFixed(1)} LUFS`);
    }
    return parts.map((text) => ({ text, changed: false }));
  }
  const out: CaptionPart[] = [];
  const lenChanged = Math.abs(before.duration - after.duration) >= 0.005;
  out.push({ text: lenChanged ? `${before.duration.toFixed(2)} → ${sec(after.duration)}` : sec(after.duration), changed: false });
  if (lenChanged) {
    out.push({ text: signed(after.duration - before.duration, 2, "s"), changed: true });
  }
  const pair = (a: string, b: string) => (a === b ? { text: b, changed: false } : { text: `${a} → ${b}`, changed: true });
  out.push(pair(rate(before.sampleRate), rate(after.sampleRate)));
  out.push(pair(channelWord(before.channels), channelWord(after.channels)));
  const peak = pair(db(before.peakDbfs), db(after.peakDbfs));
  out.push({ ...peak, text: `peak ${peak.text}`, changed: peak.changed && Math.abs((before.peakDbfs ?? 0) - (after.peakDbfs ?? 0)) >= 0.1 });
  if (before.loudnessLufs !== null && after.loudnessLufs !== null) {
    const d = Math.round((after.loudnessLufs - before.loudnessLufs) * 10) / 10;
    out.push({ text: d === 0 ? "same loudness" : `loudness ${signed(d, 1, "LU")}`, changed: d !== 0 });
  }
  return out;
}

/**
 * Draws one channel's min / max pairs into a lane `width` × `height` px; the
 * file fills `duration / span` of the width so versions share a time axis.
 */
export function drawWave(
  g: CanvasRenderingContext2D,
  peaks: [number, number][],
  width: number,
  height: number,
  duration: number,
  span: number,
  color: string,
) {
  g.clearRect(0, 0, width, height);
  const mid = height / 2;
  g.fillStyle = color;
  const used = span > 0 ? (duration / span) * width : width;
  if (peaks.length === 0) {
    return;
  }
  for (let x = 0; x < used; x++) {
    const i = Math.min(peaks.length - 1, Math.floor((x / used) * peaks.length));
    const [lo, hi] = peaks[i];
    const top = mid - hi * mid;
    g.fillRect(x, top, 1, Math.max(1, (hi - lo) * mid));
  }
  g.fillStyle = "rgb(127 127 127 / 0.35)";
  g.fillRect(0, mid, used, 1);
}

export const clock = (t: number, total: number) => `${t.toFixed(2)} / ${total.toFixed(2)} s`;
