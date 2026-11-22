import { describe, expect, it } from "vitest";
import {
  closeRepo,
  commit,
  gitInfo,
  graphRows,
  launchPath,
  openRepo,
  pickFolder,
  refreshRepo,
  repoStatus,
  stage,
  unstage,
} from "./api";
import { mockCommands } from "./test/ipc";

describe("gitInfo", () => {
  it("calls git_info and returns its result", async () => {
    const calls = mockCommands({ git_info: () => ({ path: "C:/git/git.exe", version: "2.56.1" }) });

    await expect(gitInfo()).resolves.toEqual({ path: "C:/git/git.exe", version: "2.56.1" });
    expect(calls.map((c) => c.cmd)).toEqual(["git_info"]);
  });

  it("rejects with the Rust error message", async () => {
    mockCommands({
      git_info: () => {
        throw "git was not found on PATH";
      },
    });

    await expect(gitInfo()).rejects.toBe("git was not found on PATH");
  });
});

describe("repo commands", () => {
  it("send their arguments under the names Rust expects", async () => {
    const opened = { id: 3, root: "C:/r", rows: 10, graphVersion: 0 };
    const calls = mockCommands({
      open_repo: () => opened,
      repo_status: () => ({ root: "C:/r", head: { kind: "branch", name: "main" }, entries: [] }),
      graph_rows: () => [],
      refresh_repo: () => opened,
      stage: () => opened,
      unstage: () => opened,
      commit: () => opened,
      close_repo: () => null,
    });

    await expect(openRepo("C:/r")).resolves.toEqual(opened);
    await repoStatus(3);
    await graphRows(3, 200, 100);
    await refreshRepo(3);
    await stage(3, ["a b.txt"]);
    await unstage(3, ["a b.txt"]);
    await commit(3, "msg\n\nbody");
    await closeRepo(3);

    expect(calls).toEqual([
      { cmd: "open_repo", args: { path: "C:/r" } },
      { cmd: "repo_status", args: { id: 3 } },
      { cmd: "graph_rows", args: { id: 3, start: 200, count: 100 } },
      { cmd: "refresh_repo", args: { id: 3 } },
      { cmd: "stage", args: { id: 3, paths: ["a b.txt"] } },
      { cmd: "unstage", args: { id: 3, paths: ["a b.txt"] } },
      { cmd: "commit", args: { id: 3, message: "msg\n\nbody" } },
      { cmd: "close_repo", args: { id: 3 } },
    ]);
  });
});

describe("launchPath", () => {
  it("returns null when no repo was given", async () => {
    mockCommands({ launch_path: () => null });

    await expect(launchPath()).resolves.toBeNull();
  });
});

describe("pickFolder", () => {
  it("opens a directory picker and returns the choice", async () => {
    const calls = mockCommands({ "plugin:dialog|open": () => "C:/r" });

    await expect(pickFolder()).resolves.toBe("C:/r");
    expect(calls[0].args).toMatchObject({ options: { directory: true, title: "Open repository" } });
  });
});

describe("mockCommands", () => {
  it("fails loudly on a command nobody mocked", async () => {
    mockCommands({});

    await expect(gitInfo()).rejects.toThrow("unmocked command: git_info");
  });
});
