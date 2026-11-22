import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "$lib/test/ipc";
import FindBar from "./FindBar.svelte";

function setup(rows: number[] = [3, 9, 40]) {
  const calls = mockCommands({ graph_find: () => rows });
  const onreveal = vi.fn();
  const onclose = vi.fn();
  render(FindBar, { repoId: 2, graphVersion: 0, onreveal, onclose });
  return { calls, onreveal, onclose };
}

const field = () => screen.getByRole("textbox", { name: "Find commits" });

describe("FindBar", () => {
  it("focuses itself, searches after a pause, and reveals the first match", async () => {
    const { calls, onreveal } = setup();
    expect(field()).toHaveFocus();

    await fireEvent.input(field(), { target: { value: "blimp" } });

    await vi.waitFor(() => expect(onreveal).toHaveBeenCalledWith(3));
    expect(calls).toEqual([{ cmd: "graph_find", args: { id: 2, query: "blimp" } }]);
    expect(screen.getByText("1 of 3")).toBeInTheDocument();
  });

  it("Enter and Shift+Enter walk the matches, wrapping around", async () => {
    const { onreveal } = setup();
    await fireEvent.input(field(), { target: { value: "x" } });
    await screen.findByText("1 of 3");

    await fireEvent.keyDown(field(), { key: "Enter" });
    await fireEvent.keyDown(field(), { key: "Enter", shiftKey: true });
    await fireEvent.keyDown(field(), { key: "Enter", shiftKey: true });

    expect(onreveal.mock.calls.map(([r]) => r)).toEqual([3, 9, 3, 40]);
    expect(screen.getByText("3 of 3")).toBeInTheDocument();
  });

  it("says when nothing matches; Esc closes", async () => {
    const { onclose, onreveal } = setup([]);
    await fireEvent.input(field(), { target: { value: "zzz" } });

    expect(await screen.findByText("no match")).toBeInTheDocument();
    await fireEvent.keyDown(field(), { key: "Escape" });
    expect(onclose).toHaveBeenCalled();
    expect(onreveal).not.toHaveBeenCalled();
  });
});
