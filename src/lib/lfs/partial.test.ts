import { describe, expect, it } from "vitest";
import type { LfsPlan } from "$lib/api";
import { applyEffect, sameState, shares, wantedBytes, withScene, withoutKept, withoutScene } from "./partial";

const share = (bytes: number, files = 1) => ({ bytes, files });

function plan(over: Partial<LfsPlan> = {}): LfsPlan {
  return {
    partial: false,
    state: { buildScenes: true, scenes: [], kept: [] },
    buildScenes: ["Assets/Main.unity"],
    allScenes: ["Assets/Main.unity", "Assets/Test.unity"],
    total: share(100),
    used: share(40),
    always: share(5),
    kept: share(5),
    skipped: share(50),
    skippedFolders: [],
    onDisk: share(100),
    missing: [],
    fetchedToRead: 0,
    ...over,
  };
}

describe("partial LFS plan", () => {
  it("frees what a whole repo stops needing, downloads what a fresh clone lacks", () => {
    expect(wantedBytes(plan())).toBe(50);
    expect(applyEffect(plan())).toEqual({ kind: "free", bytes: 50 });
    expect(applyEffect(plan({ onDisk: share(10) }))).toEqual({ kind: "download", bytes: 40 });
    expect(applyEffect(plan({ onDisk: share(50) }))).toEqual({ kind: "none", bytes: 0 });
  });

  it("splits the bar by share of the total", () => {
    expect(shares(plan())).toEqual([40, 5, 5, 50]);
    expect(shares(plan({ total: share(0), used: share(0), always: share(0), kept: share(0), skipped: share(0) }))).toEqual([0, 0, 0, 0]);
  });

  it("edits scenes and kept paths without duplicates", () => {
    const s = plan().state;
    const added = withScene(s, " Assets/Test.unity ");
    expect(added.scenes).toEqual(["Assets/Test.unity"]);
    expect(withScene(added, "Assets/Test.unity")).toBe(added);
    expect(withScene(s, "  ")).toBe(s);
    expect(withoutScene(added, "Assets/Test.unity").scenes).toEqual([]);
    expect(withoutKept({ ...s, kept: ["A", "B"] }, "A").kept).toEqual(["B"]);
    expect(sameState(s, { ...s })).toBe(true);
    expect(sameState(s, added)).toBe(false);
  });
});
