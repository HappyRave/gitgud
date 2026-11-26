import { describe, expect, it, vi } from "vitest";
import { swipeDrag } from "./compare";

function pointer(type: string, clientX: number) {
  const e = new MouseEvent(type, { clientX }) as PointerEvent;
  Object.defineProperty(e, "pointerId", { value: 1 });
  return e;
}

describe("swipeDrag", () => {
  it("follows the pointer across the host, within the margin, until let go", () => {
    const host = document.createElement("div");
    host.getBoundingClientRect = () => ({ left: 100, width: 200 }) as DOMRect;
    const handle = document.createElement("button");
    handle.setPointerCapture = vi.fn();
    const set = vi.fn();
    const action = swipeDrag(handle, { host: () => host, set, margin: 0.05 });

    handle.dispatchEvent(pointer("pointermove", 150));
    expect(set).not.toHaveBeenCalled();
    handle.dispatchEvent(pointer("pointerdown", 150));
    handle.dispatchEvent(pointer("pointermove", 150));
    expect(set).toHaveBeenLastCalledWith(0.25);
    handle.dispatchEvent(pointer("pointermove", 400));
    expect(set).toHaveBeenLastCalledWith(0.95);
    handle.dispatchEvent(pointer("pointerup", 400));
    handle.dispatchEvent(pointer("pointermove", 200));
    expect(set).toHaveBeenCalledTimes(2);
    action.destroy();
  });
});
