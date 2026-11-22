import { describe, expect, it } from "vitest";
import type { Branch } from "$lib/api";
import { aheadBehind, branchTree, visibleRows, type Node } from "./tree";

const b = (name: string, extra: Partial<Branch> = {}): Branch => ({
  name,
  current: false,
  upstream: null,
  ahead: 0,
  behind: 0,
  ...extra,
});

const labels = (rows: { node: Node; depth: number }[]) =>
  rows.map(({ node, depth }) => "  ".repeat(depth) + (node.kind === "folder" ? `${node.name}/` : node.name));

describe("branchTree", () => {
  it("groups by folder, plain branches before folders, each sorted", () => {
    const tree = branchTree([b("feature/stack"), b("master"), b("conflict/theirs"), b("conflict/ours"), b("hotfix/x"), b("dev")]);

    expect(labels(visibleRows(tree, new Set()))).toEqual([
      "dev",
      "master",
      "conflict/",
      "  ours",
      "  theirs",
      "feature/",
      "  stack",
      "hotfix/",
      "  x",
    ]);
  });

  it("nests deeper folders", () => {
    const tree = branchTree([b("feature/ui/theme"), b("feature/login")]);

    expect(labels(visibleRows(tree, new Set()))).toEqual(["feature/", "  login", "  ui/", "    theme"]);
  });

  it("keeps each leaf's branch data", () => {
    const main = b("main", { current: true });
    const [leaf] = branchTree([main]);
    expect(leaf).toEqual({ kind: "branch", name: "main", branch: main });
  });
});

describe("visibleRows", () => {
  it("hides what is inside a collapsed folder", () => {
    const tree = branchTree([b("feature/a"), b("feature/ui/b"), b("main")]);

    expect(labels(visibleRows(tree, new Set(["feature/"])))).toEqual(["main", "feature/"]);
    expect(labels(visibleRows(tree, new Set(["feature/ui/"])))).toEqual(["main", "feature/", "  a", "  ui/"]);
  });
});

describe("aheadBehind", () => {
  it("shows only what is non-zero", () => {
    expect(aheadBehind(b("x", { ahead: 2, behind: 1 }))).toBe("↑2 ↓1");
    expect(aheadBehind(b("x", { ahead: 3 }))).toBe("↑3");
    expect(aheadBehind(b("x", { behind: 4 }))).toBe("↓4");
    expect(aheadBehind(b("x"))).toBe("");
  });
});
