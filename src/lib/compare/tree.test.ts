import { describe, expect, it } from "vitest";
import type { Comparison, FileChange } from "$lib/api";
import { allFolders, buildTree, fileSides, imageAt, kinds, pointLabel, rev, samePoint, treeRows, WORKING_COPY } from "./tree";

const f = (path: string, change: FileChange["change"] = "modified"): FileChange => ({ path, change });

const FILES = [
  f("Assets/Game/Lobby/Lobby.unity"),
  f("Assets/Game/Lobby/Lobby.unity.meta"),
  f("Assets/Game/Lobby/Seat.cs", "added"),
  f("Assets/Game/Crab/Crab.prefab"),
  f("Assets/Deep/Only/Path/x.wav", "deleted"),
  f("README.md"),
];

describe("buildTree", () => {
  it("counts files per folder and merges single-child chains", () => {
    const root = buildTree(FILES);
    expect(root.count).toBe(6);
    expect(root.folders.map((d) => d.name)).toEqual(["Assets"]);
    const assets = root.folders[0];
    expect(assets.folders.map((d) => [d.name, d.count])).toEqual([
      ["Deep/Only/Path", 1],
      ["Game", 4],
    ]);
    expect(root.files.map((x) => x.path)).toEqual(["README.md"]);
  });

  it("draws open folders only", () => {
    const root = buildTree(FILES);
    expect(treeRows(root, new Set()).map((r) => r.name)).toEqual(["Assets", "README.md"]);
    const rows = treeRows(root, new Set(["Assets", "Assets/Game", "Assets/Game/Lobby"]));
    expect(rows.map((r) => `${"  ".repeat(r.depth)}${r.name}`)).toEqual([
      "Assets",
      "  Deep/Only/Path",
      "  Game",
      "    Crab",
      "    Lobby",
      "      Lobby.unity",
      "      Lobby.unity.meta",
      "      Seat.cs",
      "README.md",
    ]);
    expect(allFolders(root)).toEqual(["Assets", "Assets/Deep/Only/Path", "Assets/Game", "Assets/Game/Crab", "Assets/Game/Lobby"]);
  });

  it("stays quick on a 80,000-file comparison", () => {
    const many = Array.from({ length: 80_000 }, (_, i) => f(`Assets/G${i % 40}/S${i % 700}/file${i}.asset`));
    const started = performance.now();
    const root = buildTree(many);
    treeRows(root, new Set(["Assets"]));
    expect(root.count).toBe(80_000);
    expect(performance.now() - started).toBeLessThan(1500);
  });
});

describe("kinds", () => {
  it("counts extensions, most first, without .meta", () => {
    expect(kinds(FILES)).toEqual([
      { ext: ".cs", count: 1 },
      { ext: ".md", count: 1 },
      { ext: ".prefab", count: 1 },
      { ext: ".unity", count: 1 },
      { ext: ".wav", count: 1 },
    ]);
    expect(kinds([f("a/Makefile"), f("b.CS"), f("c.cs")])).toEqual([
      { ext: ".cs", count: 2 },
      { ext: "(none)", count: 1 },
    ]);
  });
});

describe("points", () => {
  const c: Comparison = {
    a: "a".repeat(40),
    b: "b".repeat(40),
    base: "c".repeat(40),
    mode: "brings",
    from: "c".repeat(40),
    onlyA: { count: 0, commits: [] },
    onlyB: { count: 0, commits: [] },
    files: [],
  };

  it("takes file versions from the split point and B, or the working copy", () => {
    expect(fileSides(c, rev("main"), rev("Feature"))).toEqual({ from: rev("c".repeat(40)), to: rev("b".repeat(40)) });
    expect(fileSides(c, rev("v1"), WORKING_COPY)).toEqual({ from: rev("c".repeat(40)), to: WORKING_COPY });
    expect(fileSides(c, WORKING_COPY, rev("v1"))).toEqual({ from: WORKING_COPY, to: rev("b".repeat(40)) });
    expect(imageAt(WORKING_COPY)).toEqual({ at: "worktree" });
    expect(imageAt(rev("abc"))).toEqual({ at: "commit", commit: "abc" });
  });

  it("names and compares points", () => {
    expect(pointLabel(WORKING_COPY)).toBe("Working copy");
    expect(pointLabel(rev("origin/main"))).toBe("origin/main");
    expect(samePoint(rev("a"), rev("a"))).toBe(true);
    expect(samePoint(rev("a"), WORKING_COPY)).toBe(false);
  });
});
