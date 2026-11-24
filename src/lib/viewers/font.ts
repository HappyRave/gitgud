import type { FontInfo } from "$lib/api";

export type GlyphState = "same" | "added" | "removed" | "changed";

export interface Glyph {
  cp: number;
  state: GlyphState;
}

export interface GlyphDiff {
  /** Every printable code point in either version, ascending. */
  glyphs: Glyph[];
  added: number;
  removed: number;
  changed: number;
}

/** Controls and blanks have nothing to draw in a grid cell. */
export function printable(cp: number): boolean {
  return !(cp < 0x21 || (cp >= 0x7f && cp <= 0xa0) || cp === 0xad || (cp >= 0x2000 && cp <= 0x200f) || cp === 0x3000 || cp === 0xfeff);
}

function hashes(info: FontInfo | null): Map<number, number> {
  const map = new Map<number, number>();
  if (info) {
    info.codepoints.forEach((cp, i) => map.set(cp, info.outlines[i]));
  }
  return map;
}

/** Glyphs added, removed and redrawn between two versions (either may be missing). */
export function diffGlyphs(before: FontInfo | null, after: FontInfo | null): GlyphDiff {
  const b = hashes(before);
  const a = hashes(after);
  const all = [...new Set([...b.keys(), ...a.keys()])].filter(printable).sort((x, y) => x - y);
  const out: GlyphDiff = { glyphs: [], added: 0, removed: 0, changed: 0 };
  const compare = before !== null && after !== null;
  for (const cp of all) {
    let state: GlyphState = "same";
    if (compare && !b.has(cp)) {
      state = "added";
    } else if (compare && !a.has(cp)) {
      state = "removed";
    } else if (compare && b.get(cp) !== a.get(cp)) {
      state = "changed";
    }
    if (state !== "same") {
      out[state]++;
    }
    out.glyphs.push({ cp, state });
  }
  return out;
}

export interface MetricRow {
  label: string;
  before: string | null;
  after: string | null;
  changed: boolean;
}

const axes = (f: FontInfo) => f.axes.map((x) => `${x.tag} ${x.min}–${x.max} (${x.default})`).join(", ") || "none";

const FIELDS: [string, (f: FontInfo) => string][] = [
  ["Family", (f) => f.family ?? "—"],
  ["Style", (f) => f.style ?? "—"],
  ["Version", (f) => f.version ?? "—"],
  ["Format", (f) => f.format],
  ["Weight", (f) => String(f.weight)],
  ["Italic", (f) => (f.italic ? "yes" : "no")],
  ["Units per em", (f) => String(f.unitsPerEm)],
  ["Ascender", (f) => String(f.ascender)],
  ["Descender", (f) => String(f.descender)],
  ["Line gap", (f) => String(f.lineGap)],
  ["x-height", (f) => (f.xHeight === null ? "—" : String(f.xHeight))],
  ["Cap height", (f) => (f.capHeight === null ? "—" : String(f.capHeight))],
  ["Glyphs", (f) => String(f.glyphCount)],
  ["Variation axes", axes],
];

/** Names and vertical metrics side by side, changed rows flagged. */
export function metricRows(before: FontInfo | null, after: FontInfo | null): MetricRow[] {
  return FIELDS.map(([label, get]) => {
    const b = before ? get(before) : null;
    const a = after ? get(after) : null;
    return { label, before: b, after: a, changed: before !== null && after !== null && a !== b };
  });
}

/** "OTF · v1.2 → v1.3": the format of the newer side, and each version string once. */
export function fontPill(path: string, before: FontInfo | null, after: FontInfo | null): string {
  const ext = path.slice(path.lastIndexOf(".") + 1).toUpperCase();
  const short = (f: FontInfo | null) => f?.version?.replace(/^Version\s*/i, "v").split(";")[0].trim() ?? null;
  const [b, a] = [short(before), short(after)];
  const ver = b && a && b !== a ? `${b} → ${a}` : (a ?? b);
  return ver ? `${ext} · ${ver}` : ext;
}

export const SAMPLE = "The quick brown fox jumps over the lazy dog";
export const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789 !?&@€";
