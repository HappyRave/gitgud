import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "$lib/test/ipc";
import SidePick from "./SidePick.svelte";

describe("SidePick", () => {
  it("embeds ImageDiff with LFS metadata, lock badge, size delta, and refreshes on Fetch", async () => {
    const oidYours = "aa".repeat(32);
    const oidTheirs = "bb".repeat(32);
    let theirsHydrated = false;
    const calls = mockCommands({
      conflict_sides: () => ({
        yours: { size: 1024, lfs: true, oid: oidYours, hydrated: true },
        theirs: { size: 30 * 1024 * 1024, lfs: true, oid: oidTheirs, hydrated: theirsHydrated },
      }),
      lfs_fetch_object: () => {
        theirsHydrated = true;
        return true;
      },
    });
    const onpick = vi.fn();

    render(SidePick, {
      repoId: 4,
      path: "Art/Hero_D.dds",
      revision: 2,
      yoursLabel: "main",
      theirsLabel: "feature/art",
      lock: { id: "9", path: "Art/Hero_D.dds", owner: "Ada", lockedAt: "2026-11-23T12:00:00Z", ours: false },
      onpick,
    });

    const region = await screen.findByRole("region", { name: "Pick a side for Art/Hero_D.dds" });
    expect(region).toHaveTextContent("Locked by Ada");
    expect(region).toHaveTextContent("LFS binary");
    expect(screen.getByRole("region", { name: "Image diff" })).toBeInTheDocument();

    const fetchButtons = await screen.findAllByRole("button", { name: "Fetch LFS object (30 MB)" });
    await fireEvent.click(fetchButtons[0]);

    expect(calls.find((c) => c.cmd === "lfs_fetch_object")?.args).toEqual({
      id: 4,
      oid: oidTheirs,
      size: 30 * 1024 * 1024,
      path: "Art/Hero_D.dds",
    });
    await waitFor(() => expect(screen.queryAllByRole("button", { name: /Fetch LFS object/ })).toHaveLength(0));

    await fireEvent.keyDown(window, { key: "2" });
    expect(onpick).toHaveBeenCalledWith("theirs");
  });
});
