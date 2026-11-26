import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { Commands, Comparison, FileChange } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import CompareView from "./CompareView.svelte";

const commit = (i: number, subject: string) => ({ id: `${i}`.repeat(40), shortId: `${i}`.repeat(7), subject, author: "JG", time: 1_800_000_000 - i * 60 });

function comparison(over: Partial<Comparison> = {}): Comparison {
  return {
    a: "a".repeat(40),
    b: "b".repeat(40),
    base: "c".repeat(40),
    mode: "brings",
    from: "c".repeat(40),
    onlyA: { count: 1, commits: [commit(1, "main: shared")] },
    onlyB: { count: 2, commits: [commit(2, "feature: a"), commit(3, "feature: b")] },
    files: [
      { path: "src/a.txt", change: "modified" },
      { path: "src/b.txt", change: "added" },
    ],
    ...over,
  };
}

const DIFF = {
  path: "src/a.txt",
  binary: false,
  hunks: [
    {
      oldStart: 1,
      oldLines: 1,
      newStart: 1,
      newLines: 1,
      lines: [
        { kind: "removed" as const, old: 1, new: null, text: "a" },
        { kind: "added" as const, old: null, new: 1, text: "a by feature" },
      ],
    },
  ],
};

/** The diff's text is split into highlighted tokens: read the viewer's whole text. */
const diffShown = () =>
  vi.waitFor(() => expect(screen.getByRole("region", { name: "File" }).textContent).toContain("a by feature"));

function setup(handlers: Partial<{ [K in keyof Commands]: (args: Commands[K]["args"]) => Commands[K]["result"] }> = {}, props = {}) {
  const calls = mockCommands({
    branches: () => ({ local: [{ name: "main", current: true, upstream: null, ahead: 0, behind: 0 }, { name: "Feature", current: false, upstream: null, ahead: 0, behind: 0 }], remotes: [{ name: "origin", branches: ["origin/main"] }], fetched: null }),
    tags: () => [{ name: "v1", target: "c".repeat(40), shortTarget: "ccccccc", annotated: false, message: null, time: 0 }],
    stashes: () => [],
    compare: () => comparison(),
    compare_file_diff: () => DIFF,
    predict_merge: () => [],
    ...handlers,
  });
  const ontake = vi.fn();
  const onclose = vi.fn();
  render(CompareView, {
    repoId: 1,
    a: { kind: "rev", rev: "main" },
    b: { kind: "rev", rev: "Feature" },
    currentBranch: "main",
    ontake,
    onclose,
    ...props,
  });
  return { calls, ontake, onclose };
}

describe("CompareView", () => {
  it("compares what B brings, lists commits per side, and shows the first file's diff", async () => {
    const { calls } = setup();

    expect(await screen.findByText("feature: a")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "compare")?.args).toEqual({
      id: 1,
      a: { kind: "rev", rev: "main" },
      b: { kind: "rev", rev: "Feature" },
      mode: "brings",
    });
    expect(screen.getByRole("heading", { name: "Only in B · 2" })).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Only in A" })).getByText("main: shared")).toBeInTheDocument();
    expect(screen.getByText("Feature merges cleanly")).toBeInTheDocument();
    await diffShown();
    expect(calls.find((c) => c.cmd === "compare_file_diff")?.args).toEqual({
      id: 1,
      from: { kind: "rev", rev: "c".repeat(40) },
      to: { kind: "rev", rev: "b".repeat(40) },
      path: "src/a.txt",
      ignoreWhitespace: false,
    });
  });

  it("switches mode, swaps sides, and says when B is fully merged", async () => {
    let merged = true;
    const { calls } = setup({
      compare: ({ mode }) => (merged && mode === "brings" ? comparison({ files: [], onlyB: { count: 0, commits: [] } }) : comparison({ mode })),
    });
    expect(await screen.findByText("Feature brings nothing: it's fully merged into main.")).toBeInTheDocument();
    expect(screen.getByText("Nothing: Feature has no commits main lacks.")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Show the snapshot difference" }));
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "compare").at(-1)?.args).toMatchObject({ mode: "snapshot" }));
    expect(screen.getByRole("radio", { name: "Snapshot difference" })).toHaveAttribute("aria-checked", "true");

    merged = false;
    await fireEvent.click(screen.getByRole("button", { name: "Swap A and B" }));
    await vi.waitFor(() =>
      expect(calls.filter((c) => c.cmd === "compare").at(-1)?.args).toMatchObject({
        a: { kind: "rev", rev: "Feature" },
        b: { kind: "rev", rev: "main" },
      }),
    );
  });

  it("predicts a conflicting merge when A is the current branch", async () => {
    setup({ predict_merge: () => ["src/a.txt"] });
    expect(await screen.findByText("Merging Feature would conflict in 1 file")).toBeInTheDocument();
  });

  it("folds big comparisons into folders with counts and kinds", async () => {
    const files: FileChange[] = Array.from({ length: 400 }, (_, i) => ({ path: `Assets/${i % 2 ? "Audio" : "Scenes"}/f${i}.${i % 2 ? "wav" : "unity"}`, change: "modified" }));
    setup({ compare: () => comparison({ files }) });

    const tree = await screen.findByRole("list", { name: "Changed files" });
    expect(within(tree).getAllByRole("button").map((b) => b.textContent)).toEqual(["▸Assets400"]);
    expect(screen.getByText("200 .unity")).toBeInTheDocument();
    expect(screen.getByText("200 .wav")).toBeInTheDocument();
    expect(screen.getByText("Pick a file to see it on both sides.")).toBeInTheDocument();

    await fireEvent.click(within(tree).getByRole("button", { name: /Assets/ }));
    expect(within(tree).getAllByRole("button").map((b) => b.textContent)).toEqual(["▾Assets400", "▸Audio200", "▸Scenes200"]);
  });

  it("takes B's version of the shown file, and Esc closes", async () => {
    const { ontake, onclose } = setup();
    await diffShown();

    await fireEvent.click(screen.getByRole("button", { name: "Take B's version" }));
    expect(ontake).toHaveBeenCalledWith("Feature", "src/a.txt");

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalled();
  });

  it("picks a side from the list or a typed hash", async () => {
    const { calls } = setup();
    await screen.findByText("feature: a");

    await fireEvent.click(screen.getByRole("button", { name: "A: main" }));
    const listbox = screen.getByRole("listbox", { name: "A side" });
    expect(within(listbox).getByText("v1")).toBeInTheDocument();
    expect(within(listbox).getByText("origin/main")).toBeInTheDocument();
    await fireEvent.click(within(listbox).getByRole("option", { name: /Working copy/ }));
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "compare").at(-1)?.args).toMatchObject({ a: { kind: "worktree" } }));

    await fireEvent.click(screen.getByRole("button", { name: "B: Feature" }));
    const find = screen.getByRole("textbox", { name: /for B/ });
    await fireEvent.input(find, { target: { value: "1a2b3c4" } });
    await fireEvent.keyDown(find, { key: "Enter" });
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "compare").at(-1)?.args).toMatchObject({ b: { kind: "rev", rev: "1a2b3c4" } }));
  });

  it("right-click on a file row opens it in Explorer from the work tree", async () => {
    const { calls } = setup({ reveal_path: () => null }, { root: "C:/work/repo" });
    const tree = await screen.findByRole("list", { name: "Changed files" });
    await fireEvent.contextMenu(within(tree).getByText("b.txt").closest("li")!);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Show in Explorer" }));
    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "reveal_path")?.args).toEqual({ root: "C:/work/repo", path: "src/b.txt" }));
  });

  it("shows git's refusal for a side that isn't a commit", async () => {
    setup({
      compare: () => {
        throw "nope isn't a branch, tag or commit here";
      },
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("nope isn't a branch, tag or commit here");
  });
});
