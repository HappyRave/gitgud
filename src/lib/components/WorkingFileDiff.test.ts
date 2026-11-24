import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { FileDiff, Side } from "$lib/api";
import { prefs } from "$lib/prefs.svelte";
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

    expect(screen.getAllByRole("button", { name: /hunk/ }).map((b) => b.textContent)).toEqual(["Unstage hunkU"]);
    await fireEvent.click(screen.getByRole("button", { name: /^Unstage hunk/ }));
    expect(onhunk).toHaveBeenCalledWith("unstage", expect.objectContaining({ oldStart: 1 }));
  });

  it("has no hunk buttons where only whole files stage", async () => {
    setup({ perHunk: false });
    await screen.findByText("version 0");

    expect(screen.queryByRole("button", { name: /hunk/ })).not.toBeInTheDocument();
  });

  it("the layout button switches to side by side and back", async () => {
    setup();
    await screen.findByText("version 0");
    expect(document.querySelector("tr.pair")).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: "unified · Ctrl+D" }));
    expect(document.querySelector("tr.pair")).not.toBeNull();
    expect(localStorage.getItem("gitgud.diff.split")).toBe("1");

    await fireEvent.click(screen.getByRole("button", { name: "split · Ctrl+D" }));
    expect(document.querySelector("tr.pair")).toBeNull();
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

  it("shows a binary image as an image diff of the side's two versions", async () => {
    mockCommands({ worktree_file_diff: () => ({ path: "Art/t.png", binary: true, hunks: [] }) });
    const view = render(WorkingFileDiff, { repoId: 3, path: "Art/t.png", side: "staged", revision: 4, perHunk: false, onhunk: () => {} });

    expect(await screen.findByAltText("HEAD")).toHaveAttribute("src", expect.stringContaining("path=Art%2Ft.png&at=head&v=4"));
    expect(screen.getByAltText("Staged")).toHaveAttribute("src", expect.stringContaining("at=index"));

    await view.rerender({ repoId: 3, path: "Art/t.png", side: "unstaged", revision: 4, perHunk: false, onhunk: () => {} });
    expect(await screen.findByAltText("On disk")).toHaveAttribute("src", expect.stringContaining("at=worktree"));
    expect(screen.getByAltText("Index")).toHaveAttribute("src", expect.stringContaining("at=index"));
  });

  it("fetches an unhydrated non-image LFS binary object and refreshes its diff metadata", async () => {
    const oid = "ab".repeat(32);
    let hydrated = false;
    const calls = mockCommands({
      worktree_file_diff: () => ({
        path: "Assets/Audio.wav",
        binary: true,
        hunks: [],
        binaryMeta: {
          before: { size: 1024, lfs: true, oid: "00".repeat(32), hydrated: true },
          after: { size: 4096, lfs: true, oid, hydrated },
        },
      }),
      lfs_fetch_object: () => {
        hydrated = true;
        return true;
      },
    });
    render(WorkingFileDiff, {
      repoId: 3,
      path: "Assets/Audio.wav",
      side: "staged",
      revision: 0,
      perHunk: false,
      onhunk: () => {},
    });

    const btn = await screen.findByRole("button", { name: "Fetch LFS object (4.0 KB)" });
    await fireEvent.click(btn);

    expect(calls.find((c) => c.cmd === "lfs_fetch_object")?.args).toEqual({
      id: 3,
      oid,
      size: 4096,
      path: "Assets/Audio.wav",
    });
    await waitFor(() => expect(screen.getAllByText("Hydrated")).toHaveLength(2));
  });

  it("opens a prefab in Visual first, and the view is remembered per format", async () => {
    prefs.views = {};
    const structural = { path: "Gun.prefab", addedObjects: 1, removedObjects: 0, modifiedProperties: 0, objects: [] };
    mockCommands({
      worktree_file_diff: () => diff("raw yaml line"),
      worktree_unity_diff: () => structural,
      model_info: () => {
        throw "Gun.prefab is not in that version";
      },
    });
    render(WorkingFileDiff, { repoId: 3, path: "Gun.prefab", side: "unstaged", perHunk: false, revision: 0, onhunk: vi.fn() });
    expect(await screen.findByRole("application", { name: "3D preview of Gun.prefab" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Visual" })).toHaveAttribute("aria-checked", "true");
    await fireEvent.click(screen.getByRole("radio", { name: "Structure" }));
    expect(JSON.parse(localStorage.getItem("gitgud.views") ?? "{}")).toEqual({ prefab: "structure" });
    expect(screen.queryByRole("application")).toBeNull();
    expect(await screen.findByText("+1 objects")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("radio", { name: "Source" }));
    expect(await screen.findByText("raw yaml line")).toBeInTheDocument();
  });

  it("shows a loading bar instead of the old view while a slow structural diff loads", async () => {
    prefs.views = { prefab: "structure" };
    let finish: (() => void) | null = null;
    const structural = { path: "Gun.prefab", addedObjects: 1, removedObjects: 0, modifiedProperties: 0, objects: [] };
    mockCommands({
      worktree_file_diff: () => diff("raw yaml line"),
      worktree_unity_diff: () => (finish ? new Promise((resolve) => (finish = () => resolve(structural))) : structural),
    });
    render(WorkingFileDiff, { repoId: 3, path: "Gun.prefab", side: "unstaged", revision: 0, perHunk: true, onhunk: vi.fn() });
    expect(await screen.findByText("+1 objects")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("radio", { name: "Source" }));
    expect(await screen.findByText("raw yaml line")).toBeInTheDocument();

    finish = () => {};
    await fireEvent.click(screen.getByRole("radio", { name: "Structure" }));

    expect(await screen.findByRole("progressbar", { name: "Loading structural diff…" })).toBeInTheDocument();
    expect(screen.queryByText("raw yaml line")).toBeNull();
    finish();
    expect(await screen.findByText("+1 objects")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });
});
