import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { OpEntry } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import OpLogPanel from "./OpLogPanel.svelte";

const at = (h: number, m: number) => new Date(2026, 10, 22, h, m).getTime() / 1000;

const ENTRIES: OpEntry[] = [
  { id: 4, time: at(13, 29), action: "Redone later", undone: true, paths: [] },
  { id: 3, time: at(13, 24), action: "Discard Engine.cs", undone: false, paths: ["Engine.cs"] },
  { id: 2, time: at(13, 21), action: 'Commit "Hunk A only"', undone: false, paths: [] },
  { id: 1, time: at(13, 20), action: "Stage hunk in Engine.cs", undone: false, paths: [] },
];

function setup(entries = ENTRIES) {
  const calls = mockCommands({ op_log: () => entries });
  const handlers = { onundo: vi.fn(), onrestoreto: vi.fn(), onrestorefiles: vi.fn() };
  const view = render(OpLogPanel, { repoId: 2, revision: 0, ...handlers });
  return { calls, view, ...handlers };
}

const rows = () => within(screen.getByRole("list", { name: "Actions" })).getAllByRole("listitem");

describe("OpLogPanel", () => {
  it("lists every action newest first with its time", async () => {
    const { calls } = setup();

    await screen.findByText("Discard Engine.cs");
    expect(rows().map((r) => r.querySelector(".tm")!.textContent + " " + r.querySelector(".t")!.textContent)).toEqual([
      "13:29 Redone later",
      "13:24 Discard Engine.cs",
      '13:21 Commit "Hunk A only"',
      "13:20 Stage hunk in Engine.cs",
    ]);
    expect(calls).toEqual([{ cmd: "op_log", args: { id: 2 } }]);
  });

  it("offers Undo on the newest live entry and Restore on older ones; undone ones are marked", async () => {
    const { onundo, onrestoreto } = setup();
    await screen.findByText("Discard Engine.cs");

    expect(rows()[0]).toHaveClass("undone");
    expect(within(rows()[0]).getByText("undone")).toBeInTheDocument();
    await fireEvent.click(within(rows()[1]).getByRole("button", { name: "Undo" }));
    await fireEvent.click(within(rows()[3]).getByRole("button", { name: "Restore" }));

    expect(onundo).toHaveBeenCalled();
    expect(onrestoreto).toHaveBeenCalledWith(ENTRIES[3]);
  });

  it("a selected discard shows its snapshot and both restores", async () => {
    const { onrestorefiles, onrestoreto } = setup();
    await fireEvent.click(await screen.findByRole("button", { name: /Discard Engine\.cs/ }));

    const card = screen.getByLabelText("Entry 13:24");
    expect(card).toHaveTextContent("Snapshot before: Engine.cs");
    await fireEvent.click(within(card).getByRole("button", { name: "Restore these files" }));
    await fireEvent.click(within(card).getByRole("button", { name: "Restore whole repo to 13:24" }));

    expect(onrestorefiles).toHaveBeenCalledWith(ENTRIES[1]);
    expect(onrestoreto).toHaveBeenCalledWith(ENTRIES[1]);
  });

  it("a non-destructive entry offers only the whole-repo restore", async () => {
    setup();
    await fireEvent.click(await screen.findByRole("button", { name: /Commit "Hunk A only"/ }));

    const card = screen.getByLabelText("Entry 13:21");
    expect(within(card).getAllByRole("button").map((b) => b.textContent)).toEqual(["Restore whole repo to 13:21"]);
  });

  it("says when there is nothing yet", async () => {
    setup([]);
    expect(await screen.findByText(/Nothing yet/)).toBeInTheDocument();
  });

  it("reloads when the revision moves", async () => {
    let list = ENTRIES.slice(1);
    mockCommands({ op_log: () => list });
    const view = render(OpLogPanel, { repoId: 2, revision: 0, onundo: vi.fn(), onrestoreto: vi.fn(), onrestorefiles: vi.fn() });
    await screen.findByText("Discard Engine.cs");

    list = [{ id: 5, time: at(14, 0), action: "Stage a.txt", undone: false, paths: [] }, ...list];
    await view.rerender({ repoId: 2, revision: 1, onundo: vi.fn(), onrestoreto: vi.fn(), onrestorefiles: vi.fn() });

    expect(await screen.findByText("Stage a.txt")).toBeInTheDocument();
  });

  it("disables restores while busy", async () => {
    mockCommands({ op_log: () => ENTRIES });
    render(OpLogPanel, { repoId: 2, revision: 0, busy: true, onundo: vi.fn(), onrestoreto: vi.fn(), onrestorefiles: vi.fn() });
    await screen.findByText("Discard Engine.cs");

    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Restore" }).every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
  });

  it("shows reclaimable storage on the prune button and triggers onprune", async () => {
    mockCommands({ op_log: () => ENTRIES });
    const onprune = vi.fn();
    render(OpLogPanel, {
      repoId: 2,
      revision: 0,
      stats: { entryCount: 12, totalBytes: 8192, prunableCount: 5, reclaimableBytes: 3584 },
      onundo: vi.fn(),
      onrestoreto: vi.fn(),
      onrestorefiles: vi.fn(),
      onprune,
    });
    await screen.findByText("Discard Engine.cs");

    expect(screen.getByRole("region", { name: "Op log" })).toHaveTextContent("12 entries · 8.0 KB");
    const btn = screen.getByRole("button", { name: "Prune old (5 · 3.5 KB)" });
    await fireEvent.click(btn);
    expect(onprune).toHaveBeenCalledTimes(1);
  });
});
