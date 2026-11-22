import { fireEvent, render, screen } from "@testing-library/svelte";
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
    open_repo: ({ path }: Commands["open_repo"]["args"]) => ({ id: next++, root: path, rows: 1 }),
    repo_status: () => STATUS,
    graph_rows: () => [ROW],
    close_repo: () => null,
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
        return { id: 1, root: path, rows: 1 };
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
