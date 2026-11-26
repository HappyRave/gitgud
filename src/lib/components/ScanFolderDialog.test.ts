import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { ScanResult } from "$lib/api";
import ScanFolderDialog from "./ScanFolderDialog.svelte";

const RESCAN: ScanResult = {
  root: "C:/Work/Games",
  totalFound: 2,
  elapsedMs: 12,
  groups: [
    {
      dir: "C:/Work/Games",
      suggestedFolder: "VEX / Games",
      repos: [
        { name: "kept", path: "C:/Work/Games/kept", alreadyInLibrary: true },
        { name: "fresh", path: "C:/Work/Games/fresh", alreadyInLibrary: false },
      ],
    },
  ],
  gone: [
    { id: "r-1", name: "doomed", path: "C:/Work/Games/doomed" },
    { id: "r-2", name: "parked", path: "C:/Work/Games/parked" },
  ],
};

describe("ScanFolderDialog", () => {
  it("on a rescan, adds new repos and removes the checked gone ones", async () => {
    const onadd = vi.fn();
    render(ScanFolderDialog, { scan: RESCAN, rescanOf: "VEX / Games", onadd, oncancel: () => {} });

    expect(screen.getByRole("dialog", { name: "Rescan VEX / Games" })).toBeInTheDocument();
    const gone = screen.getByRole("list", { name: "Gone from disk" });
    expect(within(gone).getAllByRole("checkbox").every((c) => (c as HTMLInputElement).checked)).toBe(true);

    await fireEvent.click(within(gone).getByRole("checkbox", { name: "Remove parked" }));
    await fireEvent.click(screen.getByRole("button", { name: "Add 1 · remove 1" }));
    expect(onadd).toHaveBeenCalledWith([{ path: "C:/Work/Games/fresh", folderPath: "VEX / Games", name: "fresh" }], ["r-1"]);
  });

  it("removes only, when nothing new is on disk", async () => {
    const onadd = vi.fn();
    render(ScanFolderDialog, {
      scan: { ...RESCAN, groups: [], totalFound: 0 },
      rescanOf: "VEX / Games",
      onadd,
      oncancel: () => {},
    });
    expect(screen.getByText("Nothing new on disk here.")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Remove 2 repos" }));
    expect(onadd).toHaveBeenCalledWith([], ["r-1", "r-2"]);
  });

  it("a plain scan has no gone list", () => {
    render(ScanFolderDialog, { scan: { ...RESCAN, gone: undefined }, onadd: () => {}, oncancel: () => {} });
    expect(screen.getByRole("dialog", { name: "Add repositories · scan C:/Work/Games" })).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Gone from disk" })).toBeNull();
    expect(screen.getByRole("button", { name: "Add 1 repo" })).toBeEnabled();
  });
});
