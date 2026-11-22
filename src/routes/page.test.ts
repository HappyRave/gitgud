import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it, vi } from "vitest";
import type { Commands, Row, Status } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import Page from "./+page.svelte";

const GIT = { path: "C:/git/git.exe", version: "2.56.1" };

const STATUS: Status = {
  root: "C:/work/repo",
  head: { kind: "branch", name: "main" },
  entries: [{ path: "a.txt", kind: "tracked", index: null, worktree: "modified" }],
};

const ROW: Row = {
  id: "a".repeat(40),
  shortId: "aaaaaaa",
  subject: "first commit",
  author: "Ada",
  time: 1_700_000_000,
  labels: [{ kind: "branch", name: "main", current: true }],
  wip: false,
  layout: { node: 0, segments: [], overflow: false },
};

type Handlers = Parameters<typeof mockCommands>[0];

/** A working app: git found, repos open with ids counting up from 1. */
function app(extra: Handlers = {}) {
  let next = 1;
  const handlers: Handlers = {
    git_info: () => GIT,
    launch_path: () => null,
    open_repo: ({ path }: Commands["open_repo"]["args"]) => ({ id: next++, root: path, rows: 1, graphVersion: 0 }),
    repo_status: () => STATUS,
    graph_rows: () => [ROW],
    close_repo: () => null,
    branches: () => ({
      local: [{ name: "main", current: true, upstream: null, ahead: 0, behind: 0 }],
      remotes: [],
    }),
    ...extra,
  };
  return mockCommands(handlers);
}

describe("start page: git", () => {
  it("shows a loading line while git is being located", () => {
    app({ git_info: () => new Promise(() => {}) });
    render(Page);

    expect(screen.getByText("Looking for git…")).toBeInTheDocument();
  });

  it("shows the git version and path", async () => {
    app();
    render(Page);

    expect(await screen.findByTestId("git-info")).toHaveTextContent("git 2.56.1 C:/git/git.exe");
  });

  it("shows the error when git is unusable", async () => {
    app({
      git_info: () => {
        throw "git 2.30.0 is too old; gitgud needs 2.45.2 or newer";
      },
    });
    render(Page);

    expect(await screen.findByRole("alert")).toHaveTextContent("git 2.30.0 is too old; gitgud needs 2.45.2 or newer");
  });
});

describe("start page: opening a repo", () => {
  it("asks for a repo when none was given at launch", async () => {
    app();
    render(Page);

    expect(await screen.findByText("Open a repository to see its history and changes.")).toBeInTheDocument();
  });

  it("opens the launch path: history in the centre, changes beside it", async () => {
    const calls = app({ launch_path: () => "C:/work/repo" });
    render(Page);

    expect(await screen.findByText("first commit")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Changes" })).toHaveTextContent("a.txt");
    expect(calls.find((c) => c.cmd === "open_repo")?.args).toEqual({ path: "C:/work/repo" });
    expect(calls.find((c) => c.cmd === "repo_status")?.args).toEqual({ id: 1 });
  });

  it("opens the folder picked in the dialog", async () => {
    const calls = app({ "plugin:dialog|open": () => "C:/work/repo" });
    render(Page);

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByText("first commit")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "plugin:dialog|open")?.args).toMatchObject({ options: { directory: true } });
  });

  it("closes the previous repo when another opens", async () => {
    const calls = app({ launch_path: () => "C:/one", "plugin:dialog|open": () => "C:/two" });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    const graphIds = () => calls.filter((c) => c.cmd === "graph_rows").map((c) => (c.args as { id: number }).id);
    await vi.waitFor(() => expect(graphIds()).toEqual([1, 2]));
    expect(calls.filter((c) => c.cmd === "close_repo").map((c) => c.args)).toEqual([{ id: 1 }]);
  });

  it("does nothing when the picker is cancelled", async () => {
    const calls = app({ "plugin:dialog|open": () => null });
    render(Page);

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByText("Open a repository to see its history and changes.")).toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "open_repo")).toBe(false);
  });

  it("shows why a folder can't be opened", async () => {
    app({
      launch_path: () => "C:/not-a-repo",
      open_repo: () => {
        throw "C:/not-a-repo is not inside a git repository";
      },
    });
    render(Page);

    expect(await screen.findByRole("alert")).toHaveTextContent("C:/not-a-repo is not inside a git repository");
  });

  it("keeps the open repo when the next one fails", async () => {
    let opens = 0;
    const calls = app({
      launch_path: () => "C:/work/repo",
      "plugin:dialog|open": () => "C:/not-a-repo",
      open_repo: ({ path }: Commands["open_repo"]["args"]) => {
        if (opens++ > 0) {
          throw `${path} is not inside a git repository`;
        }
        return { id: 1, root: path, rows: 1, graphVersion: 0 };
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("C:/not-a-repo is not inside a git repository");
    expect(screen.getByText("first commit")).toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "close_repo")).toBe(false);
  });
});

describe("start page: undo and redo", () => {
  const REVERTED = { opened: { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }, action: "Stage a.txt" };

  it("Ctrl+Z undoes, Ctrl+Y redoes, and the status line names the action", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", undo: () => REVERTED, redo: () => REVERTED });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(await screen.findByRole("status")).toHaveTextContent("Undid: Stage a.txt");

    await fireEvent.keyDown(window, { key: "y", ctrlKey: true });
    await vi.waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Redid: Stage a.txt"));
    expect(calls.filter((c) => c.cmd === "undo" || c.cmd === "redo").map((c) => [c.cmd, c.args])).toEqual([
      ["undo", { id: 1 }],
      ["redo", { id: 1 }],
    ]);
    expect(calls.filter((c) => c.cmd === "repo_status")).toHaveLength(3);
  });

  it("quick repeated presses each count, in order", async () => {
    let release!: () => void;
    let undos = 0;
    const calls = app({
      launch_path: () => "C:/work/repo",
      undo: () => {
        undos++;
        return undos === 1 ? new Promise((r) => (release = () => r(REVERTED))) : REVERTED;
      },
      redo: () => REVERTED,
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await fireEvent.keyDown(window, { key: "y", ctrlKey: true });
    release();

    await vi.waitFor(() =>
      expect(calls.filter((c) => c.cmd === "undo" || c.cmd === "redo").map((c) => c.cmd)).toEqual(["undo", "undo", "redo"]),
    );
  });

  it("Ctrl+Z in the commit message stays text undo", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", undo: () => REVERTED });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(screen.getByRole("textbox", { name: "Commit message" }), { key: "z", ctrlKey: true });

    expect(calls.some((c) => c.cmd === "undo")).toBe(false);
  });

  it("the header buttons do the same", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", undo: () => REVERTED });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Undo" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Undid: Stage a.txt");
    expect(calls.some((c) => c.cmd === "undo")).toBe(true);
  });

  it("shows why there is nothing to undo", async () => {
    app({
      launch_path: () => "C:/work/repo",
      undo: () => {
        throw "nothing to undo";
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });

    expect(await screen.findByRole("alert")).toHaveTextContent("nothing to undo");
  });
});

describe("start page: sidebar", () => {
  it("lists branches and double-click checks one out", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      branches: () => ({
        local: [
          { name: "main", current: true, upstream: "origin/main", ahead: 1, behind: 0 },
          { name: "feature/login", current: false, upstream: null, ahead: 0, behind: 0 },
        ],
        remotes: [{ name: "origin", branches: ["origin/main"] }],
      }),
      checkout: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 }),
    });
    render(Page);
    const branches = await screen.findByRole("region", { name: "Branches" });
    await within(branches).findByText("login");

    expect(within(branches).getByRole("button", { current: true })).toHaveTextContent("✓main↑1");
    await fireEvent.dblClick(within(branches).getByRole("button", { name: /login/ }));

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "checkout")?.args).toEqual({ id: 1, branch: "feature/login" }));
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "branches")).toHaveLength(2));
  });

  it("its Op log entry opens the panel", async () => {
    app({ launch_path: () => "C:/work/repo", op_log: () => [] });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));

    expect(await screen.findByRole("region", { name: "Op log" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Op log" })).toHaveAttribute("aria-pressed", "true");
  });
});

describe("start page: op log panel", () => {
  const ENTRY = { id: 7, time: 1_700_000_000, action: "Discard a.txt", undone: false, paths: ["a.txt"] };
  const OLDER = { id: 6, time: 1_699_999_000, action: "Stage b.txt", undone: false, paths: [] };

  it("Ctrl+3 opens it in the side pane and Ctrl+1 goes back", async () => {
    app({ launch_path: () => "C:/work/repo", op_log: () => [ENTRY] });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "3", ctrlKey: true });
    expect(await screen.findByRole("region", { name: "Op log" })).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "Commit" })).not.toBeInTheDocument();

    await fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(screen.queryByRole("region", { name: "Op log" })).not.toBeInTheDocument();
    expect(screen.getByRole("form", { name: "Commit" })).toBeInTheDocument();
  });

  it("Restore on an older entry restores to before it and says so", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      op_log: () => [ENTRY, OLDER],
      restore_to: () => ({ opened: { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }, action: "Stage b.txt" }),
    });
    render(Page);
    await screen.findByText("first commit");
    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));

    await fireEvent.click(await screen.findByRole("button", { name: "Restore" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Restored to before: Stage b.txt");
    expect(calls.find((c) => c.cmd === "restore_to")?.args).toEqual({ id: 1, entry: 6 });
  });

  it("Restore these files puts a discard back as a new action", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      op_log: () => [ENTRY],
      restore_files: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    });
    render(Page);
    await screen.findByText("first commit");
    await fireEvent.keyDown(window, { key: "3", ctrlKey: true });

    await fireEvent.click(await screen.findByRole("button", { name: /Discard a\.txt/ }));
    await fireEvent.click(screen.getByRole("button", { name: "Restore these files" }));

    await vi.waitFor(() => expect(calls.find((c) => c.cmd === "restore_files")?.args).toEqual({ id: 1, entry: 7 }));
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "op_log")).toHaveLength(2));
  });
});

describe("start page: fetch", () => {
  const OPENED = { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 1 };

  it("shows live progress, then refreshes when done", async () => {
    let finish!: () => void;
    const calls = app({
      launch_path: () => "C:/work/repo",
      fetch: () => new Promise((r) => (finish = () => r(OPENED))),
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Fetch" }));
    expect(screen.getByLabelText("Fetch progress")).toHaveTextContent("Fetching…");
    expect(screen.getByRole("button", { name: "Fetch" })).toBeDisabled();

    await emit("fetch-progress", { id: 1, progress: { phase: "Receiving objects", percent: 42, done: 42, total: 100 } });
    await vi.waitFor(() => expect(screen.getByLabelText("Fetch progress")).toHaveTextContent("Receiving objects 42%"));

    finish();
    expect(await screen.findByRole("status")).toHaveTextContent("Fetched");
    expect(screen.queryByLabelText("Fetch progress")).not.toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "fetch")?.args).toEqual({ id: 1 });
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "graph_rows")).toHaveLength(2));
  });

  it("Ctrl+Shift+F starts a fetch", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", fetch: () => OPENED });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.keyDown(window, { key: "F", ctrlKey: true, shiftKey: true });

    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "fetch")).toBe(true));
  });

  it("Cancel stops it and says so, without an error", async () => {
    let reject!: (e: string) => void;
    const calls = app({
      launch_path: () => "C:/work/repo",
      fetch: () => new Promise((_, r) => (reject = r)),
      cancel_fetch: () => {
        reject("cancelled");
        return true;
      },
    });
    render(Page);
    await screen.findByText("first commit");
    await fireEvent.click(screen.getByRole("button", { name: "Fetch" }));

    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Fetch cancelled");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "cancel_fetch")?.args).toEqual({ id: 1 });
  });

  it("shows git's error when the fetch fails", async () => {
    app({
      launch_path: () => "C:/work/repo",
      fetch: () => {
        throw "fatal: unable to access 'https://example.invalid/': Could not resolve host";
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.click(screen.getByRole("button", { name: "Fetch" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Could not resolve host");
  });

  it("ignores progress for another repo", async () => {
    app({ launch_path: () => "C:/work/repo", fetch: () => new Promise(() => {}) });
    render(Page);
    await screen.findByText("first commit");
    await fireEvent.click(screen.getByRole("button", { name: "Fetch" }));

    await emit("fetch-progress", { id: 9, progress: { phase: "Receiving objects", percent: 99, done: 99, total: 100 } });

    expect(screen.getByLabelText("Fetch progress")).not.toHaveTextContent("99%");
  });
});

describe("start page: working-copy diffs and hunks", () => {
  const HUNK_DIFF = {
    path: "a.txt",
    binary: false,
    hunks: [
      {
        oldStart: 3,
        oldLines: 1,
        newStart: 3,
        newLines: 1,
        lines: [
          { kind: "removed" as const, old: 3, new: null, text: "old" },
          { kind: "added" as const, old: null, new: 3, text: "new" },
        ],
      },
    ],
  };

  it("clicking a changed file shows its diff with Stage hunk", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", worktree_file_diff: () => HUNK_DIFF });
    render(Page);

    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));

    expect(await screen.findByRole("region", { name: "Unstaged changes in a.txt" })).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "worktree_file_diff")?.args).toEqual({ id: 1, path: "a.txt", side: "unstaged" });
    expect(screen.getByRole("button", { name: "Stage hunk @@ -3 +3 @@" })).toBeInTheDocument();
  });

  it("staging a hunk sends its header and refreshes the diff", async () => {
    let staged = false;
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () =>
        staged
          ? { ...STATUS, entries: [{ path: "a.txt", kind: "tracked", index: "modified", worktree: "modified" }] }
          : STATUS,
      worktree_file_diff: () => HUNK_DIFF,
      stage_hunk: () => {
        staged = true;
        return { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
    });
    render(Page);
    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));

    await fireEvent.click(await screen.findByRole("button", { name: "Stage hunk @@ -3 +3 @@" }));

    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "worktree_file_diff")).toHaveLength(2));
    expect(calls.find((c) => c.cmd === "stage_hunk")?.args).toEqual({
      id: 1,
      path: "a.txt",
      hunk: { oldStart: 3, oldLines: 1, newStart: 3, newLines: 1 },
    });
    expect(await screen.findByRole("list", { name: "Staged" })).toHaveTextContent("a.txt");
  });

  it("discarding a hunk or a file sends it to Rust, and Ctrl+Z can bring it back", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      worktree_file_diff: () => HUNK_DIFF,
      discard_hunk: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
      discard: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
      undo: () => ({ opened: { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }, action: "Discard a.txt" }),
    });
    render(Page);
    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));

    await fireEvent.click(await screen.findByRole("button", { name: "Discard hunk @@ -3 +3 @@" }));
    await vi.waitFor(() => expect(screen.getByRole("button", { name: "Discard a.txt" })).toBeEnabled());
    await fireEvent.click(screen.getByRole("button", { name: "Discard a.txt" }));
    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "discard")).toBe(true));
    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });

    expect(calls.find((c) => c.cmd === "discard_hunk")?.args).toEqual({
      id: 1,
      path: "a.txt",
      hunk: { oldStart: 3, oldLines: 1, newStart: 3, newLines: 1 },
    });
    expect(calls.find((c) => c.cmd === "discard")?.args).toEqual({ id: 1, paths: ["a.txt"] });
    expect(await screen.findByRole("status")).toHaveTextContent("Undid: Discard a.txt");
  });

  it("a stale hunk shows Rust's message", async () => {
    app({
      launch_path: () => "C:/work/repo",
      worktree_file_diff: () => HUNK_DIFF,
      stage_hunk: () => {
        throw "a.txt changed since its diff was shown; look again and retry";
      },
    });
    render(Page);
    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));

    await fireEvent.click(await screen.findByRole("button", { name: "Stage hunk @@ -3 +3 @@" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("changed since its diff was shown");
  });

  it("untracked files show their diff without hunk buttons", async () => {
    app({
      launch_path: () => "C:/work/repo",
      repo_status: () => ({ ...STATUS, entries: [{ path: "n.txt", kind: "untracked", index: null, worktree: null }] }),
      worktree_file_diff: () => ({ ...HUNK_DIFF, path: "n.txt" }),
    });
    render(Page);

    await fireEvent.click(await screen.findByRole("button", { name: "n.txt" }));

    await screen.findByRole("region", { name: "Unstaged changes in n.txt" });
    expect(screen.queryByRole("button", { name: /Stage hunk/ })).not.toBeInTheDocument();
  });

  it("the diff closes when its file leaves that side", async () => {
    let clean = false;
    app({
      launch_path: () => "C:/work/repo",
      repo_status: () => (clean ? { ...STATUS, entries: [] } : STATUS),
      worktree_file_diff: () => HUNK_DIFF,
      refresh_repo: () => {
        clean = true;
        return { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
    });
    render(Page);
    await fireEvent.click(await screen.findByRole("button", { name: "a.txt" }));
    await screen.findByRole("region", { name: "Unstaged changes in a.txt" });

    await emit("repo-changed", 1);

    await vi.waitFor(() => expect(screen.queryByRole("region", { name: /changes in a\.txt/ })).not.toBeInTheDocument());
  });
});

describe("start page: commit detail", () => {
  const WIP: Row = { ...ROW, id: "", shortId: "", subject: "Uncommitted changes", wip: true, labels: [] };
  const DETAIL = {
    id: ROW.id,
    shortId: ROW.shortId,
    subject: "first commit",
    body: "",
    author: { name: "Ada", email: "ada@example.invalid", time: 0 },
    committer: { name: "Ada", email: "ada@example.invalid", time: 0 },
    parents: [],
    files: [{ path: "a.txt", change: "added" as const }],
  };

  function withHistory(extra: Handlers = {}) {
    return app({
      launch_path: () => "C:/work/repo",
      open_repo: () => ({ id: 1, root: "C:/work/repo", rows: 2, graphVersion: 0 }),
      graph_rows: () => [WIP, ROW],
      commit_detail: () => DETAIL,
      commit_file_diff: () => ({ path: "a.txt", binary: false, hunks: [] }),
      ...extra,
    });
  }

  it("selecting a commit shows it in place of the working copy", async () => {
    const calls = withHistory();
    render(Page);

    await fireEvent.click(await screen.findByText("first commit"));

    expect(await screen.findByRole("region", { name: "Commit" })).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "Commit" })).not.toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "commit_detail")?.args).toEqual({ id: 1, commit: ROW.id });
  });

  it("selecting the WIP row goes back to the working copy", async () => {
    withHistory();
    render(Page);
    await fireEvent.click(await screen.findByText("first commit"));
    await screen.findByRole("region", { name: "Commit" });

    await fireEvent.click(screen.getByText("Uncommitted changes"));

    expect(screen.queryByRole("region", { name: "Commit" })).not.toBeInTheDocument();
    expect(screen.getByRole("form", { name: "Commit" })).toBeInTheDocument();
  });

  it("closing the detail also clears the highlighted row", async () => {
    withHistory();
    render(Page);
    const row = (await screen.findByText("first commit")).closest("li")!;
    await fireEvent.click(row);
    await screen.findByRole("region", { name: "Commit" });

    await fireEvent.click(screen.getByRole("button", { name: "Back to working copy" }));

    expect(screen.queryByRole("region", { name: "Commit" })).not.toBeInTheDocument();
    expect(row).toHaveAttribute("aria-selected", "false");
  });

  it("a rebuilt graph drops the selection", async () => {
    withHistory({ refresh_repo: () => ({ id: 1, root: "C:/work/repo", rows: 2, graphVersion: 1 }) });
    render(Page);
    await fireEvent.click(await screen.findByText("first commit"));
    await screen.findByRole("region", { name: "Commit" });

    await emit("repo-changed", 1);

    await vi.waitFor(() => expect(screen.queryByRole("region", { name: "Commit" })).not.toBeInTheDocument());
  });
});

describe("start page: outside changes", () => {
  it("refreshes status and graph when the repo changes outside gitgud", async () => {
    let entries = STATUS.entries;
    let version = 0;
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () => ({ ...STATUS, entries }),
      refresh_repo: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: version }),
    });
    render(Page);
    await screen.findByText("first commit");

    entries = [...entries, { path: "outside.txt", kind: "untracked", index: null, worktree: null }];
    version = 1;
    await emit("repo-changed", 1);

    expect(await screen.findByText("outside.txt")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "refresh_repo")?.args).toEqual({ id: 1 });
    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "graph_rows")).toHaveLength(2));
  });

  it("keeps the graph when only files changed", async () => {
    const calls = app({
      launch_path: () => "C:/work/repo",
      refresh_repo: () => ({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 }),
    });
    render(Page);
    await screen.findByText("first commit");

    await emit("repo-changed", 1);

    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "repo_status")).toHaveLength(2));
    expect(calls.filter((c) => c.cmd === "graph_rows")).toHaveLength(1);
  });

  it("ignores changes to a repo that is not open", async () => {
    const calls = app({ launch_path: () => "C:/work/repo", refresh_repo: () => ({ id: 9, root: "", rows: 0, graphVersion: 0 }) });
    render(Page);
    await screen.findByText("first commit");

    await emit("repo-changed", 9);
    await new Promise((r) => setTimeout(r, 20));

    expect(calls.some((c) => c.cmd === "refresh_repo")).toBe(false);
  });

  it("coalesces a burst into one refresh plus one follow-up", async () => {
    let release!: () => void;
    let refreshes = 0;
    const calls = app({
      launch_path: () => "C:/work/repo",
      refresh_repo: () => {
        refreshes++;
        return refreshes === 1
          ? new Promise((r) => (release = () => r({ id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 })))
          : { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
    });
    render(Page);
    await screen.findByText("first commit");

    for (let i = 0; i < 5; i++) {
      await emit("repo-changed", 1);
    }
    release();

    await vi.waitFor(() => expect(refreshes).toBe(2));
    await new Promise((r) => setTimeout(r, 20));
    expect(calls.filter((c) => c.cmd === "refresh_repo")).toHaveLength(2);
  });
});

describe("start page: staging and committing", () => {
  const STAGED: Status = {
    ...STATUS,
    entries: [{ path: "a.txt", kind: "tracked", index: "modified", worktree: null }],
  };

  it("stages a file and shows the refreshed status", async () => {
    let current = STATUS;
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () => current,
      stage: () => {
        current = STAGED;
        return { id: 1, root: "C:/work/repo", rows: 1, graphVersion: 0 };
      },
    });
    render(Page);

    await fireEvent.click(await screen.findByRole("button", { name: "Stage a.txt" }));

    expect(await screen.findByRole("list", { name: "Staged" })).toHaveTextContent("a.txt");
    expect(calls.find((c) => c.cmd === "stage")?.args).toEqual({ id: 1, paths: ["a.txt"] });
    expect(screen.getByRole("button", { name: "Commit 1 file" })).toBeInTheDocument();
  });

  it("commits, then refetches the graph when its version moved", async () => {
    let version = 0;
    const subjects = ["first commit"];
    const calls = app({
      launch_path: () => "C:/work/repo",
      repo_status: () => (version === 0 ? STAGED : { ...STATUS, entries: [] }),
      graph_rows: () => subjects.map((subject, i) => ({ ...ROW, id: `${i}`, subject })),
      commit: ({ message }: Commands["commit"]["args"]) => {
        subjects.unshift(message);
        version++;
        return { id: 1, root: "C:/work/repo", rows: subjects.length, graphVersion: version };
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.input(screen.getByRole("textbox", { name: "Commit message" }), { target: { value: "second" } });
    await fireEvent.click(screen.getByRole("button", { name: "Commit 1 file" }));

    expect(await screen.findByText("second")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "commit")?.args).toEqual({ id: 1, message: "second" });
    expect(calls.filter((c) => c.cmd === "graph_rows")).toHaveLength(2);
    expect(screen.getByText("Nothing to commit, working tree clean.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Commit message" })).toHaveValue("");
  });

  it("shows git's message when a commit is rejected, and keeps the message", async () => {
    app({
      launch_path: () => "C:/work/repo",
      repo_status: () => STAGED,
      commit: () => {
        throw "lint: trailing whitespace in a.txt";
      },
    });
    render(Page);
    await screen.findByText("first commit");

    await fireEvent.input(screen.getByRole("textbox", { name: "Commit message" }), { target: { value: "wip" } });
    await fireEvent.click(screen.getByRole("button", { name: "Commit 1 file" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("lint: trailing whitespace in a.txt");
    expect(screen.getByRole("textbox", { name: "Commit message" })).toHaveValue("wip");
  });
});
