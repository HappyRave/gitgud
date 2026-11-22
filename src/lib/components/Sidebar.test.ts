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
};

function setup(props: { busy?: boolean; oplogOpen?: boolean } = {}) {
  const calls = mockCommands({ branches: () => DATA });
  const oncheckout = vi.fn();
  const onoplog = vi.fn();
  const view = render(Sidebar, { repoId: 4, revision: 0, oncheckout, onoplog, ...props });
  return { calls, oncheckout, onoplog, view };
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
    expect(calls).toEqual([{ cmd: "branches", args: { id: 4 } }]);
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

  it("reloads with the revision", async () => {
    const { calls, view } = setup();
    await screen.findByText("stack");

    await view.rerender({ repoId: 4, revision: 1, oncheckout: vi.fn(), onoplog: vi.fn() });

    await vi.waitFor(() => expect(calls).toHaveLength(2));
  });
});
