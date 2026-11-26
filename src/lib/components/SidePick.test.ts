import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import { font } from "$lib/test/font";
import { mockCommands } from "$lib/test/ipc";
import SidePick from "./SidePick.svelte";

describe("SidePick merge by property offer (R9)", () => {
  const path = "Assets/Boat.controller";
  const sides = () => ({
    yours: { size: 900, lfs: true, oid: "aa".repeat(32), hydrated: true },
    theirs: { size: 910, lfs: true, oid: "bb".repeat(32), hydrated: true },
  });
  const show = (onmergebyproperty = vi.fn()) => {
    render(SidePick, {
      repoId: 1,
      path,
      revision: 0,
      yoursLabel: "main",
      theirsLabel: "lighting",
      onpick: vi.fn(),
      onmergebyproperty,
    });
    return onmergebyproperty;
  };

  it("counts the changes and opens the review on M", async () => {
    mockCommands({
      conflict_sides: sides,
      property_merge_offer: () => ({ kind: "ready", yours: 3, theirs: 1, clashes: 2, objects: 1 }),
    });
    const open = show();
    expect(await screen.findByText("3 only main")).toBeInTheDocument();
    expect(screen.getByText("1 only lighting")).toBeInTheDocument();
    expect(screen.getByText("2 clashes")).toBeInTheDocument();
    expect(screen.getByText("1 object added or deleted")).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "m" });
    expect(open).toHaveBeenCalledTimes(1);
    await fireEvent.click(screen.getByRole("button", { name: /Merge by property/ }));
    expect(open).toHaveBeenCalledTimes(2);
  });

  it("fetches the versions it needs, then counts", async () => {
    let fetched = false;
    const calls = mockCommands({
      conflict_sides: sides,
      property_merge_offer: () =>
        fetched
          ? { kind: "ready", yours: 1, theirs: 1, clashes: 0, objects: 0 }
          : { kind: "needsFetch", missing: [{ side: "theirs", oid: "cc".repeat(32), size: 2048 }] },
      lfs_fetch_object: () => {
        fetched = true;
        return true;
      },
    });
    const open = show();
    expect(await screen.findByText(/needs lighting, not downloaded from LFS yet/)).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "m" });
    expect(open).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: /^Fetch it \(/ }));
    expect(calls.find((c) => c.cmd === "lfs_fetch_object")?.args).toEqual({
      id: 1,
      oid: "cc".repeat(32),
      size: 2048,
      path,
    });
    expect(await screen.findByText("1 only main")).toBeInTheDocument();
  });

  it("says why it can't", async () => {
    mockCommands({
      conflict_sides: sides,
      property_merge_offer: () => ({ kind: "unavailable", reason: "lighting is saved as binary" }),
    });
    show();
    expect(await screen.findByText("Can't merge by property: lighting is saved as binary.")).toBeInTheDocument();
  });
});

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

  it("shows the side that is left alone when the other deleted the file", async () => {
    vi.stubGlobal(
      "FontFace",
      class {
        load = () => Promise.resolve(this);
      },
    );
    Object.defineProperty(document, "fonts", { configurable: true, value: { add: vi.fn(), delete: vi.fn() } });
    const calls = mockCommands({
      conflict_sides: () => ({ yours: { size: 2048, lfs: false }, theirs: null }),
      font_info: () => font({ A: 1 }),
    });
    render(SidePick, {
      repoId: 4,
      path: "UI/Harbour.ttf",
      revision: 1,
      yoursLabel: "main",
      theirsLabel: "feature/ui",
      lock: null,
      onpick: vi.fn(),
    });
    expect(await screen.findByText("2 glyphs")).toBeInTheDocument();
    const versions = calls.filter((c) => c.cmd === "font_info").map((c) => (c.args as { version: unknown }).version);
    expect(versions).toEqual(["yours"]);
  });
});
