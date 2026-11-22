import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "$lib/test/ipc";
import FileHistoryBlame from "./FileHistoryBlame.svelte";

describe("FileHistoryBlame", () => {
  it("loads file history, selects commits, and switches to blame", async () => {
    const calls = mockCommands({
      file_history: () => [
        {
          id: "c".repeat(40),
          shortId: "ccccccc",
          subject: "Rename player script",
          author: "Max",
          time: 1_700_100_000,
          change: "renamed",
        },
        {
          id: "b".repeat(40),
          shortId: "bbbbbbb",
          subject: "Initial player script",
          author: "Ada",
          time: 1_700_000_000,
          change: "added",
        },
      ],
      blame_file: () => [
        {
          lineNo: 1,
          commit: "b".repeat(40),
          shortCommit: "bbbbbbb",
          author: "Ada",
          time: 1_700_000_000,
          summary: "Initial player script",
          text: "line one",
        },
        {
          lineNo: 2,
          commit: "c".repeat(40),
          shortCommit: "ccccccc",
          author: "Max",
          time: 1_700_100_000,
          summary: "Rename player script",
          text: "line two",
        },
      ],
    });
    const onselectcommit = vi.fn();
    const onclose = vi.fn();

    render(FileHistoryBlame, {
      repoId: 3,
      path: "Assets/Player.cs",
      onselectcommit,
      onclose,
    });

    await screen.findByText("Rename player script");
    expect(calls[0]).toEqual({ cmd: "file_history", args: { id: 3, path: "Assets/Player.cs", limit: 200 } });

    await fireEvent.click(screen.getByRole("button", { name: /Rename player script/ }));
    expect(onselectcommit).toHaveBeenCalledWith("c".repeat(40));

    await fireEvent.click(screen.getByRole("button", { name: "Blame" }));
    await screen.findByText("line two");
    expect(calls.some((c) => c.cmd === "blame_file")).toBe(true);

    await fireEvent.click(screen.getByRole("button", { name: /bbbbbbb/ }));
    expect(onselectcommit).toHaveBeenCalledWith("b".repeat(40));

    await fireEvent.click(screen.getByRole("button", { name: "Close file inspector" }));
    expect(onclose).toHaveBeenCalled();
  });

  it("displays backend errors cleanly", async () => {
    mockCommands({
      file_history: () => {
        throw "fatal: bad path";
      },
    });

    render(FileHistoryBlame, { repoId: 1, path: "missing.txt", onclose: vi.fn() });
    expect(await screen.findByRole("alert")).toHaveTextContent("fatal: bad path");
  });
});
