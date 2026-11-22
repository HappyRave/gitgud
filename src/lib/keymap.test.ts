import { describe, expect, it } from "vitest";
import { SHORTCUT_GROUPS } from "./keymap";

describe("keymap", () => {
  it("covers all documented categories without empty shortcuts", () => {
    expect(SHORTCUT_GROUPS.map((g) => g.category)).toEqual([
      "Navigation",
      "Working copy",
      "Branches & remotes",
      "History & diff",
      "Proposal",
      "Application",
    ]);
    for (const group of SHORTCUT_GROUPS) {
      expect(group.items.length).toBeGreaterThan(0);
      for (const item of group.items) {
        expect(item.keys.trim().length).toBeGreaterThan(0);
        expect(item.label.trim().length).toBeGreaterThan(0);
      }
    }
  });
});
