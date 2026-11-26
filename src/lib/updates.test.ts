import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateInfo } from "$lib/api";
import { prefs } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import { checkForUpdates, dismissOffer, skipOffer, updates } from "./updates.svelte";

const OFFER: UpdateInfo = {
  version: "0.2.0",
  current: "0.1.3",
  notes: "New things.",
  portable: false,
  page: "https://github.com/HappyRave/gitgud/releases/tag/v0.2.0",
};

beforeEach(() => {
  localStorage.clear();
  prefs.skippedUpdate = null;
  updates.offer = null;
  updates.result = null;
});

describe("update checks", () => {
  it("offers a newer release at startup", async () => {
    const calls = mockCommands({ update_check: () => OFFER });
    await checkForUpdates(false);
    expect(updates.offer).toEqual(OFFER);
    expect(calls[0].args).toEqual({ manual: false });
  });

  it("stays quiet at startup about a skipped version, but a manual check still offers it", async () => {
    mockCommands({ update_check: () => OFFER });
    await checkForUpdates(false);
    skipOffer();
    expect(updates.offer).toBeNull();
    expect(localStorage.getItem("gitgud.update.skipped")).toBe("0.2.0");

    await checkForUpdates(false);
    expect(updates.offer).toBeNull();

    await checkForUpdates(true);
    expect(updates.offer).toEqual(OFFER);
  });

  it("offers a release newer than the skipped one", async () => {
    prefs.skippedUpdate = "0.1.9";
    mockCommands({ update_check: () => OFFER });
    await checkForUpdates(false);
    expect(updates.offer).toEqual(OFFER);
  });

  it("Later closes the popup without skipping", async () => {
    mockCommands({ update_check: () => OFFER });
    await checkForUpdates(false);
    dismissOffer();
    expect(updates.offer).toBeNull();
    await checkForUpdates(false);
    expect(updates.offer).toEqual(OFFER);
  });

  it("a manual check reports up to date or the error", async () => {
    mockCommands({ update_check: () => null });
    await checkForUpdates(true);
    expect(updates.result).toBe("gitgud is up to date.");

    mockCommands({
      update_check: () => {
        throw "cannot check for updates: offline";
      },
    });
    await checkForUpdates(true);
    expect(updates.result).toBe("cannot check for updates: offline");
    expect(updates.checking).toBe(false);
  });

  it("a startup check only logs its error", async () => {
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    mockCommands({
      update_check: () => {
        throw "offline";
      },
    });
    await checkForUpdates(false);
    expect(updates.result).toBeNull();
    expect(log).toHaveBeenCalled();
    log.mockRestore();
  });
});
