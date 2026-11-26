import { fireEvent, render, screen } from "@testing-library/svelte";
import { emit } from "@tauri-apps/api/event";
import { describe, expect, it, vi } from "vitest";
import type { UpdateInfo } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import UpdateDialog from "./UpdateDialog.svelte";

const OFFER: UpdateInfo = {
  version: "0.2.0",
  current: "0.1.3",
  notes: "New things.",
  portable: false,
  page: "https://github.com/HappyRave/gitgud/releases/tag/v0.2.0",
};

describe("UpdateDialog", () => {
  it("shows the release and installs it with progress", async () => {
    const calls = mockCommands({ update_install: () => new Promise(() => {}) });
    render(UpdateDialog, { offer: OFFER, onskip: () => {}, onlater: () => {} });

    expect(screen.getByRole("heading", { name: "gitgud 0.2.0 is out" })).toBeInTheDocument();
    expect(screen.getByText("New things.")).toBeInTheDocument();
    // Above Settings (z-index 40), where the manual check opens it.
    expect(screen.getByRole("dialog").parentElement!.style.zIndex).toBe("60");
    await fireEvent.click(screen.getByRole("button", { name: /Install and restart/ }));
    expect(calls.some((c) => c.cmd === "update_install")).toBe(true);
    await emit("update-progress", { downloaded: 50, total: 200 });
    expect(await screen.findByRole("button", { name: "Installing… 25%" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Skip this version" })).toBeDisabled();
  });

  it("shows an install failure and lets the user try again", async () => {
    mockCommands({
      update_install: () => {
        throw "cannot install the update: bad signature";
      },
    });
    render(UpdateDialog, { offer: OFFER, onskip: () => {}, onlater: () => {} });
    await fireEvent.click(screen.getByRole("button", { name: /Install and restart/ }));
    expect(await screen.findByText("cannot install the update: bad signature")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Install and restart/ })).toBeEnabled();
  });

  it("Skip and Later hand back to the page", async () => {
    mockCommands({});
    const onskip = vi.fn();
    const onlater = vi.fn();
    render(UpdateDialog, { offer: OFFER, onskip, onlater });
    await fireEvent.click(screen.getByRole("button", { name: "Skip this version" }));
    await fireEvent.click(screen.getByRole("button", { name: /Later/ }));
    expect(onskip).toHaveBeenCalledOnce();
    expect(onlater).toHaveBeenCalledOnce();
  });

  it("a portable copy opens the release page instead", async () => {
    const calls = mockCommands({ open_external_url: () => null });
    const onlater = vi.fn();
    render(UpdateDialog, { offer: { ...OFFER, portable: true }, onskip: () => {}, onlater });
    await fireEvent.click(screen.getByRole("button", { name: "Download" }));
    expect(calls.find((c) => c.cmd === "open_external_url")?.args).toEqual({ url: OFFER.page });
    expect(onlater).toHaveBeenCalledOnce();
  });
});
