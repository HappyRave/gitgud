import { render, screen, within } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import type { Entry, Status } from "$lib/api";
import StatusList from "./StatusList.svelte";

const status = (entries: Entry[], head: Status["head"] = { kind: "branch", name: "main" }): Status => ({
  root: "C:/work/repo",
  head,
  entries,
});

const rows = (group: string) =>
  within(screen.getByRole("list", { name: group }))
    .getAllByRole("listitem")
    .map((li) => li.textContent);

describe("StatusList", () => {
  it("shows the branch and the repo root", () => {
    render(StatusList, { status: status([]) });

    expect(screen.getByTestId("head")).toHaveTextContent("main");
    expect(screen.getByText("C:/work/repo")).toBeInTheDocument();
  });

  it("says when the working tree is clean", () => {
    render(StatusList, { status: status([]) });

    expect(screen.getByText("Nothing to commit, working tree clean.")).toBeInTheDocument();
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
  });

  it("groups entries with their change letter", () => {
    render(StatusList, {
      status: status([
        { path: "both.txt", kind: "tracked", index: "modified", worktree: "modified" },
        { path: "conflict.txt", kind: "conflicted", index: null, worktree: null },
        { path: "gone.txt", kind: "tracked", index: null, worktree: "deleted" },
        { path: "new.txt", kind: "tracked", index: "added", worktree: null },
        { path: "odd.txt", kind: "tracked", index: "typeChange", worktree: null },
        { path: "stray.txt", kind: "untracked", index: null, worktree: null },
      ]),
    });

    expect(rows("Conflicts")).toEqual(["!conflict.txt"]);
    expect(rows("Staged")).toEqual(["Mboth.txt", "Anew.txt", "Todd.txt"]);
    expect(rows("Changes")).toEqual(["Mboth.txt", "Dgone.txt"]);
    expect(rows("Untracked")).toEqual(["?stray.txt"]);
    expect(screen.queryByText(/working tree clean/)).not.toBeInTheDocument();
  });

  it("hides empty groups", () => {
    render(StatusList, { status: status([{ path: "a", kind: "untracked", index: null, worktree: null }]) });

    expect(screen.getAllByRole("list")).toHaveLength(1);
    expect(screen.queryByRole("heading", { name: /Staged/ })).not.toBeInTheDocument();
  });

  it("names an unborn branch", () => {
    render(StatusList, { status: status([], { kind: "unborn", name: "main" }) });

    expect(screen.getByTestId("head")).toHaveTextContent("main (no commits yet)");
  });

  it("calls out a detached HEAD", () => {
    render(StatusList, { status: status([], { kind: "detached", id: "abc1234" }) });

    expect(screen.getByTestId("head")).toHaveTextContent("detached at abc1234");
    expect(screen.getByTestId("head")).toHaveClass("detached");
  });
});
