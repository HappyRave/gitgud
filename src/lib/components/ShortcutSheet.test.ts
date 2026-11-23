import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ShortcutSheet from "./ShortcutSheet.svelte";

describe("ShortcutSheet", () => {
  it("renders shortcut groups, filters by key or label, and handles Escape", async () => {
    const onclose = vi.fn();
    render(ShortcutSheet, { onclose });

    expect(screen.getByText("Command palette")).toBeInTheDocument();
    expect(screen.getByText("Stage / unstage / discard hunk")).toBeInTheDocument();

    const input = screen.getByLabelText("Filter shortcuts") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "Ctrl+K" } });
    expect(screen.getByText("Command palette")).toBeInTheDocument();
    expect(screen.queryByText("Approve and commit proposal")).not.toBeInTheDocument();

    // First Escape clears the non-empty filter instead of closing.
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(input.value).toBe("");
    expect(onclose).not.toHaveBeenCalled();

    // Second Escape closes the sheet when filter is empty.
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(onclose).toHaveBeenCalledTimes(1);
  });
});
