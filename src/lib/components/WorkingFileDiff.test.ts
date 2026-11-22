import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { FileDiff, Side } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import WorkingFileDiff from "./WorkingFileDiff.svelte";

const diff = (text: string): FileDiff => ({
  path: "f.txt",
  binary: false,
  hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: [{ kind: "added", old: null, new: 1, text }] }],
});

function setup(props: { side?: Side; perHunk?: boolean; revision?: number } = {}) {
  let version = 0;
  const calls = mockCommands({ worktree_file_diff: () => diff(`version ${version++}`) });
  const onhunk = vi.fn();
  const view = render(WorkingFileDiff, {
    repoId: 3,
    path: "f.txt",
    side: props.side ?? "unstaged",
    revision: props.revision ?? 0,
    perHunk: props.perHunk ?? true,
    onhunk,
  });
  return { calls, onhunk, view };
}

describe("WorkingFileDiff", () => {
  it("loads the diff for the file and side", async () => {
    const { calls } = setup({ side: "staged" });

    expect(await screen.findByText("version 0")).toBeInTheDocument();
    expect(calls[0]).toEqual({ cmd: "worktree_file_diff", args: { id: 3, path: "f.txt", side: "staged" } });
    expect(screen.getByRole("region", { name: "Staged changes in f.txt" })).toBeInTheDocument();
  });

  it("offers Stage and Discard on unstaged hunks", async () => {
    const { onhunk } = setup();
    await screen.findByText("version 0");

    await fireEvent.click(screen.getByRole("button", { name: /^Stage hunk/ }));
    await fireEvent.click(screen.getByRole("button", { name: /^Discard hunk/ }));

    expect(onhunk.mock.calls.map(([action, hunk]) => [action, hunk.oldStart])).toEqual([
      ["stage", 1],
      ["discard", 1],
    ]);
  });

  it("offers only Unstage on staged hunks", async () => {
    const { onhunk } = setup({ side: "staged" });
    await screen.findByText("version 0");

    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Unstage hunk"]);
    await fireEvent.click(screen.getByRole("button", { name: /^Unstage hunk/ }));
    expect(onhunk).toHaveBeenCalledWith("unstage", expect.objectContaining({ oldStart: 1 }));
  });

  it("has no hunk buttons where only whole files stage", async () => {
    setup({ perHunk: false });
    await screen.findByText("version 0");

    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("reloads when the status revision moves", async () => {
    const { view } = setup();
    await screen.findByText("version 0");

    await view.rerender({ repoId: 3, path: "f.txt", side: "unstaged", revision: 1, perHunk: true, onhunk: () => {} });

    expect(await screen.findByText("version 1")).toBeInTheDocument();
  });

  it("shows why a diff can't be read", async () => {
    mockCommands({
      worktree_file_diff: () => {
        throw "f.txt is conflicted: resolve it, then stage the whole file";
      },
    });
    render(WorkingFileDiff, { repoId: 3, path: "f.txt", side: "unstaged", revision: 0, perHunk: true, onhunk: () => {} });

    expect(await screen.findByRole("alert")).toHaveTextContent("conflicted");
  });
});
