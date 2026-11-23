import { fireEvent, render as renderComponent, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { Entry, Status } from "$lib/api";
import StatusList from "./StatusList.svelte";

const status = (entries: Entry[], head: Status["head"] = { kind: "branch", name: "main" }): Status => ({
  root: "C:/work/repo",
  head,
  entries,
});

function render(props: { status: Status; busy?: boolean }) {
  const onstage = vi.fn();
  const onunstage = vi.fn();
  renderComponent(StatusList, { ...props, onstage, onunstage });
  return { onstage, onunstage };
}

/** Each row as badge + path, e.g. "Mboth.txt". */
const rows = (group: string) =>
  within(screen.getByRole("list", { name: group }))
    .getAllByRole("listitem")
    .map((li) => li.querySelector(".badge")!.textContent! + li.querySelector(".path")!.textContent!);

const ENTRIES: Entry[] = [
  { path: "both.txt", kind: "tracked", index: "modified", worktree: "modified" },
  { path: "conflict.txt", kind: "conflicted", index: null, worktree: null },
  { path: "gone.txt", kind: "tracked", index: null, worktree: "deleted" },
  { path: "new.txt", kind: "tracked", index: "added", worktree: null },
  { path: "odd.txt", kind: "tracked", index: "typeChange", worktree: null },
  { path: "stray.txt", kind: "untracked", index: null, worktree: null },
];

describe("StatusList", () => {
  it("shows the branch and the repo root", () => {
    render({ status: status([]) });

    expect(screen.getByTestId("head")).toHaveTextContent("main");
    expect(screen.getByTestId("head").closest(".ph")).toHaveAttribute("title", "C:/work/repo");
  });

  it("says when the working tree is clean", () => {
    render({ status: status([]) });

    expect(screen.getByText("Nothing to commit, working tree clean.")).toBeInTheDocument();
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
  });

  it("names the last action Ctrl+Z would undo on the clean state", () => {
    renderComponent(StatusList, { status: status([]), lastAction: "Commit \"Tune foam\"", onstage: vi.fn(), onunstage: vi.fn() });

    expect(screen.getByText("on main")).toBeInTheDocument();
    expect(screen.getByText("Last action: Commit \"Tune foam\" · Ctrl+Z undoes it")).toBeInTheDocument();
  });

  it("groups entries with their change letter", () => {
    render({ status: status(ENTRIES) });

    expect(rows("Conflicts")).toEqual(["!conflict.txt"]);
    expect(rows("Staged")).toEqual(["Mboth.txt", "Anew.txt", "Todd.txt"]);
    expect(rows("Changes")).toEqual(["Mboth.txt", "Dgone.txt"]);
    expect(rows("Untracked")).toEqual(["?stray.txt"]);
    expect(screen.queryByText(/working tree clean/)).not.toBeInTheDocument();
  });

  it("hides empty groups", () => {
    render({ status: status([{ path: "a", kind: "untracked", index: null, worktree: null }]) });

    expect(screen.getAllByRole("list")).toHaveLength(1);
    expect(screen.queryByRole("heading", { name: /Staged/ })).not.toBeInTheDocument();
  });

  it("names an unborn branch", () => {
    render({ status: status([], { kind: "unborn", name: "main" }) });

    expect(screen.getByTestId("head")).toHaveTextContent("main (no commits yet)");
  });

  it("calls out a detached HEAD", () => {
    render({ status: status([], { kind: "detached", id: "abc1234" }) });

    expect(screen.getByTestId("head")).toHaveTextContent("detached at abc1234");
    expect(screen.getByTestId("head")).toHaveClass("detached");
  });
});

describe("StatusList actions", () => {
  it("stages a changed or untracked file and unstages a staged one", async () => {
    const { onstage, onunstage } = render({ status: status(ENTRIES) });

    await fireEvent.click(screen.getByRole("button", { name: "Stage gone.txt" }));
    await fireEvent.click(screen.getByRole("button", { name: "Stage stray.txt" }));
    await fireEvent.click(screen.getByRole("button", { name: "Unstage new.txt" }));

    expect(onstage.mock.calls).toEqual([[["gone.txt"]], [["stray.txt"]]]);
    expect(onunstage.mock.calls).toEqual([[["new.txt"]]]);
  });

  it("marks a conflict resolved by staging it", async () => {
    const { onstage } = render({ status: status(ENTRIES) });

    await fireEvent.click(screen.getByRole("button", { name: "Mark resolved conflict.txt" }));

    expect(onstage).toHaveBeenCalledWith(["conflict.txt"]);
  });

  it("stages or unstages a whole group", async () => {
    const { onstage, onunstage } = render({ status: status(ENTRIES) });

    await fireEvent.click(screen.getByRole("button", { name: "Stage all (Changes)" }));
    await fireEvent.click(screen.getByRole("button", { name: "Stage all (Untracked)" }));
    await fireEvent.click(screen.getByRole("button", { name: "Unstage all (Staged)" }));

    expect(onstage.mock.calls).toEqual([[["both.txt", "gone.txt"]], [["stray.txt"]]]);
    expect(onunstage.mock.calls).toEqual([[["both.txt", "new.txt", "odd.txt"]]]);
  });

  it("offers no bulk action for conflicts", () => {
    render({ status: status(ENTRIES) });

    expect(screen.queryByRole("button", { name: /all \(Conflicts\)/ })).not.toBeInTheDocument();
  });

  it("opens a file's diff on the side it was clicked", async () => {
    const onselectfile = vi.fn();
    renderComponent(StatusList, { status: status(ENTRIES), onstage: vi.fn(), onunstage: vi.fn(), onselectfile });
    const button = (group: string, path: string) =>
      within(screen.getByRole("list", { name: group })).getByRole("button", { name: path });

    await fireEvent.click(button("Staged", "both.txt"));
    await fireEvent.click(button("Changes", "both.txt"));
    await fireEvent.click(button("Untracked", "stray.txt"));

    expect(onselectfile.mock.calls).toEqual([
      ["both.txt", "staged"],
      ["both.txt", "unstaged"],
      ["stray.txt", "unstaged"],
    ]);
    expect(within(screen.getByRole("list", { name: "Conflicts" })).queryByRole("button", { name: "conflict.txt" })).toBeNull();
  });

  it("offers Discard on unstaged and untracked files only", async () => {
    const ondiscard = vi.fn();
    renderComponent(StatusList, { status: status(ENTRIES), onstage: vi.fn(), onunstage: vi.fn(), ondiscard });

    await fireEvent.click(screen.getByRole("button", { name: "Discard gone.txt" }));
    await fireEvent.click(screen.getByRole("button", { name: "Discard stray.txt" }));

    expect(ondiscard.mock.calls).toEqual([[["gone.txt"]], [["stray.txt"]]]);
    expect(within(screen.getByRole("list", { name: "Staged" })).queryByRole("button", { name: /^Discard/ })).toBeNull();
    expect(within(screen.getByRole("list", { name: "Conflicts" })).queryByRole("button", { name: /^Discard/ })).toBeNull();
  });

  it("marks the shown file on its side only", () => {
    renderComponent(StatusList, {
      status: status(ENTRIES),
      selected: { path: "both.txt", side: "unstaged" },
      onstage: vi.fn(),
      onunstage: vi.fn(),
      onselectfile: vi.fn(),
    });

    const pressed = screen.getAllByRole("button", { pressed: true });
    expect(pressed).toHaveLength(1);
    expect(within(screen.getByRole("list", { name: "Changes" })).getByRole("button", { name: "both.txt" })).toBe(pressed[0]);
  });

  it("disables every action while busy", () => {
    render({ status: status(ENTRIES), busy: true });

    for (const button of screen.getAllByRole("button")) {
      expect(button).toBeDisabled();
    }
  });

  it("shows batch Stage/Discard bar when multiple unstaged files are selected and forwards Shift/Ctrl mods", async () => {
    const onstage = vi.fn();
    const ondiscard = vi.fn();
    const onselectfile = vi.fn();
    const onclearselection = vi.fn();
    const selectedKeys = new Set(["unstaged:both.txt", "unstaged:stray.txt"]);

    renderComponent(StatusList, {
      status: status(ENTRIES),
      selected: { path: "stray.txt", side: "unstaged" },
      selectedKeys,
      onstage,
      onunstage: vi.fn(),
      ondiscard,
      onselectfile,
      onclearselection,
    });

    const bar = screen.getByRole("region", { name: "Selected files" });
    expect(bar).toHaveTextContent("2 selected");

    await fireEvent.click(within(bar).getByRole("button", { name: "Stage 2" }));
    expect(onstage).toHaveBeenCalledWith(["both.txt", "stray.txt"]);

    await fireEvent.click(within(bar).getByRole("button", { name: "Discard 2" }));
    expect(ondiscard).toHaveBeenCalledWith(["both.txt", "stray.txt"]);

    await fireEvent.click(within(bar).getByRole("button", { name: "Clear" }));
    expect(onclearselection).toHaveBeenCalledTimes(1);

    await fireEvent.click(within(screen.getByRole("list", { name: "Changes" })).getByRole("button", { name: "gone.txt" }), {
      shiftKey: true,
    });
    expect(onselectfile).toHaveBeenCalledWith("gone.txt", "unstaged", { ctrl: false, shift: true });
  });
});
