import { describe, expect, it } from "vitest";
import { gitInfo } from "./api";
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

describe("mockCommands", () => {
  it("fails loudly on a command nobody mocked", async () => {
    mockCommands({});

    await expect(gitInfo()).rejects.toThrow("unmocked command: git_info");
  });
});
