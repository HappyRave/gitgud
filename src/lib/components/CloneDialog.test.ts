import { fireEvent, render, screen } from "@testing-library/svelte";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "$lib/test/ipc";
import CloneDialog from "./CloneDialog.svelte";

describe("CloneDialog", () => {
  it("submits URL/dest/longpaths, streams git + LFS progress, and calls cancel_clone on Cancel", async () => {
    let rejectClone!: (err: string) => void;
    const calls = mockCommands({
      "plugin:dialog|open": () => "D:/Work",
      clone_repo: () => new Promise((_, rej) => (rejectClone = rej)),
      cancel_clone: () => {
        rejectClone("cancelled");
        return true;
      },
    });
    const oncloned = vi.fn();
    render(CloneDialog, { oncloned, oncancel: vi.fn() });

    await fireEvent.input(screen.getByLabelText("Repository URL"), {
      target: { value: "https://github.com/org/boat.git" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "Browse…" }));
    await vi.waitFor(() => expect(screen.getByLabelText("Destination folder")).toHaveValue("D:/Work/boat"));

    await fireEvent.click(screen.getByRole("button", { name: /^Clone/ }));
    await vi.waitFor(() =>
      expect(calls.find((c) => c.cmd === "clone_repo")?.args).toEqual({
        url: "https://github.com/org/boat.git",
        dest: "D:/Work/boat",
        folderPath: null,
        longPaths: true,
        lfsPartial: true,
      }),
    );

    await emit("clone-progress", {
      phase: "LFS download",
      percent: 50,
      done: 2_097_152,
      total: 4_194_304,
      lfs: {
        state: "download",
        filesDone: 1,
        filesTotal: 2,
        bytesDone: 2_097_152,
        bytesTotal: 4_194_304,
        currentFile: "tex.png",
      },
    });
    expect(await screen.findByRole("status", { name: "Clone progress" })).toHaveTextContent("LFS 1/2");

    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await vi.waitFor(() => expect(calls.some((c) => c.cmd === "cancel_clone")).toBe(true));
  });
});
