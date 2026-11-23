import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { CommitDetail as Detail, FileDiff } from "$lib/api";
import { prefs } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import CommitDetail from "./CommitDetail.svelte";

const ID = "a".repeat(40);

const DETAIL: Detail = {
  id: ID,
  shortId: "aaaaaaa",
  subject: "Fix the frobnicator",
  body: "It was frobbing twice.\n\nCloses #12.",
  author: { name: "Ada", email: "ada@example.invalid", time: 1_700_000_000 },
  committer: { name: "Ada", email: "ada@example.invalid", time: 1_700_000_000 },
  parents: ["b".repeat(40)],
  files: [
    { path: "src/frob.rs", change: "modified" },
    { path: "tests/frob.rs", change: "added" },
  ],
};

const diffOf = (path: string): FileDiff => ({
  path,
  binary: false,
  hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: [{ kind: "added", old: null, new: 1, text: `in ${path}` }] }],
});

function setup(extra: Parameters<typeof mockCommands>[0] = {}) {
  const calls = mockCommands({
    commit_detail: () => DETAIL,
    commit_file_diff: ({ path }) => diffOf(path),
    ...extra,
  });
  const onclose = vi.fn();
  const view = render(CommitDetail, { repoId: 2, commit: ID, onclose });
  return { calls, onclose, view };
}

describe("CommitDetail", () => {
  it("shows the message, author, id and parents", async () => {
    setup();

    expect(await screen.findByRole("heading", { name: "Fix the frobnicator" })).toBeInTheDocument();
    expect(screen.getByText(/It was frobbing twice\./)).toBeInTheDocument();
    expect(screen.getByText(/Ada <ada@example.invalid>/)).toBeInTheDocument();
    expect(screen.getByText(ID)).toBeInTheDocument();
    expect(screen.getByText("bbbbbbb")).toBeInTheDocument();
  });

  const diffLine = (text: string) => (_: string, el: Element | null) => el?.tagName === "TD" && el.textContent === text;

  it("lists changed files and opens the first one's diff", async () => {
    const { calls } = setup();

    expect(await screen.findByText(diffLine("+in src/frob.rs"))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /src\/frob\.rs/ })).toHaveAttribute("aria-pressed", "true");
    expect(calls.find((c) => c.cmd === "commit_detail")?.args).toEqual({ id: 2, commit: ID });
    expect(calls.find((c) => c.cmd === "commit_file_diff")?.args).toEqual({ id: 2, commit: ID, path: "src/frob.rs" });
  });

  it("switches the diff to another file", async () => {
    setup();
    await screen.findByText(diffLine("+in src/frob.rs"));

    await fireEvent.click(screen.getByRole("button", { name: /tests\/frob\.rs/ }));

    expect(await screen.findByText(diffLine("+in tests/frob.rs"))).toBeInTheDocument();
    expect(screen.queryByText(diffLine("+in src/frob.rs"))).not.toBeInTheDocument();
  });

  it("names a root commit", async () => {
    setup({ commit_detail: () => ({ ...DETAIL, parents: [] }) });

    expect(await screen.findByText("none (root commit)")).toBeInTheDocument();
  });

  it("shows an error from Rust", async () => {
    setup({
      commit_detail: () => {
        throw "could not read the repository: object not found";
      },
    });

    expect(await screen.findByRole("alert")).toHaveTextContent("object not found");
  });

  it("goes back to the working copy", async () => {
    const { onclose } = setup();

    await fireEvent.click(screen.getByRole("button", { name: "Back to working copy" }));

    expect(onclose).toHaveBeenCalled();
  });

  it("drops a late answer for a commit no longer shown", async () => {
    let release!: () => void;
    const other = "c".repeat(40);
    const { view } = setup({
      commit_detail: ({ commit }) =>
        commit === ID ? new Promise<Detail>((r) => (release = () => r(DETAIL))) : { ...DETAIL, id: other, subject: "Other" },
    });

    await view.rerender({ repoId: 2, commit: other, onclose: () => {} });
    await screen.findByRole("heading", { name: "Other" });
    release();
    await new Promise((r) => setTimeout(r, 10));

    expect(screen.getByRole("heading", { name: "Other" })).toBeInTheDocument();
  });

  it("shows a loading bar while a slow structural diff loads", async () => {
    prefs.modelView = "source";
    let finish!: () => void;
    const structural = { path: "Gun.prefab", addedObjects: 1, removedObjects: 0, modifiedProperties: 0, objects: [] };
    setup({
      commit_detail: () => ({ ...DETAIL, files: [{ path: "Gun.prefab", change: "modified" }] }),
      commit_unity_diff: () => new Promise((resolve) => (finish = () => resolve(structural))),
    });

    expect(await screen.findByRole("progressbar", { name: "Loading structural diff…" })).toBeInTheDocument();
    finish();
    expect(await screen.findByText("+1 objects")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });
});
