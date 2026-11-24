import type { FontInfo } from "$lib/api";

/** A FontInfo whose glyphs map each character to an outline hash. */
export function font(glyphs: Record<string, number>, over: Partial<FontInfo> = {}): FontInfo {
  const cps = Object.keys(glyphs).map((c) => c.codePointAt(0)!);
  return {
    format: "OpenType (CFF)",
    faces: 1,
    family: "Harbour",
    style: "Regular",
    version: "Version 1.2;hotconv",
    weight: 400,
    italic: false,
    unitsPerEm: 1000,
    ascender: 800,
    descender: -200,
    lineGap: 0,
    xHeight: 500,
    capHeight: 700,
    glyphCount: cps.length + 1,
    axes: [],
    codepoints: cps,
    outlines: Object.values(glyphs),
    ...over,
  };
}
