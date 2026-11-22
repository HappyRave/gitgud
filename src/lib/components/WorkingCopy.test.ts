import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { FileDiff, Status } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import WorkingCopy from "./WorkingCopy.svelte";

const STATUS: Status = {
  root: "C:/r",
  head: { kind: "branch", name: "main" },
  entries: [
    { path: "both.txt", kind: "tracked", index: "modified", worktree: "modified" },
    { path: "stray.txt", kind: "untracked", index: null, worktree: null },
  ],
};

const hunk = (start: number) => ({
  oldStart: start,
  oldLines: 1,
  newStart: start,
  newLines: 1,
  lines: [
    { kind: "removed" as const, old: start, new: null, text: `old ${start}` },
    { kind: "added" as const, old: null, new: start, text: `new ${start}` },
  ],
});
const DIFF: FileDiff = { path: "both.txt", binary: false, hunks: [hunk(1), hunk(20)] };

function setup() {
  mockCommands({ worktree_file_diff: ({ path }) => ({ ...DIFF, path }) });
  const props = {
    onstage: vi.fn(),
    onunstage: vi.fn(),
    ondiscard: vi.fn(),
    onhunk: vi.fn(),
    oncommit: vi.fn(async () => {}),
  };
  render(WorkingCopy, { repoId: 1, status: STATUS, revision: 0, branch: "main", ...props });
  return props;
}

const key = (init: KeyboardEventInit) => fireEvent.keyDown(window, init);

describe("WorkingCopy keys", () => {
  it("↑↓ walk the files across groups and open each diff", async () => {
    setup();

    await key({ key: "ArrowDown" });
    expect(await screen.findByRole("region", { name: "Staged changes in both.txt" })).toBeInTheDocument();
    await key({ key: "ArrowDown" });
    expect(await screen.findByRole("region", { name: "Unstaged changes in both.txt" })).toBeInTheDocument();
    await key({ key: "ArrowDown" });
    expect(await screen.findByRole("region", { name: "Unstaged changes in stray.txt" })).toBeInTheDocument();
    await key({ key: "ArrowUp" });
    expect(await screen.findByRole("region", { name: "Unstaged changes in both.txt" })).toBeInTheDocument();
  });

  it("Space stages or unstages the selected file; Shift+⌫ discards an unstaged one", async () => {
    const { onstage, onunstage, ondiscard } = setup();

    await key({ key: "ArrowDown" });
    await key({ key: " " });
    await key({ key: "Backspace", shiftKey: true });
    await key({ key: "ArrowDown" });
    await key({ key: " " });
    await key({ key: "Backspace", shiftKey: true });

    expect(onunstage.mock.calls).toEqual([[["both.txt"]]]);
    expect(onstage.mock.calls).toEqual([[["both.txt"]]]);
    expect(ondiscard.mock.calls).toEqual([[["both.txt"]]]);
  });

  it("A stages every change and untracked file; C focuses the message", async () => {
    const { onstage } = setup();

    await key({ key: "a" });
    await key({ key: "c" });

    expect(onstage).toHaveBeenCalledWith(["both.txt", "stray.txt"]);
    expect(screen.getByRole("textbox", { name: "Commit message" })).toHaveFocus();
  });

  const diffLine = (text: string) => (_: string, el: Element | null) => el?.tagName === "TD" && el.textContent === text;

  it("Alt+↓ moves to the next hunk, S stages it, ⌫ discards it", async () => {
    const { onhunk } = setup();
    await key({ key: "ArrowDown" });
    await key({ key: "ArrowDown" });
    await screen.findByText(diffLine("+new 20"));

    await key({ key: "ArrowDown", altKey: true });
    await key({ key: "s" });
    await key({ key: "Backspace" });
    await key({ key: "u" });

    expect(onhunk.mock.calls.map(([path, action, h]) => [path, action, h.oldStart])).toEqual([
      ["both.txt", "stage", 20],
      ["both.txt", "discard", 20],
    ]);
  });

  it("U unstages the hunk on the staged side", async () => {
    const { onhunk } = setup();
    await key({ key: "ArrowDown" });
    await screen.findByText(diffLine("+new 1"));

    await key({ key: "s" });
    await key({ key: "u" });

    expect(onhunk.mock.calls.map(([, action, h]) => [action, h.oldStart])).toEqual([["unstage", 1]]);
  });

  it("Space on a focused button presses that button instead", async () => {
    const { onstage, onunstage } = setup();
    await key({ key: "ArrowDown" });

    await fireEvent.keyDown(screen.getByRole("button", { name: "Stage all (Changes)" }), { key: " " });

    expect(onstage).not.toHaveBeenCalled();
    expect(onunstage).not.toHaveBeenCalled();
  });

  it("stays off while suspended (a dialog is open)", async () => {
    mockCommands({ worktree_file_diff: ({ path }) => ({ ...DIFF, path }) });
    const onstage = vi.fn();
    render(WorkingCopy, {
      repoId: 1,
      status: STATUS,
      revision: 0,
      branch: "main",
      suspended: true,
      onstage,
      onunstage: vi.fn(),
      ondiscard: vi.fn(),
      onhunk: vi.fn(),
      oncommit: vi.fn(async () => {}),
    });

    await key({ key: "a" });

    expect(onstage).not.toHaveBeenCalled();
  });

  it("leaves keys alone when focus is somewhere else", async () => {
    const { onstage } = setup();
    const outside = document.createElement("button");
    document.body.append(outside);

    await fireEvent.keyDown(outside, { key: "a" });

    expect(onstage).not.toHaveBeenCalled();
    outside.remove();
  });
});
