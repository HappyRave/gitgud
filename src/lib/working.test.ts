import { describe, expect, it } from "vitest";
import type { Status } from "$lib/api";
import {
  applyFileClick,
  extendFileSelection,
  fileKey,
  fileOrder,
  selectedPathsForSide,
  singleFileSelection,
  stepFile,
} from "./working";

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

describe("multi-select helpers", () => {
  const MULTI: Status = {
    root: "C:/r",
    head: { kind: "branch", name: "main" },
    entries: [
      { path: "s1.txt", kind: "tracked", index: "added", worktree: null },
      { path: "a.txt", kind: "tracked", index: null, worktree: "modified" },
      { path: "b.txt", kind: "tracked", index: null, worktree: "modified" },
      { path: "u1.txt", kind: "untracked", index: null, worktree: null },
    ],
  };
  const order = fileOrder(MULTI);

  it("toggles same-side items with Ctrl+click and resets when switching sides", () => {
    let sel = applyFileClick(order, singleFileSelection(null), { path: "a.txt", side: "unstaged" });
    sel = applyFileClick(order, sel, { path: "u1.txt", side: "unstaged" }, { ctrl: true });
    expect(selectedPathsForSide(order, sel.keys, "unstaged")).toEqual(["a.txt", "u1.txt"]);

    // Clicking an already selected item with Ctrl removes it and moves active.
    sel = applyFileClick(order, sel, { path: "u1.txt", side: "unstaged" }, { ctrl: true });
    expect(selectedPathsForSide(order, sel.keys, "unstaged")).toEqual(["a.txt"]);
    expect(sel.active).toEqual({ path: "a.txt", side: "unstaged" });

    // Deselecting the last item clears the selection.
    sel = applyFileClick(order, sel, { path: "a.txt", side: "unstaged" }, { ctrl: true });
    expect(sel.active).toBeNull();
    expect(sel.keys.size).toBe(0);

    // Ctrl+clicking across to staged resets to staged only.
    sel = applyFileClick(order, applyFileClick(order, sel, { path: "a.txt", side: "unstaged" }), { path: "s1.txt", side: "staged" }, { ctrl: true });
    expect([...sel.keys]).toEqual([fileKey("s1.txt", "staged")]);
  });

  it("selects contiguous ranges across Changes and Untracked with Shift+click and Shift+Arrow", () => {
    let sel = applyFileClick(order, singleFileSelection(null), { path: "a.txt", side: "unstaged" });
    sel = applyFileClick(order, sel, { path: "u1.txt", side: "unstaged" }, { shift: true });
    expect(selectedPathsForSide(order, sel.keys, "unstaged")).toEqual(["a.txt", "b.txt", "u1.txt"]);

    // Shrinking range back keeps anchor at a.txt.
    sel = extendFileSelection(order, sel, -1);
    expect(selectedPathsForSide(order, sel.keys, "unstaged")).toEqual(["a.txt", "b.txt"]);
  });
});
