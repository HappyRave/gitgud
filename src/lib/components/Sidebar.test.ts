import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { Branches } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import Sidebar from "./Sidebar.svelte";

const DATA: Branches = {
  local: [
    { name: "master", current: true, upstream: "origin/master", ahead: 1, behind: 0 },
    { name: "feature/stack", current: false, upstream: "origin/feature/stack", ahead: 3, behind: 2 },
    { name: "conflict/ours", current: false, upstream: null, ahead: 0, behind: 0 },
  ],
  remotes: [{ name: "origin", branches: ["origin/master", "origin/feature/stack"] }],
  fetched: null,
};

function setup(props: { busy?: boolean; oplogOpen?: boolean; merging?: boolean } = {}) {
  const calls = mockCommands({ branches: () => DATA, stashes: () => [] });
  const oncheckout = vi.fn();
  const onmerge = vi.fn();
  const onrebase = vi.fn();
  const onoplog = vi.fn();
  const view = render(Sidebar, { repoId: 4, revision: 0, oncheckout, onmerge, onrebase, onoplog, ...props });
  return { calls, oncheckout, onmerge, onrebase, onoplog, view };
}

const branchRows = () =>
  within(screen.getByRole("region", { name: "Branches" }))
    .getAllByRole("button")
    .map((b) => b.textContent);

describe("Sidebar", () => {
  it("shows branches in folders with the current one ticked and ahead / behind", async () => {
    const { calls } = setup();
    await screen.findByText("stack");

    expect(branchRows()).toEqual(["✓master↑1", "▾conflict/", "ours", "▾feature/", "stack↑3 ↓2"]);
    expect(calls).toEqual([
      { cmd: "branches", args: { id: 4 } },
      { cmd: "stashes", args: { id: 4 } },
    ]);
  });

  it("folds a folder", async () => {
    setup();
    await fireEvent.click(await screen.findByRole("button", { name: /feature\// }));

    expect(branchRows()).toEqual(["✓master↑1", "▾conflict/", "ours", "▸feature/"]);
  });

  it("double-click or Enter checks out; the current branch can't be", async () => {
    const { oncheckout } = setup();
    await screen.findByText("stack");

    await fireEvent.dblClick(screen.getByRole("button", { name: /stack/ }));
    await fireEvent.keyDown(screen.getByRole("button", { name: /ours/ }), { key: "Enter" });
    await fireEvent.dblClick(screen.getByRole("button", { name: /master/ }));

    expect(oncheckout.mock.calls).toEqual([["feature/stack"], ["conflict/ours"]]);
  });

  it("opens a remote to list its branches", async () => {
    setup();
    const remotes = await screen.findByRole("region", { name: "Remotes" });
    await within(remotes).findByText("origin");
    expect(within(remotes).queryByText("master")).toBeNull();

    await fireEvent.click(within(remotes).getByRole("button", { name: /origin/ }));

    expect(within(remotes).getByText("master")).toBeInTheDocument();
    expect(within(remotes).getByText("feature/stack")).toBeInTheDocument();
  });

  it("has the op log entry", async () => {
    const { onoplog } = setup({ oplogOpen: true });

    await fireEvent.click(screen.getByRole("button", { name: "Op log" }));

    expect(onoplog).toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Op log" })).toHaveAttribute("aria-pressed", "true");
  });

  it("shows Working copy with change count when onworking is provided", async () => {
    mockCommands({ branches: () => DATA, stashes: () => [] });
    const onworking = vi.fn();
    render(Sidebar, {
      repoId: 4,
      revision: 0,
      workingOpen: false,
      changesCount: 3,
      onworking,
      oncheckout: vi.fn(),
      onmerge: vi.fn(),
      onrebase: vi.fn(),
      onoplog: vi.fn(),
    });
    const btn = await screen.findByRole("button", { name: "Working copy" });
    expect(btn).toHaveTextContent("3");
    expect(btn).toHaveAttribute("aria-pressed", "false");

    await fireEvent.click(btn);
    expect(onworking).toHaveBeenCalledTimes(1);
  });

  it("reloads with the revision", async () => {
    const { calls, view } = setup();
    await screen.findByText("stack");

    await view.rerender({ repoId: 4, revision: 1, oncheckout: vi.fn(), onmerge: vi.fn(), onrebase: vi.fn(), onoplog: vi.fn() });

    await vi.waitFor(() => expect(calls.filter((c) => c.cmd === "branches")).toHaveLength(2));
  });

  it("right-click on a branch offers check out, merge into and rebase onto it", async () => {
    const { oncheckout, onmerge, onrebase } = setup();
    await screen.findByText("stack");

    await fireEvent.contextMenu(screen.getByRole("button", { name: /stack/ }), { clientX: 30, clientY: 40 });
    const items = screen.getAllByRole("menuitem").map((i) => i.textContent?.trim());
    expect(items).toEqual([
      "Check out feature/stack Dbl-click",
      "Merge feature/stack into master…",
      "Rebase master onto feature/stack…",
    ]);

    await fireEvent.click(screen.getByRole("menuitem", { name: /Merge/ }));
    expect(onmerge).toHaveBeenCalledWith("feature/stack");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(oncheckout).not.toHaveBeenCalled();

    await fireEvent.contextMenu(screen.getByRole("button", { name: /stack/ }), { clientX: 30, clientY: 40 });
    await fireEvent.click(screen.getByRole("menuitem", { name: /Rebase/ }));
    expect(onrebase).toHaveBeenCalledWith("feature/stack");
  });

  it("compares a branch with the current one when asked", async () => {
    mockCommands({ branches: () => DATA, stashes: () => [] });
    const oncompare = vi.fn();
    render(Sidebar, { repoId: 4, revision: 0, oncheckout: vi.fn(), onmerge: vi.fn(), onrebase: vi.fn(), onoplog: vi.fn(), oncompare });
    await screen.findByText("stack");

    await fireEvent.contextMenu(screen.getByRole("button", { name: /stack/ }), { clientX: 30, clientY: 40 });
    await fireEvent.click(screen.getByRole("menuitem", { name: "Compare with master" }));
    expect(oncompare).toHaveBeenCalledWith("feature/stack");
  });

  it("offers merge on remote branches, and Esc closes the menu", async () => {
    const { onmerge } = setup();
    const remotes = await screen.findByRole("region", { name: "Remotes" });
    await fireEvent.click(await within(remotes).findByRole("button", { name: /origin/ }));

    await fireEvent.contextMenu(within(remotes).getByRole("button", { name: /feature\/stack/ }));
    expect(screen.getAllByRole("menuitem").map((i) => i.textContent?.trim())).toEqual([
      "Merge origin/feature/stack into master…",
      "Rebase master onto origin/feature/stack…",
    ]);
    await fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.queryByRole("menu")).toBeNull();
    expect(onmerge).not.toHaveBeenCalled();
  });

  it("during a merge: no checkout, merge disabled", async () => {
    const { oncheckout } = setup({ merging: true });
    await screen.findByText("stack");

    await fireEvent.dblClick(screen.getByRole("button", { name: /stack/ }));
    await fireEvent.contextMenu(screen.getByRole("button", { name: /stack/ }));

    expect(oncheckout).not.toHaveBeenCalled();
    for (const item of screen.getAllByRole("menuitem")) {
      expect(item).toBeDisabled();
    }
  });

  it("wires + button, F2/Delete keys, upstream, remote checkout, and PR links", async () => {
    const oncreatebranch = vi.fn();
    const onrenamebranch = vi.fn();
    const ondeletebranch = vi.fn();
    const onsetupstream = vi.fn();
    const oncheckoutremote = vi.fn();
    const onopenpr = vi.fn();
    mockCommands({
      branches: () => ({
        local: [
          { name: "master", current: true, upstream: "origin/master", ahead: 1, behind: 0 },
          { name: "feature/stack", current: false, upstream: null, ahead: 0, behind: 0 },
        ],
        remotes: [{ name: "origin", branches: ["origin/master", "origin/feature/stack"] }],
        fetched: null,
      }),
      stashes: () => [],
    });
    render(Sidebar, {
      repoId: 4,
      revision: 0,
      oncheckout: vi.fn(),
      onmerge: vi.fn(),
      onrebase: vi.fn(),
      onoplog: vi.fn(),
      oncreatebranch,
      onrenamebranch,
      ondeletebranch,
      onsetupstream,
      oncheckoutremote,
      onopenpr,
    });
    await screen.findByText("stack");

    await fireEvent.click(screen.getByRole("button", { name: "New branch" }));
    expect(oncreatebranch).toHaveBeenCalledWith();

    const stackBtn = screen.getByRole("button", { name: /stack/ });
    await fireEvent.keyDown(stackBtn, { key: "F2" });
    expect(onrenamebranch).toHaveBeenCalledWith("feature/stack");
    await fireEvent.keyDown(stackBtn, { key: "Delete" });
    expect(ondeletebranch).toHaveBeenCalledWith("feature/stack");
    await fireEvent.keyDown(screen.getByRole("button", { name: /master/ }), { key: "Delete" });
    expect(ondeletebranch).toHaveBeenCalledTimes(1);

    await fireEvent.contextMenu(stackBtn);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Set upstream to origin/feature/stack" }));
    expect(onsetupstream).toHaveBeenCalledWith("feature/stack", "origin/feature/stack");

    await fireEvent.contextMenu(screen.getByRole("button", { name: /master/ }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Unset upstream (origin/master)" }));
    expect(onsetupstream).toHaveBeenCalledWith("master", null);

    await fireEvent.contextMenu(stackBtn);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Open Pull Request ↗" }));
    expect(onopenpr).toHaveBeenCalledWith("feature/stack");

    const remotes = screen.getByRole("region", { name: "Remotes" });
    await fireEvent.click(within(remotes).getByRole("button", { name: /origin/ }));
    const remoteBranch = within(remotes).getByRole("button", { name: /feature\/stack/ });
    await fireEvent.contextMenu(remoteBranch);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Check out origin/feature/stack…" }));
    expect(oncheckoutremote).toHaveBeenCalledWith("origin/feature/stack");

    await fireEvent.contextMenu(remoteBranch);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Delete origin/feature/stack…" }));
    expect(ondeletebranch).toHaveBeenCalledWith("feature/stack", "origin");
  });

  it("shows stashes section and handles select, pop, apply and drop", async () => {
    const onstashselect = vi.fn();
    const onstashpop = vi.fn();
    const onstashapply = vi.fn();
    const onstashdrop = vi.fn();
    mockCommands({
      branches: () => DATA,
      stashes: () => [{ index: 0, shortId: "stash@{0}", id: "s0".repeat(20), parentId: "b0".repeat(20), baseId: "b0".repeat(20), branch: "master", message: "WIP lighting", time: 1_700_000_000 }],
    });
    render(Sidebar, {
      repoId: 4,
      revision: 0,
      oncheckout: vi.fn(),
      onmerge: vi.fn(),
      onrebase: vi.fn(),
      onoplog: vi.fn(),
      onstashselect,
      onstashpop,
      onstashapply,
      onstashdrop,
    });

    const stashes = await screen.findByRole("region", { name: "Stashes" });
    const row = within(stashes).getByRole("button", { name: /WIP lighting/ });
    await fireEvent.click(row);
    expect(onstashselect).toHaveBeenCalledWith(0);

    await fireEvent.contextMenu(row);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Pop stash" }));
    expect(onstashpop).toHaveBeenCalledWith(0);

    await fireEvent.contextMenu(row);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Apply stash" }));
    expect(onstashapply).toHaveBeenCalledWith(0);

    await fireEvent.contextMenu(row);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Drop stash" }));
    expect(onstashdrop).toHaveBeenCalledWith(0);
  });
});
