import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import QuickSwitch from "./QuickSwitch.svelte";

describe("QuickSwitch", () => {
  const folders = [{ id: "f-1", name: "Studio", parentId: null, order: 0 }];
  const repos = [
    {
      id: "r-1",
      name: "arcade-shell",
      path: "D:/Work/arcade-shell",
      folderId: "f-1",
      pinned: false,
      order: 0,
      lastOpened: 100,
      cached: null,
    },
    {
      id: "r-2",
      name: "boat-attack",
      path: "D:/Games/boat-attack",
      folderId: null,
      pinned: true,
      order: 1,
      lastOpened: 200,
      cached: null,
    },
  ];

  it("filters matches and opens on Enter or click, and closes on Esc", async () => {
    const onopen = vi.fn();
    const onclose = vi.fn();
    const dlg = render(QuickSwitch, {
      repos,
      folders,
      openPaths: new Set(["d:/games/boat-attack"]),
      onopen,
      onaddrepo: vi.fn(),
      onscanfolder: vi.fn(),
      onclone: vi.fn(),
      onclose,
    });

    const input = screen.getByRole("textbox", { name: "Go to repository" });
    await fireEvent.input(input, { target: { value: "arc" } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    await fireEvent.keyDown(dlg.container.querySelector("[role='dialog']")!, { key: "Enter" });
    expect(onopen).toHaveBeenCalledWith("D:/Work/arcade-shell");

    await fireEvent.keyDown(dlg.container.querySelector("[role='dialog']")!, { key: "Escape" });
    expect(onclose).toHaveBeenCalled();
  });
});
