import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import CommandPalette from "./CommandPalette.svelte";

const mkRepo = () => ({
  repoName: "boat",
  branches: [
    { name: "main", current: true },
    { name: "feature/physics", current: false },
    { name: "hotfix/sound", current: false },
  ],
  canPull: true,
  canPush: true,
  canStash: false,
  canUndo: true,
  canRedo: false,
  onfetch: vi.fn(),
  onpull: vi.fn(),
  onpush: vi.fn(),
  onstash: vi.fn(),
  onundo: vi.fn(),
  onredo: vi.fn(),
  onnewbranch: vi.fn(),
  oncheckout: vi.fn(),
  onmerge: vi.fn(),
  onrebase: vi.fn(),
  ondeletebranch: vi.fn(),
  onfind: vi.fn(),
  onoplog: vi.fn(),
});

describe("CommandPalette", () => {
  it("filters commands and invokes selected command", async () => {
    const repo = mkRepo();
    const onclose = vi.fn();
    render(CommandPalette, {
      repo,
      onopenrepo: vi.fn(),
      onscanfolder: vi.fn(),
      onclone: vi.fn(),
      onquickswitch: vi.fn(),
      onshortcuts: vi.fn(),
      onsettings: vi.fn(),
      onclose,
    });

    const input = screen.getByLabelText("Filter commands");
    await fireEvent.input(input, { target: { value: "fetch remotes" } });
    await fireEvent.keyDown(input, { key: "Enter" });

    expect(onclose).toHaveBeenCalled();
    expect(repo.onfetch).toHaveBeenCalled();
  });

  it("steps into inline branch picker and supports Backspace back to commands", async () => {
    const repo = mkRepo();
    const onclose = vi.fn();
    render(CommandPalette, {
      repo,
      onopenrepo: vi.fn(),
      onscanfolder: vi.fn(),
      onclone: vi.fn(),
      onquickswitch: vi.fn(),
      onshortcuts: vi.fn(),
      onsettings: vi.fn(),
      onclose,
    });

    await fireEvent.click(screen.getByRole("option", { name: /Checkout branch/ }));
    const argInput = screen.getByLabelText("Filter Checkout branch");
    expect(screen.getByRole("option", { name: /feature\/physics/ })).toBeInTheDocument();

    // Backspace from empty arg query returns to main command list.
    await fireEvent.keyDown(argInput, { key: "Backspace" });
    expect(screen.getByLabelText("Filter commands")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("option", { name: /Checkout branch/ }));
    await fireEvent.input(screen.getByLabelText("Filter Checkout branch"), { target: { value: "hotfix" } });
    await fireEvent.keyDown(screen.getByLabelText("Filter Checkout branch"), { key: "Enter" });

    expect(onclose).toHaveBeenCalled();
    expect(repo.oncheckout).toHaveBeenCalledWith("hotfix/sound");
  });

  it("shows git equivalents and skips disabled commands when navigating with arrows", async () => {
    const repo = mkRepo();
    const onclose = vi.fn();
    render(CommandPalette, {
      repo,
      onopenrepo: vi.fn(),
      onscanfolder: vi.fn(),
      onclone: vi.fn(),
      onquickswitch: vi.fn(),
      onshortcuts: vi.fn(),
      onsettings: vi.fn(),
      onclose,
    });

    expect(screen.getByText("git fetch --all")).toBeInTheDocument();
    const input = screen.getByLabelText("Filter commands");
    // Step down past Fetch (0), Pull (1), Push (2); Stash (3) is disabled so it should jump to Checkout (4).
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });

    const checkoutOption = screen.getByRole("option", { name: /Checkout branch/ });
    expect(checkoutOption).toHaveAttribute("aria-selected", "true");
  });
});
