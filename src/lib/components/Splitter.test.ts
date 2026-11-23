import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Splitter from "./Splitter.svelte";

describe("Splitter", () => {
  it("tracks vertical pointer drags and cleans up body styles", async () => {
    const onstart = vi.fn();
    const ondrag = vi.fn();
    const onreset = vi.fn();
    render(Splitter, { orientation: "vertical", label: "Resize sidebar", onstart, ondrag, onreset });

    const sep = screen.getByRole("separator", { name: "Resize sidebar" });
    await fireEvent.pointerDown(sep, { button: 0, clientX: 200, pointerId: 1 });
    expect(onstart).toHaveBeenCalledTimes(1);
    expect(sep).toHaveClass("dragging");
    expect(document.body.style.cursor).toBe("col-resize");

    await fireEvent.pointerMove(window, { clientX: 245, pointerId: 1 });
    expect(ondrag).toHaveBeenCalledWith(45);

    await fireEvent.pointerUp(window, { pointerId: 1 });
    expect(sep).not.toHaveClass("dragging");
    expect(document.body.style.cursor).toBe("");

    await fireEvent.dblClick(sep);
    expect(onreset).toHaveBeenCalledTimes(1);
  });

  it("supports keyboard stepping and Home reset for horizontal separators", async () => {
    const onstep = vi.fn();
    const onreset = vi.fn();
    render(Splitter, { orientation: "horizontal", label: "Resize file list", ondrag: vi.fn(), onstep, onreset });

    const sep = screen.getByRole("separator", { name: "Resize file list" });
    await fireEvent.keyDown(sep, { key: "ArrowDown" });
    await fireEvent.keyDown(sep, { key: "ArrowUp" });
    await fireEvent.keyDown(sep, { key: "Home" });

    expect(onstep.mock.calls).toEqual([[16], [-16]]);
    expect(onreset).toHaveBeenCalledTimes(1);
  });
});
