import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ContextMenu from "./ContextMenu.svelte";

describe("ContextMenu", () => {
  it("clamps coordinates within the viewport and handles keyboard navigation", async () => {
    const originalRect = HTMLElement.prototype.getBoundingClientRect;
    HTMLElement.prototype.getBoundingClientRect = () =>
      ({ width: 220, height: 120, top: 0, left: 0, bottom: 120, right: 220, x: 0, y: 0, toJSON: () => ({}) }) as DOMRect;
    try {
      const runFirst = vi.fn();
      const runSecond = vi.fn();
      const onclose = vi.fn();

      render(ContextMenu, {
        x: window.innerWidth - 10,
        y: window.innerHeight - 10,
        items: [
          { label: "First", run: runFirst },
          { label: "Second", hint: "F2", run: runSecond },
        ],
        onclose,
      });

      const menu = screen.getByRole("menu");
      await vi.waitFor(() => {
        expect(parseInt(menu.style.left, 10)).toBeLessThanOrEqual(window.innerWidth - 224);
        expect(parseInt(menu.style.top, 10)).toBeLessThanOrEqual(window.innerHeight - 124);
      });

      await fireEvent.keyDown(window, { key: "ArrowDown" });
      expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: /Second/ }));

      await fireEvent.keyDown(window, { key: "Escape" });
      expect(onclose).toHaveBeenCalled();
    } finally {
      HTMLElement.prototype.getBoundingClientRect = originalRect;
    }
  });
});
