import { describe, expect, it } from "vitest";
import { font } from "$lib/test/font";
import { diffGlyphs, fontPill, metricRows, printable } from "./font";

describe("diffGlyphs", () => {
  it("finds added, removed and redrawn glyphs", () => {
    const d = diffGlyphs(font({ A: 1, B: 2, "€": 3 }), font({ A: 1, B: 9, "₽": 4 }));
    expect(d.glyphs).toEqual([
      { cp: 0x41, state: "same" },
      { cp: 0x42, state: "changed" },
      { cp: 0x20ac, state: "removed" },
      { cp: 0x20bd, state: "added" },
    ]);
    expect([d.added, d.removed, d.changed]).toEqual([1, 1, 1]);
  });

  it("leaves a lone version untinted and skips blanks and controls", () => {
    const d = diffGlyphs(null, font({ " ": 0, " ": 0, A: 1, "​": 0 }));
    expect(d.glyphs).toEqual([{ cp: 0x41, state: "same" }]);
    expect(d.added).toBe(0);
    expect([printable(0x20), printable(0x41), printable(0xad)]).toEqual([false, true, false]);
  });
});

describe("metricRows", () => {
  it("flags only the rows that changed", () => {
    const rows = metricRows(font({}), font({}, { weight: 600, axes: [{ tag: "wght", name: "Weight", min: 100, default: 400, max: 900 }] }));
    expect(rows.filter((r) => r.changed).map((r) => r.label)).toEqual(["Weight", "Variation axes"]);
    expect(rows.find((r) => r.label === "Variation axes")?.after).toBe("wght 100–900 (400)");
  });

  it("changes nothing for a new font", () => {
    expect(metricRows(null, font({})).some((r) => r.changed)).toBe(false);
  });
});

describe("fontPill", () => {
  it("shows the extension and each version once", () => {
    expect(fontPill("UI/Harbour.otf", font({}), font({}, { version: "Version 1.3" }))).toBe("OTF · v1.2 → v1.3");
    expect(fontPill("UI/Harbour.woff2", font({}), font({}))).toBe("WOFF2 · v1.2");
    expect(fontPill("a.ttf", null, font({}, { version: null }))).toBe("TTF");
  });
});
