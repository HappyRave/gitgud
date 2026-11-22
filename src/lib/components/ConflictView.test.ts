import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { MergeState } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import ConflictView from "./ConflictView.svelte";

const STATE: MergeState = {
  theirs: "other",
  message: "Merge branch 'other'",
  conflicts: [
    { path: "Ship.prefab", kind: "unityYaml", resolved: false },
    { path: "a.cs", kind: "code", resolved: false },
    { path: "t.png", kind: "binary", resolved: false },
  ],
};

function setup(merge: MergeState = STATE, sides = { yours: { size: 10, lfs: false }, theirs: null }) {
  const calls = mockCommands({
    conflict_text: ({ path }) => {
      if (path === "Ship.prefab") {
        throw "Ship.prefab is binary: pick a side";
      }
      return { finalNewline: true, chunks: [{ kind: "conflict", yours: ["y"], base: [], theirs: ["t"] }] };
    },
    conflict_sides: () => sides,
  });
  const onresolvewith = vi.fn(async () => true);
  const onresolveside = vi.fn(async () => true);
  const view = render(ConflictView, {
    repoId: 1,
    merge,
    revision: 0,
    branch: "main",
    how: new Map([["a.cs", "use both"]]),
    message: "Merge branch 'other'",
    onresolvewith,
    onresolveside,
    oncommit: vi.fn(),
  });
  return { calls, onresolvewith, onresolveside, view };
}

describe("ConflictView", () => {
  it("groups the list by kind and opens the first open file", async () => {
    setup();
    const list = screen.getByRole("navigation", { name: "Conflicts" });

    expect(within(list).getAllByRole("listitem").map((l) => l.textContent?.replace(/\s+/g, " ").trim())).toEqual([
      "Unity YAML1",
      "U Ship.prefab text merge",
      "Code1",
      "U a.cs text merge",
      "Binary1",
      "U t.png pick a side",
    ]);
    expect(within(list).getByRole("button", { current: true })).toHaveTextContent("Ship.prefab");
    // A Unity file git can't merge as text falls back to picking a side.
    expect(await screen.findByRole("region", { name: "Pick a side for Ship.prefab" })).toBeInTheDocument();
  });

  it("a deleted side offers keeping the deletion", async () => {
    setup();
    await fireEvent.click(screen.getByRole("button", { name: /t\.png/ }));
    const pick = await screen.findByRole("region", { name: "Pick a side for t.png" });

    expect(await within(pick).findByText("Deleted on this side")).toBeInTheDocument();
    expect(within(pick).getByRole("button", { name: /Keep it deleted/ })).toBeInTheDocument();
  });

  it("after a pick, moves to the next open file", async () => {
    const { onresolveside, view } = setup();
    await fireEvent.click(screen.getByRole("button", { name: /t\.png/ }));
    await screen.findByRole("region", { name: "Pick a side for t.png" });
    await screen.findByText("10 B");

    await fireEvent.keyDown(window, { key: "1" });
    expect(onresolveside).toHaveBeenCalledWith("t.png", "yours");
    await view.rerender({
      merge: { ...STATE, conflicts: STATE.conflicts.map((c) => (c.path === "t.png" ? { ...c, resolved: true } : c)) },
    });

    expect(await screen.findByRole("region", { name: "Pick a side for Ship.prefab" })).toBeInTheDocument();
  });

  it("a resolved file says how, and Next conflict moves on", async () => {
    setup({ ...STATE, conflicts: STATE.conflicts.map((c) => (c.path === "a.cs" ? { ...c, resolved: true } : c)) });
    await fireEvent.click(screen.getByRole("button", { name: /a\.cs/ }));

    expect(screen.getByText("Resolved: use both")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Next conflict" }));
    expect(await screen.findByRole("region", { name: "Pick a side for t.png" })).toBeInTheDocument();
  });

  it("with everything resolved, shows the summary and its message", async () => {
    setup({ ...STATE, conflicts: STATE.conflicts.map((c) => ({ ...c, resolved: true })) });

    const summary = screen.getByRole("region", { name: "Merge summary" });
    expect(within(summary).getByText("3 files resolved")).toBeInTheDocument();
    expect(within(summary).getByRole("textbox", { name: "Merge commit message" })).toHaveValue("Merge branch 'other'");
  });
});
