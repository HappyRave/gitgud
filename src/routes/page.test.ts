import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import type { Status } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import Page from "./+page.svelte";

const GIT = { path: "C:/git/git.exe", version: "2.56.1" };

const STATUS: Status = {
  root: "C:/work/repo",
  head: { kind: "branch", name: "main" },
  entries: [{ path: "a.txt", kind: "tracked", index: null, worktree: "modified" }],
};

describe("start page: git", () => {
  it("shows a loading line while git is being located", () => {
    mockCommands({ git_info: () => new Promise(() => {}), launch_path: () => null });
    render(Page);

    expect(screen.getByText("Looking for git…")).toBeInTheDocument();
  });

  it("shows the git version and path", async () => {
    mockCommands({ git_info: () => GIT, launch_path: () => null });
    render(Page);

    expect(await screen.findByTestId("git-info")).toHaveTextContent("git 2.56.1 C:/git/git.exe");
  });

  it("shows the error when git is unusable", async () => {
    mockCommands({
      git_info: () => {
        throw "git 2.30.0 is too old; gitgud needs 2.45.2 or newer";
      },
      launch_path: () => null,
    });
    render(Page);

    expect(await screen.findByRole("alert")).toHaveTextContent("git 2.30.0 is too old; gitgud needs 2.45.2 or newer");
  });
});

describe("start page: opening a repo", () => {
  it("asks for a repo when none was given at launch", async () => {
    mockCommands({ git_info: () => GIT, launch_path: () => null });
    render(Page);

    expect(await screen.findByText("Open a repository to see its changes.")).toBeInTheDocument();
  });

  it("opens the launch path straight away", async () => {
    const calls = mockCommands({ git_info: () => GIT, launch_path: () => "C:/work/repo", repo_status: () => STATUS });
    render(Page);

    expect(await screen.findByRole("list", { name: "Changes" })).toHaveTextContent("a.txt");
    expect(calls.find((c) => c.cmd === "repo_status")?.args).toEqual({ path: "C:/work/repo" });
  });

  it("opens the folder picked in the dialog", async () => {
    const calls = mockCommands({
      git_info: () => GIT,
      launch_path: () => null,
      "plugin:dialog|open": () => "C:/work/repo",
      repo_status: () => STATUS,
    });
    render(Page);

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByTestId("head")).toHaveTextContent("main");
    expect(calls.find((c) => c.cmd === "plugin:dialog|open")?.args).toMatchObject({
      options: { directory: true },
    });
  });

  it("does nothing when the picker is cancelled", async () => {
    const calls = mockCommands({
      git_info: () => GIT,
      launch_path: () => null,
      "plugin:dialog|open": () => null,
    });
    render(Page);

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByText("Open a repository to see its changes.")).toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "repo_status")).toBe(false);
  });

  it("shows why a folder can't be opened", async () => {
    mockCommands({
      git_info: () => GIT,
      launch_path: () => "C:/not-a-repo",
      repo_status: () => {
        throw "C:/not-a-repo is not inside a git repository";
      },
    });
    render(Page);

    expect(await screen.findByRole("alert")).toHaveTextContent("C:/not-a-repo is not inside a git repository");
  });

  it("replaces an error with the next repo that opens", async () => {
    let fail = true;
    mockCommands({
      git_info: () => GIT,
      launch_path: () => "C:/not-a-repo",
      "plugin:dialog|open": () => "C:/work/repo",
      repo_status: () => {
        if (fail) {
          fail = false;
          throw "C:/not-a-repo is not inside a git repository";
        }
        return STATUS;
      },
    });
    render(Page);
    await screen.findByRole("alert");

    await fireEvent.click(screen.getByRole("button", { name: "Open repository…" }));

    expect(await screen.findByTestId("head")).toHaveTextContent("main");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
