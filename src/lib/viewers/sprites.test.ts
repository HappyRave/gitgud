import { describe, expect, it } from "vitest";
import type { Sprite, SpriteSheet } from "$lib/api";
import { spriteBox, spriteCaption, spriteStates } from "./sprites";

const sp = (name: string, x: number, w = 32): Sprite => ({ name, rect: [x, 0, w, 32], pivot: [0.5, 0.5], border: [0, 0, 0, 0] });

const sheet: SpriteSheet = {
  mode: "multiple",
  before: [sp("anchor", 0), sp("wheel", 32), sp("raft", 64)],
  after: [sp("anchor", 0, 40), sp("wheel", 48), sp("fuel", 96)],
  changes: [
    { name: "anchor", kinds: ["resized"], note: "resized 32×32 → 40×32" },
    { name: "wheel", kinds: ["moved"], note: "moved 32, 0 → 48, 0" },
    { name: "fuel", kinds: ["added"], note: "32×32" },
    { name: "raft", kinds: ["removed"], note: "" },
  ],
};

describe("sprites", () => {
  it("flips Unity's bottom-left rects to image space; Single covers the texture", () => {
    expect(spriteBox(sp("a", 10), 128, 64)).toEqual({ left: 10, top: 32, width: 32, height: 32 });
    expect(spriteBox({ ...sp("", 0), rect: [0, 0, 0, 0] }, 128, 64)).toEqual({ left: 0, top: 0, width: 128, height: 64 });
  });

  it("names each sprite's change and writes the caption", () => {
    expect(Object.fromEntries(spriteStates(sheet))).toEqual({ anchor: "changed", wheel: "changed", fuel: "added", raft: "removed" });
    expect(spriteCaption(sheet).map((p) => p.text)).toEqual([
      "3 slices",
      "+1 fuel",
      "−1 raft",
      "wheel moved 32, 0 → 48, 0",
      "anchor resized 32×32 → 40×32",
    ]);
    expect(spriteCaption({ ...sheet, changes: [] }).at(-1)?.text).toBe("slicing unchanged");
    expect(spriteCaption({ mode: "none", before: null, after: [], changes: [] })[0].text).toBe("not imported as sprites");
  });
});
