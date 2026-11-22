import { describe, expect, it } from "vitest";
import type { Status } from "$lib/api";
import { fileOrder, stepFile } from "./working";

const STATUS: Status = {
  root: "C:/r",
  head: { kind: "branch", name: "main" },
  entries: [
    { path: "both.txt", kind: "tracked", index: "modified", worktree: "modified" },
    { path: "c.txt", kind: "conflicted", index: null, worktree: null },
    { path: "new.txt", kind: "tracked", index: "added", worktree: null },
    { path: "stray.txt", kind: "untracked", index: null, worktree: null },
  ],
};

describe("fileOrder", () => {
  it("lists staged, then changed, then untracked files; conflicts open no diff", () => {
    expect(fileOrder(STATUS)).toEqual([
      { path: "both.txt", side: "staged" },
      { path: "new.txt", side: "staged" },
      { path: "both.txt", side: "unstaged" },
      { path: "stray.txt", side: "unstaged" },
    ]);
  });
});

describe("stepFile", () => {
  const order = fileOrder(STATUS);

  it("starts at the first file", () => {
    expect(stepFile(order, null, 1)).toEqual(order[0]);
  });

  it("moves by side as well as path, and stops at the ends", () => {
    expect(stepFile(order, { path: "new.txt", side: "staged" }, 1)).toEqual({ path: "both.txt", side: "unstaged" });
    expect(stepFile(order, order[3], 1)).toEqual(order[3]);
    expect(stepFile(order, order[0], -1)).toEqual(order[0]);
  });

  it("restarts at the top when the selection is gone, and is null with no files", () => {
    expect(stepFile(order, { path: "gone.txt", side: "staged" }, 1)).toEqual(order[0]);
    expect(stepFile([], null, 1)).toBeNull();
  });
});
