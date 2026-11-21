import { describe, expect, it } from "vitest";
import { gitInfo, launchPath, pickFolder, repoStatus } from "./api";
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

describe("repoStatus", () => {
  it("passes the path to repo_status", async () => {
    const status = { root: "C:/r", head: { kind: "branch", name: "main" } as const, entries: [] };
    const calls = mockCommands({ repo_status: () => status });

    await expect(repoStatus("C:/r")).resolves.toEqual(status);
    expect(calls).toEqual([{ cmd: "repo_status", args: { path: "C:/r" } }]);
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
