import { describe, expect, it } from "vitest";
import type { LfsPlan } from "$lib/api";
import {
  applyEffect,
  byFolder,
  childrenOf,
  folderOf,
  sameState,
  segments,
  shares,
  under,
  wantedBytes,
  withScene,
  withoutKept,
  withoutScene,
} from "./partial";

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
    skippedOnDisk: share(50),
    skippedFolders: [],
    skippedFiles: [],
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

  it("says what each bar segment means, and whether Apply removes the unused part", () => {
    const fmt = (b: number) => `${b} B`;
    const segs = segments(plan(), fmt);
    expect(segs.map((s) => [s.kind, s.pct, s.bytes])).toEqual([
      ["used", 40, 40],
      ["always", 5, 5],
      ["kept", 5, 5],
      ["skipped", 50, 50],
    ]);
    expect(segs[3].about).toMatch(/^50 B of it is on disk now\. Apply turns it back into pointer files/);
    expect(segments(plan({ skippedOnDisk: share(0, 0) }), fmt)[3].about).toMatch(/^Left as pointer files, not downloaded/);
  });

  it("groups unused files as the plan's folders do, and lists a folder's children", () => {
    expect(folderOf("a.bin")).toBe("(top level)");
    expect(folderOf("Assets/x.png")).toBe("Assets");
    expect(folderOf("Assets/Pack/Sub/Deep/x.png")).toBe("Assets/Pack/Sub");
    const files = [
      { path: "Assets/Pack/Sub/a.png", bytes: 10, onDisk: true },
      { path: "Assets/Pack/Sub/Deep/b.png", bytes: 30, onDisk: false },
      { path: "Assets/Pack/Sub/Deep/More/c.png", bytes: 5, onDisk: true },
      { path: "Assets/Pack/Sub/Big/d.png", bytes: 50, onDisk: true },
      { path: "top.bin", bytes: 1, onDisk: false },
    ];
    const groups = byFolder(files);
    expect([...groups.keys()]).toEqual(["Assets/Pack/Sub", "(top level)"]);
    const kids = childrenOf(groups.get("Assets/Pack/Sub")!, "Assets/Pack/Sub");
    expect(kids.map((k) => [k.name, k.folder, k.bytes, k.files, k.onDiskBytes])).toEqual([
      ["Big", true, 50, 1, 50],
      ["Deep", true, 35, 2, 5],
      ["a.png", false, 10, 1, 10],
    ]);
    const deep = under(groups.get("Assets/Pack/Sub")!, "Assets/Pack/Sub/Deep");
    expect(childrenOf(deep, "Assets/Pack/Sub/Deep").map((k) => k.path)).toEqual([
      "Assets/Pack/Sub/Deep/More",
      "Assets/Pack/Sub/Deep/b.png",
    ]);
    expect(childrenOf(groups.get("(top level)")!, "(top level)").map((k) => k.path)).toEqual(["top.bin"]);
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
